#!/usr/bin/env bash
# 多方言合并改造的**完整机械转换流水线**（从干净基线到「无语法错误」状态）。
#
# 用法：
#   git checkout <基线> -- src/          # 先回到改造前
#   bash scripts/dialect-migrate/run.sh  # 跑这条流水线
#
# 关键约束：
#   1. 每一步之后都要跑符号体检（audit_symbols.py）—— 它是「脚本有没有
#      悄悄吞掉代码」的唯一可靠判据（行数对比会漏判）。
#   2. 流水线是**幂等**的：中途失败可以从头重跑（除了明确标注需要重置的）。
set -euo pipefail
cd "$(dirname "$0")/../.."

S=scripts/dialect-migrate

echo "==> 0. 手工重写（lib.rs / stub.rs / 若干方言分支函数）"
python3 $S/00_rewrite_handwritten.py

echo "==> 1. cfg 三元组 → 运行期 dispatch"
python3 $S/01_convert_cfg_triples.py --apply

echo "==> 2. 残留 cfg → 运行期方言判断"
python3 $S/02_cfg_to_runtime.py --apply

echo "==> 3. 查询链 → dispatch"
python3 $S/03_wrap_query_chains.py --apply

echo "==> 4. 变量形式查询 → dispatch"
python3 $S/04_wrap_var_queries.py --apply

echo "==> 5. 手工重写的函数"
python3 $S/05_handfix.py

echo "==> 6. RETURNING 方言差异"
python3 $S/06_fix_returning.py || true

echo "==> 7. 去掉第一轮残留的多余花括号"
python3 $S/07_fix_extra_brace.py --apply

echo "==> 8. 去掉 dispatch 调用后多余的 .await"
python3 $S/08_strip_double_await.py --apply

echo "==> 9. 合并运行期方言块（内容相同的合一，不同的成 if-else 链）"
python3 $S/09_merge_dialect_blocks.py --apply

echo "==> 10. 多语句 dispatch body → 拆成「宏外 + 单表达式宏体」"
python3 $S/10_split_body.py --apply

echo "==> 11. 宏的 body 参数改成 \$q:expr（接受 async 块）"
python3 $S/11_macro_expr_matcher.py

echo "==> 12. ExecResult 的 last_insert_id 语义修正"
python3 $S/12_fix_last_insert_id.py

echo "==> 13. 把直接把 pool 交给 sqlx 的调用包进 dispatch"
python3 $S/13_fix_pool_exec.py --check /tmp/last_check.txt --apply || true

echo "==> 14. 修正 dispatch 调用里的 pool 作用域（state.pool）"
python3 $S/14_fix_pool_scope.py --check /tmp/last_check.txt --apply || true

echo "==> 15. 合并方言 if 链"
python3 $S/15_merge_if_chain.py --apply || true

echo "==> 符号体检（必须零丢失）"
python3 $S/audit_symbols.py

echo "==> 内容膨胀守卫（必须通过）"
bash $S/guard.sh

echo
echo "接下来：cargo check --message-format=short 收敛剩余错误，"
echo "然后 cargo fmt && cargo test --no-fail-fast。"

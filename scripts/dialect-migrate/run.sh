#!/usr/bin/env bash
# 多方言合并改造的**机械转换流水线**（从「改造前」的基线到「方言分支全部改成运行期分发」）。
#
# 用法：
#   git checkout 43b178b -- src/            # 先回到改造前
#   bash scripts/dialect-migrate/run.sh     # 跑这条流水线
#
# 关键约束：
#   1. 每一步之后都要跑符号体检（audit_symbols.py）—— 它是「脚本有没有
#      悄悄吞掉代码」的唯一可靠判据（行数对比会漏判）。
#   2. **dialect_guard 测试是占位符的守门人** —— 它扫出「在 postgres 构建里
#      会被编译、却仍用裸 `?` 的 SQL」。postgres 的预编译协议只认 `$1`，
#      裸 `?` 在 PG 上必然 syntax error，而 SQLite/MySQL 完全看不出来。
#      这次改造里它一次抓出 411 处。
#   3. 流水线**幂等**：中途失败可以从头重跑。
set -euo pipefail
cd "$(dirname "$0")/../.."

S=scripts/dialect-migrate
BASE="${BASE:-43b178b}"
FILES=(
  src/db/alarm.rs src/db/audit_log.rs src/db/cloud_record.rs src/db/common_channel.rs
  src/db/device.rs src/db/group.rs src/db/jt1078.rs src/db/log.rs src/db/media_server.rs
  src/db/mobile_position.rs src/db/platform.rs src/db/platform_channel.rs
  src/db/platform_group.rs src/db/platform_region.rs src/db/position_history.rs
  src/db/record_plan.rs src/db/region.rs src/db/role.rs src/db/seq_sync.rs
  src/db/stream_proxy.rs src/db/stream_push.rs src/db/user.rs src/db/user_api_key.rs
  src/handlers/alarm.rs src/handlers/cloud_record_extra.rs src/handlers/common_channel.rs
  src/handlers/device.rs src/handlers/device_control.rs src/handlers/health.rs
  src/handlers/jt1078.rs src/handlers/platform.rs src/handlers/position.rs
  src/handlers/region.rs src/handlers/server.rs src/handlers/stream.rs src/handlers/stub.rs
  src/handlers/sy_camera.rs src/handlers/sy_contract.rs src/handlers/system.rs
  src/jt1078/manager.rs src/jt1078/server.rs src/lib.rs src/router.rs
  src/scheduler/record_plan.rs src/sip/gb28181/subscription_lifecycle.rs src/sip/server.rs
  src/test_support.rs src/zlm/media_node.rs
)

echo "==> 0. 回到改造前基线（$BASE）"
git checkout "$BASE" -- src/ || true

echo "==> 1. cfg 方言分支 → 运行期按 pool 分发"
# 只转换**完整方言集合**的分支组（缺一方言的会跳过并打印出来，交人工处理）。
python3 $S/cfg_runtime.py "${FILES[@]}"

echo "==> 2. cfg 之外直接把 pool 交给 sqlx 的调用点 → dispatch 宏"
# 必须在第 1 步之后跑：否则会把已经分好方言的分支再包一层。
python3 $S/03_wrap_query_chains.py --apply
python3 $S/04_wrap_var_queries.py --apply

echo "==> 3. 宏体里的裸 ? → 运行期方言改写"
# dispatch! 的宏体在三个方言展开同一份 SQL，裸 `?` 在 postgres 上必然语法错。
python3 $S/fix_raw_placeholders.py "${FILES[@]}"

echo "==> 4. 守门人：dialect_guard 必须 0 违规"
cargo test --lib dialect_guard 2>&1 | tail -20

echo "==> 5. 符号体检（必须零丢失）"
python3 $S/audit_symbols.py

echo "==> 6. 内容膨胀守卫"
bash $S/guard.sh

echo
echo "接下来：cargo fmt && cargo clippy --all-targets --all-features && cargo test --no-fail-fast，"
echo "然后用 scripts/dialect_smoke_all.sh 起三个实例（sqlite / postgres / mysql，"
echo "只改 GBSERVER__DATABASE__URL）跑真实库冒烟。"

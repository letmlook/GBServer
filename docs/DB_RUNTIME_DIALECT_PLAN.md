# 多方言合并改造：技术方案与进度

> 目标：**一个二进制同时支持 SQLite / PostgreSQL / MySQL**，运行期按
> `database.url` 的 scheme 选库，不再用互斥的 cargo feature 决定编进哪个后端。
>
> 状态：**架构已定案并实测验证**；机械转换已完成约 70%；剩余部分需人工逐条核对
> SQL 的方言等价写法（见文末「剩余清单」）。

## 一、为什么不用 sqlx 的 `Any` 驱动

`Any` 驱动看起来正是为「运行时切库」设计的，但**实测不支持本项目必需的列类型**
（sqlx 0.7.4 + 三个真实库，2026-10-24）：

| SQL 类型 | SQLite | PostgreSQL | MySQL |
|---|---|---|---|
| 整数（INT/BIGINT/SERIAL…） | ✅ | ✅ | ✅ |
| TEXT / VARCHAR | ✅ | ✅ | ✅ |
| DOUBLE | ✅ | ✅ | ✅ |
| **REAL（float4）** | ✅ | ❌ 与 Any 的 Double(float8) 不兼容 | ✅ |
| **BOOL / BOOLEAN** | ❌ 存的是 INTEGER | ❌ `Any driver does not support the Postgres type PgTypeInfo(Bool)` | ❌ `...MySql type MySqlTypeInfo { type: Tiny }` |
| **TINYINT** | — | — | ❌ 整个类型不支持（MySQL 的 `BOOL` 就是 `TINYINT(1)`） |

失败发生在**类型映射层**（`sqlx-core/src/any/row.rs` 的 `AnyRow::map_from` 用
`AnyTypeInfo::try_from`），连值都取不到 —— 自定义 `Decode` newtype 也救不了：
`AnyValueRef.kind` 是 `pub(crate)`，外部 crate 拿不到值，`sqlx::any` 的公开白名单里
也没有 `AnyValueKind`。

本项目有 **76 处 bool 字段**（`stream_proxy` 16、`device` 15、`stream_push` 10…），
MySQL 的 `BOOL` 恰恰就是 `TINYINT(1)` —— 走 `Any` 等于把这 76 处全部改成
「SQL 层转成整数 + Rust 侧换 newtype」，且 `TINYINT` 普通列也解不了。

> 反过来记一笔 `Any` 的**优点**：它的 `i64` 解码比原生 pg 宽松
> （`try_integer()` 接受 INT2/INT4/INT8），pg 的 INT4→i64 那类坑在 `Any` 下自动消失。

## 二、采用的方案：各方言原生 pool + 运行期分发

`src/db/mod.rs` 的 `Pool` 从**编译期类型别名**改成**运行期多态枚举**：

```rust
pub enum Pool {
    Sqlite(sqlx::SqlitePool),
    Postgres(sqlx::PgPool),
    MySql(sqlx::MySqlPool),
}
```

三个驱动**同时编进同一个二进制**（`Cargo.toml` 的 `default = ["sqlite", "mysql", "postgres"]`），
`create_pool()` 按 URL scheme 建对应的池，并把**进程级方言**写进全局
（`src/db/dialect.rs` 的 `Dialect`）。

### 关键设计点

1. **`Dialect` 枚举 + 全局注入**（`src/db/dialect.rs`）
   一个进程只连一个库，方言在 `create_pool` 之后不再变 → 放进全局，
   SQL 生成函数不必把 dialect 一路透传。

2. **`dispatch!` / `dispatch_exec!` 两个宏**（`src/db/mod.rs`）
   宏体展开三次，每次 `$p` 是各自方言的具体 pool 引用，于是
   `sqlx::query_as::<_, T>(&sql).bind(..).fetch_all($p)` 在每个分支里各自完成类型推导。
   * `dispatch!` —— 返回**行**的查询（三个分支的返回类型一致）。
   * `dispatch_exec!` —— 执行语句：各方言的 `QueryResult` **类型不同**
     （`MySqlQueryResult` / `PgQueryResult` / `SqliteQueryResult`），
     直接放进同一个 `match` 会报 "arms have incompatible types"，所以用
     `ExecResult` 收敛；同时把**方言专有**的自增 id 也带出来
     （MySQL `last_insert_id()` / SQLite `last_insert_rowid()` / PG 无 → `None`，
     PG 取新 id 要走 `INSERT ... RETURNING id`）。

3. **SQL 文本统一用 `?`**，由 `dialect::current_sql()` 在运行期改写成 `$1..$n`（pg）。
   * 这让「一份 SQL 文本服务三种方言」成为常态（仓库原约定，见
     `docs/DB_DIALECT_NOTES.md` 规则 1）。
   * `Pool::sql()` 与 `dialect::current_sql()` 等价；后者不要求调用方手里有 pool
     —— handlers 层的 pool 通常是 `state.pool`，名字不统一。

4. **方言专属语法按运行期分支**（`docs/DB_DIALECT_NOTES.md` 规则 4 已有清单）：
   `RETURNING id`（MySQL 无）、`ON CONFLICT` vs `ON DUPLICATE KEY UPDATE`、
   `CREATE INDEX IF NOT EXISTS`（MySQL 无）、`ADD COLUMN IF NOT EXISTS`（MySQL 无）、
   `CAST(x AS TEXT)` vs `AS CHAR` vs `AS SIGNED`、`ILIKE`、`||` vs `CONCAT()`。

5. **schema 初始化按运行期选脚本**（`src/lib.rs`）
   `schema_sql(dialect)` 取代三个 `#[cfg]` 常量；pg / mysql 的列类型纠正
   在非对应方言直接跳过（原来靠 `#[cfg(not(feature = ...))]` 的空实现）。

## 三、实测验证

用独立 spike crate（`/tmp/any_spike`，只依赖 sqlx + 三个真实库）验证：

- `enum Pool` + 运行期分发：SQLite / PostgreSQL / MySQL **三库全绿**，
  同一份 Rust 代码、同一份 `Option<bool>` 字段、事务正常、
  `ON CONFLICT` / `ON DUPLICATE KEY` 各自成臂。
- `Any` 驱动：bool / tinyint / real 全部失败（错误原文见第一节）。

## 四、机械转换工具

`scripts/dialect-migrate/` 下是本次的转换脚本（按顺序）：

| 脚本 | 作用 |
|---|---|
| `00_rewrite_handwritten.py` | 重写 `lib.rs` / `stub.rs` / 若干方言分支函数 |
| `01_convert_cfg_triples.py` | 三方言 `cfg` 三元组 → 一份 SQL + `dispatch!` |
| `02_cfg_to_runtime.py` | 残留 `cfg` → 运行期方言判断（含递归合并嵌套三元组） |
| `03_wrap_query_chains.py` | 直接把 `pool` 交给 sqlx 的查询链 → `dispatch!` |
| `04_wrap_var_queries.py` | 变量形式（`let mut q = sqlx::query(..)` + 循环 bind）→ `dispatch!` |
| `05_handfix.py` | 脚本合不拢的函数（`{pN}` 模板、`RETURNING` 差异等） |
| `06_fix_returning.py` | `RETURNING id` 的三方言差异（MySQL 无） |
| `audit_symbols.py` | **符号体检**：与基线比对函数/类型/常量名集合 |

```bash
BASE=<改造前的提交> bash scripts/dialect-migrate/run.sh
```

### 为什么必须做符号体检

第一版脚本**静默吞掉了 19 个文件的内容**（`device.rs` 从 2576 行掉到 804 行、
丢了 13 个函数）—— 原因是按行号从后往前替换时**行号会漂移**，后续编辑就切错了位置；
且「链首..链尾」的启发式范围跨过了函数边界。

**行数对比会漏判**（三份 SQL 合一会让行数合理地减少）。可靠判据是
**符号集合差集**：`audit_symbols.py` 报告「零丢失」才算这一轮转换没破坏代码。

> 脚本改写脚本时每次都要重跑端到端体检（符号差集 + 编译错误数）。
> 本次改造在这个环节反复迭代了十余轮，脚本每修一个 bug 常引入两个新 bug ——
> 长尾必须人工核对，不能再靠自动改写。

## 五、进度与剩余清单

### 转换进度

| 阶段 | 编译错误 |
|---|---|
| 改造前（三个互斥 feature） | 0（三份构建各编译一次） |
| 第一次尝试「三个 feature 同开」 | **142**（编译期类型别名 + cfg 尾表达式写法不兼容） |
| 引入运行期 `Pool` 枚举后 | 441 |
| 机械转换 + 逐类修复后 | **131** |

**审计基线**：符号体检（`scripts/dialect-migrate/audit_symbols.py`）以**改造前的提交
`43b178b`** 为基线比对函数/类型/常量名集合 —— 早期版本误用 `HEAD` 作基线，
而 `HEAD` 已经是 WIP 提交，**丢了函数反而看不出来**（`db::platform::update` 就
是这样被卷进方言块、缩进成局部定义，调用方找不到）。

### 剩余清单（人工逐条核对）

剩余错误集中在三类，**都不是编译期能自动判定的**：

1. **方言专属 SQL 文本错配**（最需要人工）——
   `match pool.dialect() { Postgres => "...$1::text...", _ => "...?..." }`
   里两个分支的 SQL 必须是**各自方言**的形态。原代码是三个 `#[cfg]` 分支，
   合并成 match 时容易把 pg 的文本配给 sqlite。
   典型：`$1::text IS NULL`（pg 需要 cast）、`?1`（SQLite 编号占位符）、
   `CAST(x AS INTEGER)`（pg）vs `AS SIGNED`（mysql）vs `AS CHAR`（mysql）。
2. **变量作用域** —— 方言块内的 `let` 与块外使用对不上。
3. **类型推断** —— `query_scalar` 缺显式泛型、跨方言的整型宽度（INT4 vs INT8）。

**核对要点**（脚本无法代劳）：

- **bool 列**：保持 `Option<bool>` 不变（各方言原生解码），**不要**改成整数。
- **pg 专有 `$n` 文本**：逐条确认 sqlite / mysql 下有等价写法。
- **bind 数量**：pg 用 `$1` 引用同一参数一次，sqlite / mysql 的 `?` 需要重复
  绑定 —— 统一成 `?` 后 bind 链必须同步调整。

### 已修掉的「静默错误」（编译能过、运行才炸）

| 形态 | 后果 |
|---|---|
| `cfg!(feature = "postgres")` | 三个 feature 同开后恒为 true → **所有方言**都走 pg 分支（占位符 `$N`、`:text` cast 全错） |
| `#[cfg(any(feature = "postgres", test))]` 修饰纯函数 | 生产构建里函数不存在 → 编译不过 |
| `#[cfg(not(feature = "X"))]` 转成 `current().is_False()` | 非法方法名 |
| 方言 match 的两个分支配错 SQL | sqlite / mysql 上语法错或类型错 |

`cargo check --message-format=short` 的错误分布（约 430 处）：

| 层 | 错误数 | 主要形态 |
|---|---|---|
| `src/handlers/` | ~230 | `state.pool` 作用域、动态 WHERE 拼接、事务块 |
| `src/db/` | ~200 | `RETURNING` 方言差异、`{pN}` 模板、`last_insert_id` |

**核对要点**（脚本无法代劳）：

1. **pg 专有 `$n` 文本**：改造后仍有 ~116 处 SQL 里带 `$n`（原本是 pg 分支的写法）。
   这些 SQL 在 sqlite / mysql 下会直接报语法错 → 必须逐条换成 `?` + `pool.sql()`。
2. **bind 数量**：pg 用 `$1` 引用同一参数一次，sqlite / mysql 的 `?` 需要重复绑定 ——
   统一成 `?` 后 bind 链必须同步调整（第一轮转换已处理大部分）。
3. **bool 列**：保持 `Option<bool>` 不变（各方言原生解码），**不要**改成整数。
4. **`RETURNING`**：MySQL 不支持 → 该函数需要「MySQL 一支 + 其余两支共用 RETURNING」。

## 六、验证要求

```bash
cargo fmt
cargo clippy --all-targets --all-features
cargo test --no-fail-fast                        # SQLite 单测
docker compose up -d postgres
docker compose --profile mysql up -d mysql
# 用各方言构建起实例，跑 scripts/dialect_smoke.py（读路径 + 写路径）
```

改造后**不再需要** `cargo build --no-default-features --features postgres` ——
一个二进制起三个实例即可，只改 `GBSERVER__DATABASE__URL`。

---

## 七、工具链：`scripts/dialect-migrate/cfg_runtime.py`（本轮新增，主转换器）

前 27 个脚本是「边改边猜」的一代产物 —— 它们能收敛大部分机械替换，但会在
**语句形态**、**多语句块**、**变量作用域**上留下大量需要人工返工的坑
（重复 bind、变量被塞进分支内部、`?` 位置错位）。

本轮重写了一个**只做一件事**的转换器：

```bash
# 从「改造前」的基线出发
git checkout 43b178b -- src/
python3 scripts/dialect-migrate/cfg_runtime.py <文件…>   # cfg 方言分支 → 运行期分发
python3 scripts/dialect-migrate/03_wrap_query_chains.py --apply   # 剩下的 execute(pool) 链 → dispatch!
python3 scripts/dialect-migrate/04_wrap_var_queries.py --apply    # 变量形式查询 → dispatch!
```

### 设计要点

1. **不合并各方言的 SQL 文本**。每种方言保留自己那份（那本来就是对的），
   只把「选哪一份」的时机从编译期搬到运行期 —— 用
   `crate::dispatch_dialect!`（每个分支一份 SQL）。
2. **只认「归一化后完全一致」的分支才合并**（`$1`→`?`、`AS TEXT`→`AS CHAR`、
   `true`→`1` …），不一致就跳过并打印出来交给人工 —— 绝不猜着改。
3. **语句边界按括号配平判定**，不能用「遇到空行 / `#[` / `}` 就停」：
   那会把后面**共享的代码**（例如 `affected += r.rows_affected();`）一起吞进
   cfg 守卫的体里。
4. **`let X = ...` 的识别要允许类型标注**（`let row: Option<T> = ...`），
   否则变量会跑进分支内部，函数体后面就找不到它。

### 与旧脚本的关系

`03` / `04` 仍然有用（它们负责「cfg 之外直接把 pool 交给 sqlx」的调用点），
但**必须在 `cfg_runtime.py` 之后跑** —— 否则会把已经分好方言的分支再包一层。

### 四个宏（`src/db/mod.rs`）

| 宏 | 语义 |
|---|---|
| `dispatch!` | 同一份代码在三个方言展开；宏体**必须是一个 future**（不能有 `.await`） |
| `dispatch_exec!` | 同上，但把方言相关的 `QueryResult` 统一映射成 `ExecResult` |
| `dispatch_dialect!` | **方言专属 SQL**：每个分支一份自己的 SQL；分支块里可以有 `.await` 和 `?` |
| `dispatch_exec_dialect!` | 方言专属的「执行语句」，映射成 `ExecResult` |

四个宏都是 `match &$pool { ... }` —— 拿到的是**该方言的具体 pool 引用**
（`&PgPool` / `&MySqlPool` / `&SqlitePool`），不是 `Pool` 枚举本身。

### 事务（`Pool::begin()`）

事务对象的**类型**是方言相关的（`Transaction<'_, Sqlite|Pg|MySql>`），
不能跨方言共用一个变量。仓库里的做法是**局部 `macro_rules!`** 把事务体抽出来，
三个方言分支各展开一次：

```rust
macro_rules! body { ($tx:ident) => {{ /* 用 &mut *$tx 执行 */ }}; }
crate::dispatch_dialect!(
    &state.pool,
    |p| { let mut tx = p.begin().await?; let out = body!(tx)?; tx.commit().await?; Ok(out) },
    |p| { /* 同上 */ },
    |p| { /* 同上 */ },
);
```

注意 `Transaction` 本身**不是** `Executor` —— 它 `Deref` 到 `DB::Connection`，
所以执行时写 `&mut *tx`。

# 多方言（SQLite / MySQL / PostgreSQL）开发注意事项

> 为什么需要这份文档：本项目默认 feature 是 **SQLite**，`cargo test` 与日常开发都在
> SQLite 上跑；而 **SQLite 是动态类型、MySQL 宽进严出、只有 PostgreSQL 严格校验**
> 占位符语法与列类型。于是大量缺陷在 SQLite 上"完全正常"，只在 postgres 构建里炸。
> 2026-09-12 第四十三轮第一次把后端跑在真实 PostgreSQL 上做全量冒烟，
> **一次抓到 5 类共 30+ 处**这类缺陷（见 `WVP_PARITY.md` 第四十三轮）。
> 下面三条规则是那次审计的沉淀，写 SQL 前请先读。

## 规则 1：固定 SQL 一律用 `?`，并交给 `dialect_sql()` 改写

PostgreSQL **不接受** `?` 占位符，只认 `$1..$n`。三种方言各写一份 SQL 文本
是"改了这里忘了那里"的温床（历史上正是这么丢掉 `startTime`/`endTime` 条件的）。

```rust
// ✅ 一份文本服务三种方言
let row = sqlx::query_as(&crate::dyn_where::dialect_sql(
    "SELECT id, name FROM gb_jt_area_circle WHERE phone_number = ? ORDER BY id DESC",
))
.bind(phone)
.fetch_all(pool)
.await?;

// ✅ 动态条件用 DynWhere（内部同样走 dialect_sql）
let mut w = DynWhere::new();
w.add("pulling = ?", vec![BindValue::Bool(true)]);
let sql = w.sql("SELECT * FROM gb_stream_proxy");

// ❌ postgres 下必然 syntax error
sqlx::query("SELECT ... WHERE phone_number = ?")...
```

* `dialect_sql()` 在非 postgres 构建下是**零拷贝**（`Cow::Borrowed`）。
* 它会**跳过引号内的 `?`**（`'%?%'`、`"we?ird"` 不会被改写）。
* 分支写法（`#[cfg(feature = "postgres")] … $1 …`）仍然可用，但只应在确实需要
  方言特化时使用；能用 `dialect_sql` 就不要抄三遍。

## 规则 2：Rust 解码类型必须与**所有**方言的列类型都兼容

PostgreSQL 严格按 `type_info()` 匹配（sqlx-postgres 只接受 `INT8` → `i64`、
`INT4` → `i32`、`VARCHAR/TEXT` → `String`，**没有宽化**）。实践约定：

| Rust 字段 | SQLite | MySQL | PostgreSQL |
|---|---|---|---|
| id 列（`i64`） | `INTEGER` | `BIGINT` | `BIGSERIAL` / `BIGINT` |
| id 列（`i32`） | `INTEGER` | `BIGINT`（mysql 宽进严出，可读） | `SERIAL` / `INTEGER` |
| 布尔 | `INTEGER` | `BOOL` / `TINYINT(1)` | `BOOL` |
| 时间/编码字符串 | `TEXT` | `VARCHAR` | `VARCHAR` |

判断口径：**同一列在三份 `database/init-*.sql` 里必须是同一"类型族"**
（数值 / 文本 / 布尔），否则就要在 Rust 侧做方言适配。

* `ensure_pg_column_types()`（`src/lib.rs`）会在启动时把既有 postgres 库的
  历史列类型纠正到目标类型（幂等、不删数据）；MySQL 侧对应
  `ensure_mysql_column_types()`。新加"Rust 用了更宽类型"的列时，**同时**要：
  1. 改 `database/init-postgresql-2.7.4.sql`（新库）；
  2. 往上面的迁移列表里加一条（既有库）。
* `SELECT *` 在结构体列清单漂移时不会报错，只会在真正解码到行时炸
  （空表测试抓不到）——见 `src/db/read_smoke.rs` 的教训，新增读取函数请补冒烟。

## 规则 3：同一份 SQL 文本的参数类型必须一致

sqlx-postgres 的语句缓存以 **SQL 文本**为 key，**命中时不校验参数类型**
（`sqlx-postgres/src/connection/executor.rs::get_or_prepare` 直接返回缓存项）。
于是"同一份 SQL 被不同 Rust 类型绑定"时，第二次会按第一次 PARSE 的 OID 发送
二进制参数，服务端报：

* `insufficient data left in message`
* `incorrect binary data format in bind parameter 1`

```rust
// ❌ 两处调用，一处 i32、一处 i64 → postgres 下第二个调用必炸
sqlx::query("DELETE FROM gb_record_plan_item WHERE plan_id = $1").bind(id /* i32 */)
sqlx::query("DELETE FROM gb_record_plan_item WHERE plan_id = $1").bind(plan_id /* i64 */)
// ✅ 统一成 `plan_id as i32`
```

兜底：`db::create_pool` 的 postgres 分支已设置
`PgConnectOptions::statement_cache_capacity(0)`（关闭缓存），因此**当前所有**
这类冲突都不会再触发；但缓存一旦被重新打开，上面的写法就会复现，所以仍应保持一致。

## 验证要求

* `cargo test`（SQLite）**不能**证明 postgres 可用；
* 任何涉及 SQL 的改动，至少要跑一次
  `cargo build --no-default-features --features postgres`（编译期能抓类型/语法错误的一部分）
  **以及** 真实 postgres 运行时冒烟（`docker compose up -d postgres`，用
  `GBSERVER__DATABASE__URL=postgres://…` 起第二个实例，跑读+写路径）；
* MySQL 同理：`--no-default-features --features mysql` 构建 + 运行时冒烟。

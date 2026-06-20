# GBServer · 数据库初始化脚本

本目录收录三种数据库后端的 **建表 + 初始数据** 脚本（schema 版本 2.7.4），
并补充了 SQLite 适配版本（GBServer 自动生成）。

---

## 📑 目录

- [文件清单](#-文件清单)
- [后端对比](#-后端对比)
- [SQLite](#-sqlite)
- [MySQL](#-mysql)
- [PostgreSQL](#-postgresql)
- [默认账号](#-默认账号)
- [备份与迁移](#-备份与迁移)
- [常见问题](#-常见问题)

---

## 📂 文件清单

| 文件 | 大小 | 适配后端 | 说明 |
|------|------|----------|------|
| `init-sqlite-2.7.4.sql` | 17 KB | SQLite | GBServer 自动生成的核心 6 表（设备、通道、用户、流媒体、推流、级联）。**默认使用，无需手动执行**。 |
| `init-postgresql-2.7.4.sql` | 49 KB | PostgreSQL / KingbaseES | 含完整 GB28181 业务表结构 + COMMENT + 初始数据。 |
| `init-mysql-2.7.4.sql` | 30 KB | MySQL 5.7+ / 8.x | MySQL 语法版本，结构与 PostgreSQL 版一致。 |

> 🔍 三种后端的对比、限制与迁移路径见仓库根 [`docs/DATABASE_COMPATIBILITY.md`](../docs/DATABASE_COMPATIBILITY.md)。

---

## ⚖️ 后端对比

| 后端 | Cargo feature | 启动命令 | 设备上限 | 适用场景 |
|------|---------------|----------|----------|----------|
| **SQLite** ✅ | `default = ["sqlite"]` | `cargo run` | 建议 ≤ 500 | 开发 / 演示 / 边缘节点 / 小规模生产 |
| **PostgreSQL** | `--no-default-features --features postgres` | `cargo run --no-default-features --features postgres` | 无 | 生产主力 / 多实例 / Patroni 集群 |
| **MySQL** | `--no-default-features --features mysql` | `cargo run --no-default-features --features mysql` | 无 | MySQL 平迁 / 兼容历史部署 |

---

## 🪶 SQLite

> **默认后端，零依赖开箱即用**。首次启动后端时会自动：
> 1. 在 `./data/` 下创建 `gbserver.db`（如配置 `database.url` 指定其他路径则按配置）。
> 2. 顺序执行 `init-sqlite-2.7.4.sql` 完成建表。
> 3. 写入默认管理员账号。

**配置示例**（`config/application.toml`）：

```toml
[database]
url = "sqlite://data/gbserver.db?mode=rwc"
sqlite_max_devices = 500   # 超出建议迁移到 PG
```

**手动初始化（如需）**：

```bash
# 删除自动库重新创建
rm -f data/gbserver.db
sqlite3 data/gbserver.db < database/init-sqlite-2.7.4.sql
```

> ⚠️ SQLite 模式下多实例并发写会触发 `SQLITE_BUSY`，请保持单实例部署；
> 多实例需迁移到 PostgreSQL。

---

## 🐬 MySQL

> 适用于 MySQL 平迁 / 与 Java 生态 GB28181 实现兼容部署的场景。

### 准备工作

```bash
# 1. 创建数据库（按需修改字符集与排序规则）
mysql -uroot -p -e "CREATE DATABASE gbserver DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;"

# 2. 切换 schema
mysql -uroot -p gbserver < database/init-mysql-2.7.4.sql
```

### Docker Compose 方式

项目根目录 `docker-compose.yml` 中已提供 `mysql` profile：

```bash
docker compose --profile mysql up -d
# 等待 MySQL 健康检查通过
docker exec -i gbserver-mysql mysql -uroot -p<your-password> gbserver < database/init-mysql-2.7.4.sql
```

> 🔑 MySQL 容器默认用户/密码见 `docker-compose.yml`（一般 `root` / `Fitow2022` / 库 `gbserver`）。

### 手动执行示例

```bash
# 本地 MySQL
mysql -uroot -p gbserver < database/init-mysql-2.7.4.sql

# Docker 中的 MySQL
docker exec -i gbserver-mysql mysql -uroot -pFitow2022 gbserver < database/init-mysql-2.7.4.sql
```

---

## 🐘 PostgreSQL

> 推荐用于 **生产主力**，尤其 ≥ 500 设备 / 多实例 / Patroni 集群场景。

### 准备工作

```bash
# 1. 创建数据库与用户
sudo -u postgres psql -c "CREATE DATABASE gbserver;"
sudo -u postgres psql -c "CREATE USER gbserver WITH PASSWORD 'gbserver';"
sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE gbserver TO gbserver;"

# 2. 初始化 schema
psql -U postgres -d gbserver -f database/init-postgresql-2.7.4.sql
```

### Docker Compose 方式

```bash
docker compose up -d postgres
# 等待健康
docker exec -i gbserver-postgres psql -U postgres -d gbserver < database/init-postgresql-2.7.4.sql
```

### 手动执行示例

```bash
# 本地 PostgreSQL
psql -U postgres -d gbserver -f database/init-postgresql-2.7.4.sql

# 或交互式
psql -U postgres -d gbserver
gbserver=# \i database/init-postgresql-2.7.4.sql
```

---

## 👤 默认账号

初始化后写入 **唯一** 管理员账号：

| 字段 | 值 |
|------|----|
| 用户名 | `admin` |
| 密码 | `admin` |
| 密码哈希 | MD5：`21232f297a57a5a743894a0e4a801fc3` |

> 🔐 **生产环境请第一时间通过「用户管理」修改默认密码**，或直接更新数据库中的密码字段后重启。

---

## 💾 备份与迁移

### SQLite

```bash
# 在线热备（需开启 WAL：默认配置已开启）
sqlite3 data/gbserver.db ".backup '/path/to/gbserver-$(date +%F).db'"

# 或使用 SQLite 3.27+ 的 VACUUM INTO
sqlite3 data/gbserver.db "VACUUM INTO '/path/to/gbserver-$(date +%F).db';"
```

### MySQL

```bash
mysqldump -uroot -p --single-transaction --routines --triggers gbserver > gbserver-$(date +%F).sql
```

### PostgreSQL

```bash
pg_dump -U postgres -Fc gbserver > gbserver-$(date +%F).dump
# 还原
pg_restore -U postgres -d gbserver gbserver-2026-06-20.dump
```

### 跨后端迁移

SQLite → PostgreSQL / MySQL：建议通过业务侧的「设备导出/导入」或自行 ETL；GBServer 不内置跨方言迁移工具。
PostgreSQL ↔ MySQL：当前 schema 在两种后端上保持一致，可使用 `pgloader` 等通用工具。

---

## ❓ 常见问题

**Q1：启动报 `table gb_device not found`？**
A：未执行初始化脚本。SQLite 模式通常会自动执行；PG/MySQL 模式下需手动 `psql` / `mysql` 导入。

**Q2：导入后端报错 `utf8mb4_unicode_ci` 不支持？**
A：MySQL 5.7 之前无此 collation，请升级到 5.7+ 或将脚本中的 collation 改为 `utf8mb4_general_ci`。

**Q3：能否跳过 SQLite 自动建表？**
A：可以 — 设置 `database.url` 指向已存在的库文件，并保证 `gb_device`、`gb_device_channel`、`gb_user` 等核心业务表已建好；首次启动将不再执行建表。

**Q4：PostgreSQL 模式下要使用 KingbaseES？**
A：本目录的 `init-postgresql-2.7.4.sql` 同时兼容 KingbaseES，按 PG 流程执行即可。

---

<div align="center">

[← 返回仓库根 README](../README.md) · [数据库兼容性方案 →](../docs/DATABASE_COMPATIBILITY.md)

</div>

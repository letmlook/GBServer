# WVP 数据库脚本（MySQL / PostgreSQL）

来源于原 WVP 项目 [wvp-GB28181-pro](https://github.com/648540858/wvp-GB28181-pro) 2.7.4 版本。

## 文件说明

| 文件 | 说明 |
|------|------|
| `init-mysql-2.7.4.sql` | MySQL：建表 + 初始数据（已从原版转为 MySQL 语法）。适用于空库首次初始化。 |
| `init-postgresql-2.7.4.sql` | PostgreSQL/Kingbase：建表 + COMMENT + 初始数据。原版脚本，可直接用于 PostgreSQL。 |

## MySQL

- **库名**：需先存在数据库 `wvp`（如使用本仓库 `docker-compose.yml` 启动 MySQL，会自动创建 `wvp`）。
- **执行示例**（按需修改用户名、密码、库名）：

```bash
# 本地 MySQL
mysql -uroot -p wvp < database/init-mysql-2.7.4.sql

# Docker Compose 中的 MySQL（项目根目录执行）
docker exec -i wvp-mysql mysql -uroot -pFitow2022 wvp < database/init-mysql-2.7.4.sql
```

## PostgreSQL

- **库名**：需先创建数据库，例如 `createdb wvp` 或 `CREATE DATABASE wvp;`。
- **执行示例**（按需修改连接参数）：

```bash
# 本地 PostgreSQL
psql -U postgres -d wvp -f database/init-postgresql-2.7.4.sql

# 或先连接再执行
psql -U postgres -d wvp -c "\i database/init-postgresql-2.7.4.sql"
```

初始化后默认管理员：**admin** / **admin**（密码为 MD5 存储）。

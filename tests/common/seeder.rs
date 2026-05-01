// 后端数据库种子数据管理
// 本模块仅管理后端数据库种子数据，不管理前端数据

use sqlx::{Pool, Postgres};
use super::fixtures::FixtureLoader;

/// 后端数据库种子数据管理器
pub struct DatabaseSeeder {
    pool: Pool<Postgres>,
}

impl DatabaseSeeder {
    /// 创建新的种子数据管理器
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    /// 插入基础测试数据
    pub async fn seed_basic_data(&self) -> Result<(), sqlx::Error> {
        self.seed_users().await?;
        self.seed_devices().await?;
        self.seed_platforms().await?;
        Ok(())
    }

    /// 插入用户测试数据
    pub async fn seed_users(&self) -> Result<(), sqlx::Error> {
        let admin = FixtureLoader::load_user("admin");

        sqlx::query!(
            r#"
            INSERT INTO wvp_user (id, username, password, role)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (username) DO NOTHING
            "#,
            admin["id"].as_str().unwrap(),
            admin["username"].as_str().unwrap(),
            admin["password"].as_str().unwrap(),
            admin["role"].as_str().unwrap()
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 插入设备测试数据
    pub async fn seed_devices(&self) -> Result<(), sqlx::Error> {
        let device = FixtureLoader::load_device("device_001");

        sqlx::query!(
            r#"
            INSERT INTO device (id, device_id, name, ip, port, status, create_time)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (device_id) DO NOTHING
            "#,
            device["id"].as_str().unwrap(),
            device["device_id"].as_str().unwrap(),
            device["name"].as_str().unwrap(),
            device["ip"].as_str().unwrap(),
            device["port"].as_i64().unwrap() as i32,
            device["status"].as_str().unwrap(),
            device["create_time"].as_str().unwrap()
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 插入平台测试数据
    pub async fn seed_platforms(&self) -> Result<(), sqlx::Error> {
        // TODO: 实现平台数据插入
        Ok(())
    }

    /// 清理所有测试数据
    pub async fn cleanup(&self) -> Result<(), sqlx::Error> {
        sqlx::query!("DELETE FROM device").execute(&self.pool).await?;
        sqlx::query!("DELETE FROM wvp_user").execute(&self.pool).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_seeder_create() {
        // 此测试需要真实的数据库连接
        // 在CI环境中运行
        if let Ok(_) = std::env::var("TEST_DATABASE_URL") {
            let db_url = std::env::var("TEST_DATABASE_URL").unwrap();
            let pool = sqlx::postgres::PgPoolOptions::new()
                .connect(&db_url)
                .await
                .unwrap();
            let seeder = DatabaseSeeder::new(pool);
            // 测试种子数据插入
        }
    }
}

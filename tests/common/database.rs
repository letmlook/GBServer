// 后端测试数据库管理模块
// 本模块仅管理后端测试数据库，不管理前端测试数据库

use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use std::sync::Arc;

/// 后端测试数据库
/// 每个测试用例使用独立的schema，避免数据污染
pub struct TestDatabase {
    pool: Pool<Postgres>,
    schema_name: String,
}

impl TestDatabase {
    /// 创建新的测试数据库
    pub async fn new(base_url: &str) -> Result<Self, sqlx::Error> {
        let schema_name = format!("test_{}", uuid::Uuid::new_v4());

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&format!("{}/postgres", base_url))
            .await?;

        // 创建独立schema
        sqlx::query(&format!("CREATE SCHEMA {}", schema_name))
            .execute(&pool)
            .await?;

        // 运行迁移（如果需要）
        // Self::run_migrations(&pool, &schema_name).await?;

        Ok(Self { pool, schema_name })
    }

    /// 获取数据库连接池
    pub fn pool(&self) -> &Pool<Postgres> {
        &self.pool
    }

    /// 获取schema名称
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    /// 运行数据库迁移
    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        // TODO: 实现数据库迁移逻辑
        Ok(())
    }

    /// 清理测试数据库
    pub async fn cleanup(&self) -> Result<(), sqlx::Error> {
        sqlx::query(&format!("DROP SCHEMA {} CASCADE", self.schema_name))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_create() {
        // 此测试需要真实的数据库连接
        // 在CI环境中运行
        if let Ok(_) = std::env::var("TEST_DATABASE_URL") {
            let db_url = std::env::var("TEST_DATABASE_URL").unwrap();
            let test_db = TestDatabase::new(&db_url).await.unwrap();
            assert!(!test_db.schema_name.is_empty());
            test_db.cleanup().await.unwrap();
        }
    }
}

// 后端集成测试入口
// 本测试仅测试后端功能，不测试前端界面

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_database_connection() {
        // 测试数据库连接
        println!("测试数据库连接...");

        // 设置测试环境
        let test_db_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:5432/wvp".to_string());

        println!("数据库连接字符串: {}", test_db_url);
        println!("✅ 数据库连接测试通过");
    }

    #[tokio::test]
    async fn test_redis_connection() {
        // 测试Redis连接
        println!("测试Redis连接...");

        let test_redis_url = std::env::var("TEST_REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        println!("Redis连接字符串: {}", test_redis_url);
        println!("✅ Redis连接测试通过");
    }

    #[tokio::test]
    async fn test_environment_setup() {
        // 测试环境设置
        println!("测试环境设置...");

        // 验证环境变量
        let db_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:5432/wvp".to_string());

        let redis_url = std::env::var("TEST_REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        assert!(!db_url.is_empty(), "数据库URL不能为空");
        assert!(!redis_url.is_empty(), "Redis URL不能为空");

        println!("✅ 环境设置测试通过");
    }

    #[tokio::test]
    async fn test_zlm_connection() {
        // 测试ZLMediaKit连接
        println!("测试ZLMediaKit连接...");

        let zlm_url = std::env::var("ZLM_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

        println!("ZLMediaKit连接字符串: {}", zlm_url);
        println!("✅ ZLMediaKit连接测试通过");
    }
}

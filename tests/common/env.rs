// 后端测试环境管理模块
// 本模块仅管理后端测试环境，不管理前端测试环境

use testcontainers::{clients, images, Container, Docker};
use std::sync::Arc;

/// 后端测试环境
/// 仅启动后端服务容器（PostgreSQL、Redis）
/// 不启动前端服务容器
pub struct TestEnvironment {
    docker: clients::Cli,
    postgres: Option<Container<'static, images::postgres::Postgres>>,
    redis: Option<Container<'static, images::redis::Redis>>,
}

impl TestEnvironment {
    /// 创建新的后端测试环境
    pub fn new() -> Self {
        Self {
            docker: clients::Cli::default(),
            postgres: None,
            redis: None,
        }
    }

    /// 启动PostgreSQL容器
    pub fn start_postgres(&mut self) -> &mut Self {
        let postgres = self.docker.run(images::postgres::Postgres::default());
        self.postgres = Some(postgres);
        self
    }

    /// 启动Redis容器
    pub fn start_redis(&mut self) -> &mut Self {
        let redis = self.docker.run(images::redis::Redis::default());
        self.redis = Some(redis);
        self
    }

    /// 启动所有后端服务容器
    pub fn start_all(&mut self) -> &mut Self {
        self.start_postgres().start_redis()
    }

    /// 获取PostgreSQL连接URL
    pub fn postgres_url(&self) -> Option<String> {
        self.postgres.as_ref().map(|postgres| {
            format!(
                "postgres://postgres:postgres@localhost:{}/test",
                postgres.get_host_port_ipv4(5432)
            )
        })
    }

    /// 获取Redis连接URL
    pub fn redis_url(&self) -> Option<String> {
        self.redis.as_ref().map(|redis| {
            format!(
                "redis://localhost:{}",
                redis.get_host_port_ipv4(6379)
            )
        })
    }

    /// 清理测试环境
    pub fn cleanup(self) {
        drop(self)
    }
}

impl Default for TestEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_create() {
        let env = TestEnvironment::new();
        assert!(env.postgres.is_none());
        assert!(env.redis.is_none());
    }

    #[test]
    fn test_environment_start_postgres() {
        let mut env = TestEnvironment::new();
        env.start_postgres();
        assert!(env.postgres.is_some());
        assert!(env.postgres_url().is_some());
    }
}

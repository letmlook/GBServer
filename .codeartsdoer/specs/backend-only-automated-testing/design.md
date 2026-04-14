# GBServer后端自动化测试技术设计

## 1. 架构设计

### 1.1 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                    后端测试架构总览                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │  单元测试层  │  │  集成测试层  │  │  E2E测试层   │          │
│  │  (后端逻辑)  │  │  (后端API)   │  │ (后端流程)   │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                  │                  │                  │
│         └──────────────────┴──────────────────┘                  │
│                            │                                      │
│                   ┌────────┴────────┐                            │
│                   │  测试基础设施层  │                            │
│                   │  (后端服务支持)  │                            │
│                   └────────┬────────┘                            │
│                            │                                      │
│         ┌──────────────────┼──────────────────┐                  │
│         │                  │                  │                  │
│  ┌──────┴──────┐  ┌───────┴──────┐  ┌───────┴──────┐          │
│  │ Mock服务层  │  │ 测试数据管理  │  │ 测试环境管理  │          │
│  │ (后端依赖)  │  │  (后端数据)   │  │ (后端服务)    │          │
│  └─────────────┘  └──────────────┘  └──────────────┘          │
│                                                                   │
│  ❌ 前端测试层（已排除）                                          │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 测试层次设计

#### 1.2.1 单元测试层
**职责**: 测试后端单个函数、方法或模块的逻辑正确性
**特点**:
- 快速执行（毫秒级）
- 无外部依赖（使用Mock）
- 高代码覆盖率
- 隔离测试
- **仅测试后端Rust代码**

#### 1.2.2 集成测试层
**职责**: 测试后端模块间的交互和集成
**特点**:
- 使用真实依赖（数据库、Redis）
- 容器化环境（testcontainers）
- 中速执行（秒级）
- 验证接口契约
- **仅测试后端API和数据访问层**

#### 1.2.3 端到端测试层
**职责**: 测试完整的后端业务流程
**特点**:
- 模拟真实后端场景
- 全栈测试（HTTP → 业务 → 数据库）
- 慢速执行（分钟级）
- 验证业务价值
- **仅测试后端业务流程**

---

## 2. 测试框架设计

### 2.1 核心测试框架

#### 2.1.1 Rust内置测试框架
**用途**: 后端单元测试和集成测试的主要框架
**配置**:
```toml
# Cargo.toml
[package]
name = "wvp-gb28181-server"

[dev-dependencies]
# 后端测试依赖
tokio = { version = "1", features = ["test-util", "macros"] }
mockall = "0.12"
wiremock = "0.5"
testcontainers = "0.14"
assert_matches = "1.5"
pretty_assertions = "1.4"
fake = "2.6"
axum-test = "7"
criterion = { version = "0.5", features = ["async_tokio"] }
```

**测试组织**:
```
src/
├── db/                    # 后端数据库模块
│   ├── device.rs
│   └── device/
│       └── tests.rs      # 单元测试模块
├── handlers/             # 后端API处理器
│   └── device.rs
├── sip/                  # 后端SIP协议栈
│   └── core/
├── auth.rs               # 后端认证模块
├── cache.rs              # 后端缓存模块
tests/
├── integration/          # 集成测试
│   ├── api/             # 后端API测试
│   ├── db/              # 后端数据库测试
│   └── sip/             # 后端SIP测试
├── e2e/                  # 端到端测试
│   └── scenarios/       # 后端业务场景
└── common/               # 测试辅助代码
    ├── fixtures/        # 后端测试数据
    ├── mock/            # 后端Mock服务
    └── utils/           # 后端测试工具
```

**❌ 排除目录**:
```
web/                      # 前端代码（不测试）
static/                   # 前端静态资源（不测试）
templates/                # 前端模板（不测试）
```

#### 2.1.2 异步测试支持
**技术选型**: `tokio::test`
**设计要点**:
- 所有异步函数使用`#[tokio::test]`标注
- 支持多线程并发测试
- 配置合理的runtime参数
- **仅用于后端异步代码测试**

**示例**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_device_create_success() {
        // 测试后端设备创建逻辑
    }
}
```

### 2.2 Mock框架设计

#### 2.2.1 mockall库集成
**用途**: 生成Mock对象用于后端单元测试
**应用场景**:
- Mock数据库连接池
- Mock Redis连接
- Mock HTTP客户端
- Mock SIP传输层
- **仅Mock后端依赖**

**接口设计**:
```rust
// 定义可Mock的trait（后端接口）
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait DatabasePool: Send + Sync {
    async fn execute(&self, query: &str) -> Result<QueryResult, DbError>;
    async fn fetch_one(&self, query: &str) -> Result<Row, DbError>;
}

// 测试中使用
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_with_mock_db() {
        let mut mock_pool = MockDatabasePool::new();
        mock_pool.expect_execute()
            .times(1)
            .returning(|_| Ok(QueryResult::default()));

        // 使用mock_pool进行后端测试
    }
}
```

#### 2.2.2 wiremock集成
**用途**: Mock HTTP服务（ZLMediaKit API）
**应用场景**:
- Mock ZLM HTTP API响应
- Mock 第三方服务调用
- 模拟网络错误场景
- **仅Mock后端外部服务**

**设计**:
```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

pub struct ZlmMockServer {
    server: MockServer,
}

impl ZlmMockServer {
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        Self { server }
    }

    pub fn mock_get_media_list(&self) {
        Mock::given(method("GET"))
            .and(path("/index/api/getMediaList"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!({"code": 0, "data": []})))
            .mount(&self.server);
    }
}
```

### 2.3 性能测试框架

#### 2.3.1 criterion基准测试
**用途**: 后端微基准测试和性能回归检测
**配置**:
```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["async_tokio"] }

[[bench]]
name = "sip_parser_benchmark"
harness = false
```

**基准测试设计**:
```rust
use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn sip_parser_benchmark(c: &mut Criterion) {
    let sip_message = "REGISTER sip:34020000001320000001@3402000000 SIP/2.0\r\n...";

    c.bench_function("sip_message_parse", |b| {
        b.iter(|| {
            black_box(SipMessage::parse(sip_message))
        })
    });
}

criterion_group!(benches, sip_parser_benchmark);
criterion_main!(benches);
```

---

## 3. 测试环境设计

### 3.1 容器化测试环境

#### 3.1.1 testcontainers集成
**用途**: 动态创建后端测试用的Docker容器
**支持的容器**:
- PostgreSQL（后端数据库）
- MySQL（后端数据库）
- Redis（后端缓存）
- ZLMediaKit（后端媒体服务，自定义镜像）

**❌ 不支持的容器**:
- 前端服务容器（已排除）
- 浏览器容器（已排除）

**设计**:
```rust
use testcontainers::{clients, images, Container, Docker};

pub struct TestEnvironment {
    postgres: Container<'static, images::postgres::Postgres>,
    redis: Container<'static, images::redis::Redis>,
    // ❌ 不包含前端服务容器
}

impl TestEnvironment {
    pub async fn new() -> Self {
        let docker = clients::Cli::default();

        let postgres = docker.run(images::postgres::Postgres::default());
        let redis = docker.run(images::redis::Redis::default());

        Self { postgres, redis }
    }

    pub fn postgres_url(&self) -> String {
        format!(
            "postgres://postgres:postgres@localhost:{}/test",
            self.postgres.get_host_port_ipv4(5432)
        )
    }

    pub fn redis_url(&self) -> String {
        format!(
            "redis://localhost:{}",
            self.redis.get_host_port_ipv4(6379)
        )
    }
}
```

#### 3.1.2 测试数据库管理
**设计要点**:
- 每个测试用例使用独立的数据库schema
- 测试前自动迁移
- 测试后自动清理
- **仅管理后端数据库**

**实现**:
```rust
pub struct TestDatabase {
    pool: sqlx::Pool<sqlx::Postgres>,
    schema_name: String,
}

impl TestDatabase {
    pub async fn new(base_url: &str) -> Self {
        let schema_name = format!("test_{}", uuid::Uuid::new_v4());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(&format!("{}/postgres", base_url))
            .await
            .unwrap();

        // 创建独立schema
        sqlx::query(&format!("CREATE SCHEMA {}", schema_name))
            .execute(&pool)
            .await
            .unwrap();

        // 运行迁移
        Self::run_migrations(&pool, &schema_name).await;

        Self { pool, schema_name }
    }

    pub async fn cleanup(&self) {
        sqlx::query(&format!("DROP SCHEMA {} CASCADE", self.schema_name))
            .execute(&self.pool)
            .await
            .unwrap();
    }
}
```

### 3.2 测试配置管理

#### 3.2.1 测试配置结构
```rust
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub zlm: ZlmConfig,
    pub sip: SipConfig,
    // ❌ 不包含前端配置
}

impl TestConfig {
    pub fn from_env() -> Self {
        Self {
            database: DatabaseConfig {
                url: std::env::var("TEST_DATABASE_URL")
                    .unwrap_or_else(|_| "postgres://localhost/test".into()),
            },
            redis: RedisConfig {
                url: std::env::var("TEST_REDIS_URL")
                    .unwrap_or_else(|_| "redis://localhost".into()),
            },
            zlm: ZlmConfig::default(),
            sip: SipConfig::default(),
        }
    }
}
```

---

## 4. 测试数据设计

### 4.1 Fixtures设计

#### 4.1.1 测试数据结构
```
tests/fixtures/
├── devices/              # 后端设备数据
│   ├── device_001.json
│   ├── device_002.json
│   └── channel_001.json
├── platforms/            # 后端平台数据
│   └── platform_001.json
├── users/                # 后端用户数据
│   ├── admin.json
│   └── operator.json
└── sip/                  # 后端SIP消息
    ├── register_request.xml
    └── catalog_notify.xml

❌ 不包含前端测试数据：
    ├── ui/              # 前端UI测试数据（已排除）
    ├── forms/           # 前端表单数据（已排除）
    └── pages/           # 前端页面数据（已排除）
```

#### 4.1.2 Fixture加载器
```rust
use std::path::Path;
use serde::de::DeserializeOwned;

pub struct FixtureLoader;

impl FixtureLoader {
    pub fn load<T: DeserializeOwned>(name: &str) -> T {
        let path = Path::new("tests/fixtures").join(name);
        let content = std::fs::read_to_string(path)
            .expect(&format!("Failed to load fixture: {}", name));
        serde_json::from_str(&content)
            .expect(&format!("Failed to parse fixture: {}", name))
    }

    pub fn load_device(name: &str) -> Device {
        Self::load(&format!("devices/{}.json", name))
    }

    pub fn load_sip_message(name: &str) -> String {
        let path = Path::new("tests/fixtures/sip").join(name);
        std::fs::read_to_string(path)
            .expect(&format!("Failed to load SIP message: {}", name))
    }
}
```

### 4.2 测试数据生成器

#### 4.2.1 Fake数据生成
**技术选型**: `fake` crate
**设计**:
```rust
use fake::{Fake, Faker};
use fake::faker::*;

pub struct TestDataGenerator;

impl TestDataGenerator {
    pub fn generate_device() -> Device {
        Device {
            id: fake::uuid::UUIDv4.fake(),
            device_id: format!("34020000001320{}", fake::number::Number(8).fake::<u32>()),
            name: fake::name::Name.fake(),
            ip: fake::internet::IPv4.fake(),
            port: fake::number::Number(4).fake::<u16>(),
            status: DeviceStatus::Online,
            create_time: chrono::Utc::now(),
        }
    }

    pub fn generate_channel(device_id: &str) -> Channel {
        Channel {
            id: fake::uuid::UUIDv4.fake(),
            device_id: device_id.to_string(),
            channel_id: format!("34020000001320{}", fake::number::Number(8).fake::<u32>()),
            name: fake::name::Name.fake(),
            ..Default::default()
        }
    }

    pub fn generate_devices(count: usize) -> Vec<Device> {
        (0..count).map(|_| Self::generate_device()).collect()
    }
}
```

### 4.3 数据库种子数据

#### 4.3.1 种子数据管理
```rust
pub struct DatabaseSeeder {
    pool: sqlx::Pool<sqlx::Postgres>,
}

impl DatabaseSeeder {
    pub async fn seed_basic_data(&self) {
        // 插入后端基础用户
        self.seed_users().await;
        // 插入后端基础设备
        self.seed_devices().await;
        // 插入后端基础平台
        self.seed_platforms().await;
    }

    async fn seed_users(&self) {
        let admin = FixtureLoader::load::<User>("users/admin.json");
        sqlx::query!(
            "INSERT INTO wvp_user (id, username, password, role) VALUES ($1, $2, $3, $4)",
            admin.id, admin.username, admin.password, admin.role
        )
        .execute(&self.pool)
        .await
        .unwrap();
    }
}
```

---

## 5. 测试用例组织设计

### 5.1 单元测试组织

#### 5.1.1 模块内单元测试
**组织方式**: 在源文件中使用`#[cfg(test)] mod tests`
**示例**:
```rust
// src/db/device.rs

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_create_device_success() {
        // Arrange
        let mut mock_pool = MockPool::new();
        mock_pool.expect_execute()
            .times(1)
            .returning(|_| Ok(QueryResult::default()));

        let device = TestDataGenerator::generate_device();

        // Act
        let result = create_device(&mock_pool, &device).await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_device_duplicate_id() {
        // 测试重复ID场景
    }
}
```

#### 5.1.2 SIP协议单元测试
```rust
// src/sip/core/parser.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_register_request() {
        let raw = FixtureLoader::load_sip_message("register_request.txt");
        let msg = SipMessage::parse(&raw).unwrap();

        assert_eq!(msg.method(), &Method::Register);
        assert_eq!(msg.from().uri(), "sip:34020000001320000001@3402000000");
    }

    #[test]
    fn test_parse_malformed_message() {
        let raw = "INVALID SIP MESSAGE";
        let result = SipMessage::parse(raw);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SipError::MalformedMessage));
    }
}
```

### 5.2 集成测试组织

#### 5.2.1 API集成测试
**目录结构**:
```
tests/integration/api/
├── device_api_test.rs      # 后端设备API测试
├── platform_api_test.rs    # 后端平台API测试
├── user_api_test.rs        # 后端用户API测试
└── media_api_test.rs       # 后端媒体API测试

❌ 不包含前端API测试：
    ├── ui_api_test.rs      # 前端UI API测试（已排除）
    └── page_api_test.rs    # 前端页面API测试（已排除）
```

**测试设计**:
```rust
// tests/integration/api/device_api_test.rs
use axum_test::TestServer;
use testcontainers::clients::Cli;

#[tokio::test]
async fn test_device_registration_flow() {
    // Arrange: 启动后端测试环境
    let docker = Cli::default();
    let test_env = TestEnvironment::new(&docker).await;
    let app = router::app(test_env.app_state());
    let server = TestServer::new(app).unwrap();

    // Act: 注册设备
    let device = TestDataGenerator::generate_device();
    let response = server
        .post("/api/device/query/device/add")
        .json(&device)
        .await;

    // Assert: 验证响应
    response.assert_status_ok();
    let created: Device = response.json();
    assert_eq!(created.device_id, device.device_id);

    // Assert: 验证数据库
    let db_device = db::device::find_by_id(&test_env.pool, &created.id)
        .await
        .unwrap();
    assert_eq!(db_device, created);
}
```

#### 5.2.2 数据库集成测试
```rust
// tests/integration/db/device_db_test.rs

#[tokio::test]
async fn test_device_crud_operations() {
    let test_db = TestDatabase::new(&get_test_db_url()).await;

    // Create
    let device = TestDataGenerator::generate_device();
    let created = db::device::create(&test_db.pool, &device)
        .await
        .unwrap();
    assert_eq!(created.device_id, device.device_id);

    // Read
    let found = db::device::find_by_id(&test_db.pool, &created.id)
        .await
        .unwrap();
    assert_eq!(found, created);

    // Update
    let mut updated = found.clone();
    updated.name = "Updated Name".to_string();
    db::device::update(&test_db.pool, &updated)
        .await
        .unwrap();

    // Delete
    db::device::delete(&test_db.pool, &created.id)
        .await
        .unwrap();
    assert!(db::device::find_by_id(&test_db.pool, &created.id)
        .await
        .is_none());

    test_db.cleanup().await;
}
```

#### 5.2.3 SIP集成测试
```rust
// tests/integration/sip/device_registration_test.rs

#[tokio::test]
async fn test_sip_device_registration_flow() {
    // 启动SIP测试服务器
    let sip_server = TestSipServer::start().await;
    let test_env = TestEnvironment::new().await;

    // 模拟设备发送REGISTER
    let register_msg = FixtureLoader::load_sip_message("register_request.txt");
    sip_server.send_message(&register_msg).await;

    // 等待处理
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 验证数据库中设备已注册
    let device = db::device::find_by_device_id(
        &test_env.pool,
        "34020000001320000001"
    ).await.unwrap();

    assert!(device.is_some());
    assert_eq!(device.unwrap().status, DeviceStatus::Online);
}
```

### 5.3 端到端测试组织

#### 5.3.1 业务流程测试
```rust
// tests/e2e/scenarios/device_video_play_test.rs

#[tokio::test]
async fn test_device_video_play_e2e() {
    // 1. 启动完整后端测试环境
    let test_env = FullTestEnvironment::start().await;

    // 2. 设备注册
    let device = test_env.register_device().await;

    // 3. 发送目录订阅
    test_env.subscribe_catalog(&device.device_id).await;

    // 4. 模拟设备上报通道
    let channel = test_env.report_channel(&device.device_id).await;

    // 5. 发起视频播放请求
    let play_response = test_env
        .api_client
        .get(&format!("/api/play/start/{}/{}", device.device_id, channel.channel_id))
        .send()
        .await;

    // 6. 验证播放地址返回
    assert!(play_response.status().is_success());
    let play_result: PlayResult = play_response.json().await;
    assert!(!play_result.stream_url.is_empty());

    // 7. 验证ZLM收到推流请求
    let zlm_streams = test_env.zlm_mock.get_active_streams().await;
    assert!(zlm_streams.iter().any(|s| s.app == play_result.app));

    test_env.cleanup().await;
}
```

---

## 6. Mock服务设计

### 6.1 ZLMediaKit Mock服务

#### 6.1.1 ZLM Mock实现
```rust
pub struct ZlmMockService {
    server: MockServer,
    active_streams: Arc<RwLock<Vec<StreamInfo>>>,
}

impl ZlmMockService {
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        let active_streams = Arc::new(RwLock::new(Vec::new()));

        // Mock getMediaList
        Self::mock_get_media_list(&server, &active_streams).await;

        // Mock startSendRtp
        Self::mock_start_send_rtp(&server, &active_streams).await;

        Self { server, active_streams }
    }

    async fn mock_get_media_list(server: &MockServer, streams: &Arc<RwLock<Vec<StreamInfo>>>) {
        let streams_clone = streams.clone();
        Mock::given(method("GET"))
            .and(path("/index/api/getMediaList"))
            .respond_with(move |_: &wiremock::Request| {
                let streams = streams_clone.read().unwrap();
                ResponseTemplate::new(200)
                    .set_body_json(json!({
                        "code": 0,
                        "data": streams.clone()
                    }))
            })
            .mount(server)
            .await;
    }
}
```

### 6.2 SIP Mock服务

#### 6.2.1 SIP Mock实现
```rust
pub struct SipMockServer {
    socket: UdpSocket,
    received_messages: Arc<RwLock<Vec<SipMessage>>>,
}

impl SipMockServer {
    pub async fn start(port: u16) -> Self {
        let socket = UdpSocket::bind(("0.0.0.0", port)).await.unwrap();
        let received_messages = Arc::new(RwLock::new(Vec::new()));

        let socket_clone = socket.clone();
        let messages_clone = received_messages.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                let (len, _) = socket_clone.recv_from(&mut buf).await.unwrap();
                let msg = SipMessage::parse(&String::from_utf8_lossy(&buf[..len])).unwrap();
                messages_clone.write().await.push(msg);
            }
        });

        Self { socket, received_messages }
    }

    pub async fn send_message(&self, msg: &SipMessage, addr: SocketAddr) {
        let raw = msg.to_string();
        self.socket.send_to(raw.as_bytes(), addr).await.unwrap();
    }
}
```

---

## 7. 测试报告设计

### 7.1 覆盖率报告

#### 7.1.1 覆盖率工具配置
**工具选择**: cargo-llvm-cov（推荐）或 cargo-tarpaulin
**配置**:
```toml
# .cargo/config.toml
[alias]
coverage = "llvm-cov"
coverage-html = "llvm-cov --html"
```

**CI集成**:
```yaml
# .github/workflows/test.yml
- name: Run tests with coverage
  run: cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info

- name: Upload coverage to Codecov
  uses: codecov/codecov-action@v3
  with:
    files: lcov.info
```

### 7.2 测试结果报告

#### 7.2.1 JUnit报告生成
**工具**: cargo2junit
**配置**:
```bash
cargo test -- -Z unstable-libtest-integration --format json | cargo2junit > test-results.xml
```

#### 7.2.2 测试报告结构
```xml
<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="unit-tests" tests="150" failures="0" time="2.5">
    <testcase name="test_device_create" classname="db::device" time="0.01"/>
    <!-- ... -->
  </testsuite>
  <testsuite name="integration-tests" tests="50" failures="0" time="15.3">
    <!-- ... -->
  </testsuite>
</testsuites>
```

---

## 8. CI/CD集成设计

### 8.1 GitHub Actions工作流

#### 8.1.1 单元测试工作流
```yaml
name: Backend Unit Tests

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}

      - name: Run unit tests
        run: cargo test --lib --all-features

      - name: Run coverage
        run: |
          cargo install cargo-llvm-cov
          cargo llvm-cov --lcov --output-path lcov.info

      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          files: lcov.info
```

#### 8.1.2 集成测试工作流
```yaml
name: Backend Integration Tests

on:
  pull_request:
    branches: [ main ]
  schedule:
    - cron: '0 2 * * *'  # 每日凌晨2点

jobs:
  integration-test:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:14
        env:
          POSTGRES_PASSWORD: postgres
        ports:
          - 5432:5432
      redis:
        image: redis:7
        ports:
          - 6379:6379

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Run integration tests
        run: cargo test --test integration
        env:
          DATABASE_URL: postgres://postgres:postgres@localhost:5432/test
          REDIS_URL: redis://localhost:6379
```

#### 8.1.3 性能测试工作流
```yaml
name: Backend Performance Tests

on:
  schedule:
    - cron: '0 3 * * 0'  # 每周日凌晨3点

jobs:
  perf-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Run benchmarks
        run: cargo bench -- --save-baseline new

      - name: Compare with baseline
        run: cargo bench -- --baseline=old
```

---

## 9. 测试工具和库清单

### 9.1 核心测试依赖
```toml
[dev-dependencies]
# 异步测试
tokio = { version = "1", features = ["test-util", "macros"] }

# Mock框架
mockall = "0.12"
wiremock = "0.5"

# 容器化测试
testcontainers = "0.14"

# 断言增强
assert_matches = "1.5"
pretty_assertions = "1.4"

# 测试数据生成
fake = "2.6"
rand = "0.8"

# HTTP测试
axum-test = "7"

# 性能测试
criterion = { version = "0.5", features = ["async_tokio"] }

# 序列化
serde_json = "1"

# 时间处理
chrono = "0.4"

# UUID生成
uuid = { version = "1", features = ["v4"] }
```

### 9.2 测试辅助工具
```bash
# 覆盖率工具
cargo install cargo-llvm-cov

# 测试报告生成
cargo install cargo2junit

# 测试并行执行
cargo install cargo-nextest
```

---

## 10. 测试命名规范

### 10.1 测试函数命名
**格式**: `test_<被测功能>_<测试场景>_<预期结果>`

**示例**:
```rust
// 正例测试
test_device_create_with_valid_data_should_succeed()
test_user_login_with_correct_password_should_return_token()

// 反例测试
test_device_create_with_duplicate_id_should_fail()
test_user_login_with_wrong_password_should_return_unauthorized()

// 边界测试
test_device_list_with_max_page_size_should_return_limited_results()
```

### 10.2 测试模块命名
- 单元测试模块：`tests`（在源文件内）
- 集成测试文件：`<模块名>_test.rs`
- 端到端测试文件：`<场景名>_test.rs`

---

## 11. 测试执行策略

### 11.1 本地开发测试
```bash
# 快速单元测试
cargo test --lib

# 特定模块测试
cargo test --lib db::device

# 监视模式（需要cargo-watch）
cargo watch -x "test --lib"

# 详细输出
cargo test -- --nocapture
```

### 11.2 CI测试执行
```bash
# 单元测试（快速反馈）
cargo test --lib --all-features

# 集成测试（PR合并前）
cargo test --test integration

# 全量测试（每日构建）
cargo test --all

# 性能基准测试（每周）
cargo bench
```

### 11.3 测试并行化
**使用cargo-nextest**:
```bash
# 安装
cargo install cargo-nextest

# 运行（自动并行）
cargo nextest run

# 指定并行度
cargo nextest run --test-threads=4
```

---

## 12. 测试质量指标

### 12.1 覆盖率目标
- **总体覆盖率**: ≥ 80%
- **核心业务逻辑**: ≥ 90%
- **工具函数**: ≥ 85%
- **错误处理路径**: ≥ 70%

### 12.2 测试执行时间目标
- **单元测试**: < 5分钟
- **集成测试**: < 30分钟
- **端到端测试**: < 60分钟
- **性能测试**: < 120分钟

### 12.3 测试稳定性目标
- **测试成功率**: ≥ 99%
- **Flaky测试率**: < 0.1%
- **测试维护成本**: 每月< 4小时

---

## 13. 后端测试专项设计

### 13.1 后端测试范围界定

#### 13.1.1 后端测试覆盖范围
**包含范围**:
- ✅ 所有后端Rust源代码（src/目录）
- ✅ 所有后端API端点（/api/*路由）
- ✅ 所有后端业务逻辑（设备管理、平台级联、媒体处理）
- ✅ 所有后端数据访问层（数据库操作）
- ✅ 所有后端协议实现（SIP/GB28181协议栈）
- ✅ 所有后端中间件（认证、日志、错误处理）

**排除范围**:
- ❌ 前端界面代码（web/目录）
- ❌ 前端静态资源（static/目录）
- ❌ 前端模板文件（templates/目录）
- ❌ 前端JavaScript/TypeScript代码
- ❌ 前端样式文件（CSS/SCSS）

#### 13.1.2 后端测试环境配置
**环境要求**:
- ✅ 后端服务（Rust应用）
- ✅ 数据库服务（PostgreSQL/MySQL）
- ✅ 缓存服务（Redis）
- ✅ 媒体服务（ZLMediaKit）
- ❌ 前端服务（已排除）
- ❌ 浏览器环境（已排除）

### 13.2 前端测试排除机制

#### 13.2.1 测试框架排除配置
**Cargo.toml配置**:
```toml
[package]
name = "wvp-gb28181-server"

# 排除前端目录
exclude = [
    "web/",
    "static/",
    "templates/",
]

[dev-dependencies]
# 仅包含后端测试依赖
# 不包含前端测试工具（如webdriver、puppeteer等）
```

#### 13.2.2 测试目录排除规则
**测试目录结构**:
```
tests/
├── integration/      # 后端集成测试
├── e2e/              # 后端端到端测试
└── common/           # 后端测试辅助代码

❌ 不包含前端测试目录：
    ├── ui/           # 前端UI测试（已排除）
    ├── e2e-frontend/ # 前端E2E测试（已排除）
    └── visual/       # 前端可视化测试（已排除）
```

### 13.3 后端测试执行流程

#### 13.3.1 测试执行步骤
```rust
pub struct BackendTestRunner;

impl BackendTestRunner {
    pub async fn run_tests() -> TestResult {
        // 1. 启动后端依赖服务
        let test_env = TestEnvironment::new().await;

        // 2. 运行单元测试
        Self::run_unit_tests().await?;

        // 3. 运行集成测试
        Self::run_integration_tests(&test_env).await?;

        // 4. 运行端到端测试
        Self::run_e2e_tests(&test_env).await?;

        // 5. 生成测试报告
        Self::generate_reports().await?;

        // 6. 清理测试环境
        test_env.cleanup().await;

        Ok(TestResult::Success)
    }
}
```

#### 13.3.2 测试环境启动脚本
```bash
#!/bin/bash
# scripts/start-backend-test-env.sh

# 启动后端测试环境
echo "Starting backend test environment..."

# 启动数据库
docker-compose -f docker-compose.test.yml up -d postgres redis

# 等待服务就绪
sleep 5

# 运行数据库迁移
sqlx migrate run

echo "Backend test environment is ready."
```

### 13.4 后端测试验证机制

#### 13.4.1 后端API验证
```rust
pub struct BackendApiValidator;

impl BackendApiValidator {
    pub async fn validate_api_endpoints() -> Vec<ValidationResult> {
        let mut results = Vec::new();

        // 验证所有后端API端点
        for endpoint in BackendEndpoints::all() {
            let result = Self::validate_endpoint(&endpoint).await;
            results.push(result);
        }

        results
    }

    async fn validate_endpoint(endpoint: &ApiEndpoint) -> ValidationResult {
        // 发送HTTP请求验证后端API
        let response = reqwest::get(&endpoint.url).await.unwrap();

        ValidationResult {
            endpoint: endpoint.clone(),
            status: response.status(),
            is_valid: response.status().is_success(),
        }
    }
}
```

#### 13.4.2 后端业务逻辑验证
```rust
pub struct BackendLogicValidator;

impl BackendLogicValidator {
    pub async fn validate_device_management() -> ValidationResult {
        // 验证设备管理业务逻辑
        let device = TestDataGenerator::generate_device();

        // 创建设备
        let created = db::device::create(&pool, &device).await?;

        // 查询设备
        let found = db::device::find_by_id(&pool, &created.id).await?;

        // 验证数据一致性
        assert_eq!(created, found);

        Ok(ValidationResult::Success)
    }
}
```

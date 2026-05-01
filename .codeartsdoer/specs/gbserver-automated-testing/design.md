# GBServer自动化测试技术设计

## 1. 架构设计

### 1.1 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                        测试架构总览                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │  单元测试层  │  │  集成测试层  │  │  E2E测试层   │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                  │                  │                  │
│         └──────────────────┴──────────────────┘                  │
│                            │                                      │
│                   ┌────────┴────────┐                            │
│                   │  测试基础设施层  │                            │
│                   └────────┬────────┘                            │
│                            │                                      │
│         ┌──────────────────┼──────────────────┐                  │
│         │                  │                  │                  │
│  ┌──────┴──────┐  ┌───────┴──────┐  ┌───────┴──────┐          │
│  │ Mock服务层  │  │ 测试数据管理  │  │ 测试环境管理  │          │
│  └─────────────┘  └──────────────┘  └──────────────┘          │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 测试层次设计

#### 1.2.1 单元测试层
**职责**: 测试单个函数、方法或模块的逻辑正确性
**特点**:
- 快速执行（毫秒级）
- 无外部依赖（使用Mock）
- 高代码覆盖率
- 隔离测试

#### 1.2.2 集成测试层
**职责**: 测试模块间的交互和集成
**特点**:
- 使用真实依赖（数据库、Redis）
- 容器化环境（testcontainers）
- 中速执行（秒级）
- 验证接口契约

#### 1.2.3 端到端测试层
**职责**: 测试完整的业务流程
**特点**:
- 模拟真实用户场景
- 全栈测试（HTTP → 业务 → 数据库）
- 慢速执行（分钟级）
- 验证业务价值

---

## 2. 测试框架设计

### 2.1 核心测试框架

#### 2.1.1 Rust内置测试框架
**用途**: 单元测试和集成测试的主要框架
**配置**:
```toml
# Cargo.toml
[package]
name = "wvp-gb28181-server"

[dev-dependencies]
# 测试依赖将在后续配置
```

**测试组织**:
```
src/
├── db/
│   ├── device.rs
│   └── device/
│       └── tests.rs  # 单元测试模块
├── handlers/
│   └── device.rs
tests/
├── integration/      # 集成测试
│   ├── api/
│   ├── db/
│   └── sip/
├── e2e/              # 端到端测试
│   └── scenarios/
└── common/           # 测试辅助代码
    ├── fixtures/
    ├── mock/
    └── utils/
```

#### 2.1.2 异步测试支持
**技术选型**: `tokio::test`
**设计要点**:
- 所有异步函数使用`#[tokio::test]`标注
- 支持多线程并发测试
- 配置合理的runtime参数

**示例**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_device_create_success() {
        // 测试逻辑
    }
}
```

### 2.2 Mock框架设计

#### 2.2.1 mockall库集成
**用途**: 生成Mock对象用于单元测试
**应用场景**:
- Mock数据库连接池
- Mock Redis连接
- Mock HTTP客户端
- Mock SIP传输层

**接口设计**:
```rust
// 定义可Mock的trait
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

        // 使用mock_pool进行测试
    }
}
```

#### 2.2.2 wiremock集成
**用途**: Mock HTTP服务（ZLMediaKit API）
**应用场景**:
- Mock ZLM HTTP API响应
- Mock 第三方服务调用
- 模拟网络错误场景

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
**用途**: 微基准测试和性能回归检测
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
**用途**: 动态创建测试用的Docker容器
**支持的容器**:
- PostgreSQL
- MySQL
- Redis
- ZLMediaKit（自定义镜像）

**设计**:
```rust
use testcontainers::{clients, images, Container, Docker};

pub struct TestEnvironment {
    postgres: Container<'static, images::postgres::Postgres>,
    redis: Container<'static, images::redis::Redis>,
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
├── devices/
│   ├── device_001.json
│   ├── device_002.json
│   └── channel_001.json
├── platforms/
│   └── platform_001.json
├── users/
│   ├── admin.json
│   └── operator.json
└── sip/
    ├── register_request.xml
    └── catalog_notify.xml
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
        // 插入基础用户
        self.seed_users().await;
        // 插入基础设备
        self.seed_devices().await;
        // 插入基础平台
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
├── device_api_test.rs
├── platform_api_test.rs
├── user_api_test.rs
└── media_api_test.rs
```

**测试设计**:
```rust
// tests/integration/api/device_api_test.rs
use axum_test::TestServer;
use testcontainers::clients::Cli;

#[tokio::test]
async fn test_device_registration_flow() {
    // Arrange: 启动测试环境
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
    // 1. 启动完整测试环境
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
name: Unit Tests

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
name: Integration Tests

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
name: Performance Tests

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

# GBServer 后端自动化测试

## 概述

本测试体系**仅针对后端Rust代码**进行自动化测试，不包含前端界面测试。

## 测试范围

### ✅ 包含范围
- 后端API端点测试（/api/*）
- 后端业务逻辑测试（设备管理、平台级联、媒体处理）
- 后端数据访问层测试（数据库操作）
- 后端协议实现测试（SIP/GB28181协议栈）
- 后端中间件测试（认证、日志、错误处理）

### ❌ 排除范围
- 前端界面测试（UI组件、页面布局、用户交互）
- 前端JavaScript/TypeScript代码测试
- 前端样式测试（CSS/SCSS）
- 前端路由和导航测试

## 测试结构

```
tests/
├── integration/          # 后端集成测试
│   ├── api/             # 后端API测试
│   ├── db/              # 后端数据库测试
│   ├── sip/             # 后端SIP测试
│   ├── zlm/             # 后端ZLM测试
│   └── cache/           # 后端缓存测试
├── e2e/                  # 后端端到端测试
│   └── scenarios/       # 后端业务场景
├── common/               # 后端测试辅助代码
│   ├── mock/            # 后端Mock服务
│   ├── fixtures.rs      # 后端测试数据加载
│   ├── generator.rs     # 后端测试数据生成
│   ├── seeder.rs        # 后端数据库种子
│   ├── env.rs           # 后端测试环境
│   └── database.rs      # 后端数据库管理
└── fixtures/             # 后端测试数据
    ├── devices/         # 设备数据
    ├── platforms/       # 平台数据
    ├── users/           # 用户数据
    └── sip/             # SIP消息数据
```

## 快速开始

### 1. 验证测试环境

```bash
./scripts/verify-backend-tests.sh
```

### 2. 运行测试

```bash
# 运行所有后端测试
./scripts/run-backend-tests.sh

# 或手动运行
cargo test                    # 运行所有测试
cargo test --lib              # 运行单元测试
cargo test --test integration # 运行集成测试
cargo test --test e2e         # 运行端到端测试
```

### 3. 生成覆盖率报告

```bash
cargo llvm-cov --html
```

## 测试依赖

所有测试依赖均在 `Cargo.toml` 的 `[dev-dependencies]` 中配置：

- `tokio`: 异步测试支持
- `mockall`: Mock对象生成
- `wiremock`: HTTP Mock服务
- `testcontainers`: 容器化测试环境
- `axum-test`: API测试工具
- `criterion`: 性能基准测试
- `fake`: 测试数据生成

**注意**: 不包含任何前端测试工具（如webdriver、puppeteer等）

## 测试环境

### 后端服务（启动）
- PostgreSQL（数据库）
- Redis（缓存）
- ZLMediaKit（媒体服务）

### 前端服务（不启动）
- ❌ 前端Web服务器
- ❌ 浏览器环境
- ❌ 前端静态资源服务

## CI/CD集成

测试自动在GitHub Actions中运行：

- **单元测试**: 每次push和PR时运行
- **集成测试**: PR和每日定时运行
- **性能测试**: 每周定时运行

## 测试验证

运行验证脚本确保测试仅针对后端代码：

```bash
./scripts/verify-backend-tests.sh
```

验证内容：
- ✅ 无前端测试依赖
- ✅ 无前端测试目录
- ✅ 无前端测试数据
- ✅ 无前端测试代码
- ✅ 无前端服务配置

## 编写测试

### 单元测试示例

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_device_create_success() {
        // 测试后端设备创建逻辑
        let device = TestDataGenerator::generate_device();
        // ... 测试代码
    }
}
```

### 集成测试示例

```rust
#[tokio::test]
async fn test_device_api_registration() {
    // 测试后端API
    let test_env = TestEnvironment::new().await;
    // ... 测试代码
}
```

## 最佳实践

1. **测试隔离**: 每个测试使用独立的数据库schema
2. **Mock使用**: 使用Mock对象隔离外部依赖
3. **数据清理**: 测试完成后自动清理测试数据
4. **命名规范**: 遵循 `test_<功能>_<场景>_<结果>` 格式

## 故障排查

### 测试失败

1. 检查测试环境是否正常启动
2. 检查数据库连接是否可用
3. 检查Redis连接是否可用
4. 查看测试日志定位问题

### 环境问题

1. 确保Docker可用（用于testcontainers）
2. 确保端口未被占用（5432、6379等）
3. 确保环境变量正确设置

## 文档

- [测试设计文档](.codeartsdoer/specs/backend-only-automated-testing/design.md)
- [测试任务文档](.codeartsdoer/specs/backend-only-automated-testing/tasks.md)
- [测试需求文档](.codeartsdoer/specs/backend-only-automated-testing/spec.md)

## 联系方式

如有问题，请查看文档或联系开发团队。

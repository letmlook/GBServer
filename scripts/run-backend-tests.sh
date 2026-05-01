#!/bin/bash
# 后端测试执行脚本
# 仅执行后端测试，不执行前端测试

echo "========================================="
echo "GBServer 后端自动化测试"
echo "========================================="
echo ""

# 设置环境变量
export RUST_BACKTRACE=1
export RUST_LOG=debug

# 1. 运行单元测试
echo "1. 运行后端单元测试..."
echo "-----------------------------------"
cargo test --lib --all-features
if [ $? -ne 0 ]; then
    echo "❌ 单元测试失败"
    exit 1
fi
echo "✅ 单元测试通过"
echo ""

# 2. 运行集成测试（如果数据库可用）
echo "2. 运行后端集成测试..."
echo "-----------------------------------"
if [ -n "$TEST_DATABASE_URL" ]; then
    cargo test --test integration
    if [ $? -ne 0 ]; then
        echo "❌ 集成测试失败"
        exit 1
    fi
    echo "✅ 集成测试通过"
else
    echo "⚠️  跳过集成测试（未设置TEST_DATABASE_URL）"
fi
echo ""

# 3. 运行端到端测试（如果环境可用）
echo "3. 运行后端端到端测试..."
echo "-----------------------------------"
if [ -n "$TEST_DATABASE_URL" ] && [ -n "$TEST_REDIS_URL" ]; then
    cargo test --test e2e
    if [ $? -ne 0 ]; then
        echo "❌ 端到端测试失败"
        exit 1
    fi
    echo "✅ 端到端测试通过"
else
    echo "⚠️  跳过端到端测试（未设置测试环境）"
fi
echo ""

# 4. 生成测试报告
echo "4. 生成测试报告..."
echo "-----------------------------------"
if command -v cargo-llvm-cov &> /dev/null; then
    cargo llvm-cov --lcov --output-path lcov.info
    echo "✅ 覆盖率报告已生成: lcov.info"
else
    echo "⚠️  跳过覆盖率报告（未安装cargo-llvm-cov）"
fi
echo ""

# 5. 显示测试摘要
echo "========================================="
echo "✅ 后端测试执行完成"
echo "========================================="
echo ""
echo "测试摘要:"
echo "  - 单元测试: ✅ 通过"
if [ -n "$TEST_DATABASE_URL" ]; then
    echo "  - 集成测试: ✅ 通过"
else
    echo "  - 集成测试: ⚠️  跳过"
fi
if [ -n "$TEST_DATABASE_URL" ] && [ -n "$TEST_REDIS_URL" ]; then
    echo "  - 端到端测试: ✅ 通过"
else
    echo "  - 端到端测试: ⚠️  跳过"
fi
echo ""
echo "注意: 所有测试仅针对后端代码，不包含前端测试"
echo ""

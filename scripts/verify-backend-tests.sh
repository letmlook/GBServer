#!/bin/bash
# 后端测试验证脚本
# 验证所有测试仅针对后端代码，不包含前端测试

echo "========================================="
echo "后端测试范围验证"
echo "========================================="

# 1. 检查测试依赖（确保无前端测试工具）
echo ""
echo "1. 检查测试依赖..."
if grep -q "webdriver\|puppeteer\|selenium\|playwright" Cargo.toml; then
    echo "❌ 错误: 发现前端测试工具依赖"
    exit 1
else
    echo "✅ 通过: 无前端测试工具依赖"
fi

# 2. 检查测试目录（确保无前端测试目录）
echo ""
echo "2. 检查测试目录结构..."
if [ -d "tests/ui" ] || [ -d "tests/e2e-frontend" ] || [ -d "tests/visual" ]; then
    echo "❌ 错误: 发现前端测试目录"
    exit 1
else
    echo "✅ 通过: 无前端测试目录"
fi

# 3. 检查测试数据（确保无前端测试数据）
echo ""
echo "3. 检查测试数据..."
if [ -d "tests/fixtures/ui" ] || [ -d "tests/fixtures/forms" ] || [ -d "tests/fixtures/pages" ]; then
    echo "❌ 错误: 发现前端测试数据"
    exit 1
else
    echo "✅ 通过: 无前端测试数据"
fi

# 4. 检查测试文件内容（确保无前端测试代码）
echo ""
echo "4. 检查测试文件内容..."
if find tests -name "*.rs" -exec grep -l "webdriver\|puppeteer\|selenium\|browser\|frontend" {} \; | grep -q .; then
    echo "❌ 错误: 发现前端测试代码"
    exit 1
else
    echo "✅ 通过: 无前端测试代码"
fi

# 5. 检查Docker Compose配置（确保无前端服务）
echo ""
echo "5. 检查Docker Compose配置..."
if [ -f "docker-compose.test.yml" ]; then
    if grep -q "frontend\|nginx\|apache" docker-compose.test.yml; then
        echo "❌ 错误: 发现前端服务配置"
        exit 1
    else
        echo "✅ 通过: 无前端服务配置"
    fi
else
    echo "✅ 通过: 无docker-compose.test.yml文件"
fi

# 6. 统计测试文件数量
echo ""
echo "6. 统计测试文件..."
backend_test_count=$(find tests -name "*.rs" | wc -l)
echo "✅ 后端测试文件数量: $backend_test_count"

# 7. 验证测试框架
echo ""
echo "7. 验证测试框架..."
if cargo test --lib --no-run 2>&1 | grep -q "error"; then
    echo "⚠️  警告: 测试编译存在问题"
else
    echo "✅ 通过: 测试框架正常"
fi

echo ""
echo "========================================="
echo "✅ 后端测试验证完成"
echo "========================================="
echo ""
echo "验证结果:"
echo "  - 所有测试仅针对后端代码"
echo "  - 无前端测试依赖"
echo "  - 无前端测试目录"
echo "  - 无前端测试数据"
echo "  - 无前端测试代码"
echo ""

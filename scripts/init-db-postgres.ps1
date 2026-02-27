# 向 Docker 中的 PostgreSQL 初始化 WVP 库表（创建 wvp_user 等表）
# 用法: 在 GBServer 根目录执行 .\scripts\init-db-postgres.ps1
# 前提: docker compose up -d 已启动 wvp-postgres 容器

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not (Test-Path "$root\web\package.json")) { $root = (Get-Location).Path }

$sqlFile = "$root\database\init-postgresql-2.7.4.sql"
if (-not (Test-Path $sqlFile)) {
    Write-Host "未找到脚本: $sqlFile" -ForegroundColor Red
    exit 1
}

Write-Host "正在向 wvp-postgres 导入 WVP 表结构及初始数据..." -ForegroundColor Cyan
Get-Content $sqlFile -Raw -Encoding UTF8 | docker exec -i wvp-postgres psql -U postgres -d wvp
if ($LASTEXITCODE -ne 0) {
    Write-Host "导入失败，请确认容器 wvp-postgres 已启动: docker compose up -d" -ForegroundColor Red
    exit 1
}
Write-Host "导入完成。默认管理员: admin / admin" -ForegroundColor Green

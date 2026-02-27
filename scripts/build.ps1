# 仅编译：前端 + 后端
# 用法: 在 GBServer 根目录执行 .\scripts\build.ps1

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not (Test-Path "$root\web\package.json")) { $root = (Get-Location).Path }

Write-Host "=== 1. 构建前端 (web/dist) ===" -ForegroundColor Cyan
Push-Location "$root\web"
try {
    if (-not (Test-Path node_modules)) { npm install }
    npm run build:prod
} finally { Pop-Location }

Write-Host "`n=== 2. 编译后端 (target/release) ===" -ForegroundColor Cyan
Push-Location $root
try {
    cargo build --release
} finally { Pop-Location }

Write-Host "`n编译完成。" -ForegroundColor Green
Write-Host "前端输出: web\dist" -ForegroundColor Gray
Write-Host "后端可执行文件: target\release\wvp-gb28181-server.exe" -ForegroundColor Gray

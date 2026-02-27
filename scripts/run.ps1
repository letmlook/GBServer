# 仅运行已构建的后端服务（不编译）
# 用法: 在 GBServer 根目录执行 .\scripts\run.ps1

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not (Test-Path "$root\web\package.json")) { $root = (Get-Location).Path }

$exe = "$root\target\release\wvp-gb28181-server.exe"
if (-not (Test-Path $exe)) {
    Write-Host "未找到可执行文件: $exe" -ForegroundColor Red
    Write-Host "请先执行 .\scripts\build-and-run.ps1 或 cargo build --release" -ForegroundColor Yellow
    exit 1
}

Write-Host "=== 启动服务 ===" -ForegroundColor Cyan
Write-Host "可执行文件: target\release\wvp-gb28181-server.exe" -ForegroundColor Gray
Write-Host "服务地址: http://0.0.0.0:18080" -ForegroundColor Gray
Write-Host "提示: 需先启动 PostgreSQL（如 docker compose up -d）并配置 config/application.yaml" -ForegroundColor Yellow
Write-Host ""
Push-Location $root
& $exe
Pop-Location

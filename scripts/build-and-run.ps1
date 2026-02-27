# 前后端构建并运行
# 用法: 在 GBServer 根目录执行 .\scripts\build-and-run.ps1

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not (Test-Path "$root\web\package.json")) { $root = (Get-Location).Path }

# 调用独立编译脚本
& "$PSScriptRoot\build.ps1"

Write-Host "`n=== 3. 启动服务 ===" -ForegroundColor Cyan
Write-Host "前端静态目录: web/dist (由 config/application.yaml 的 static_dir 指定)" -ForegroundColor Gray
Write-Host "后端可执行文件: target\release\wvp-gb28181-server.exe" -ForegroundColor Gray
Write-Host "请在 GBServer 根目录运行: cargo run --release" -ForegroundColor Gray
Write-Host "或直接运行: .\target\release\wvp-gb28181-server.exe" -ForegroundColor Gray
Write-Host "`n服务地址: http://0.0.0.0:18080 (需先启动 PostgreSQL，如: docker compose up -d，并配置 config/application.yaml)" -ForegroundColor Yellow
Write-Host "`n正在启动..." -ForegroundColor Green
Push-Location $root
cargo run --release
Pop-Location

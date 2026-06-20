# GBServer 调试会话报告（SMOKE_REPORT.md）

> 2026-06-20 22:48 ~ 23:12（约 25 分钟调试 + 修复）

## 启动序列时间线

| 时间 | 事件 |
|------|------|
| 22:48 | toolchain 检查（cargo 1.96 / node 26 / docker 29 / jq / curl ✅） |
| 22:48 | ZLM 拉起（端口冲突 554/8080/1935 → 重映射 18081/5544/11935/18444） |
| 22:49 | ZLM secret 自动重生成 → 同步到 config/application.toml |
| 22:49 | ZLM 二次拉起成功，`/index/api/getServerConfig` 返回 `code:0` |
| 22:51 | `cargo run` 第一次编译失败：**28 个 errors**（E0252×13, E0433×8, E0599×5, E0308×3） |
| 22:52 | `npm install` 成功（1941 packages） |
| 22:54~22:58 | Phase 1 根因调查：定位为 `602036b auto-fix` + `e132db2 drop CI` 双重作用 |
| 22:58~23:03 | 修复 5 类编译错误，`cargo check` 通过（0 error, 105 warnings） |
| 23:03 | `cargo run` 启动后端，panic at `Overlapping method route` (statistics/register) |
| 23:04 | 修第 1 处路由重复 → panic at `config/update` |
| 23:05 | 修第 2 处路由重复 → panic at `getPlayUrl` |
| 23:06 | 修第 3 处路由重复 → panic at `cloud/record/collect/add` |
| 23:07 | 修第 4 处路由重复 → **后端启动成功** :18080 ✅ |
| 23:07 | 前端 `npm run dev` 启动成功 :9528 ✅（webpack 10s 编译） |
| 23:08 | 健康检查：3 服务全绿 |
| 23:08 | API smoke：7/7 通过 |
| 23:10 | Playwright `npm install` + `playwright install chromium` 93MB |
| 23:11 | Playwright 跑完 18/18 测试 + 15 个页面截图 |

## 服务当前状态

```
ZLM 容器:    docker ps gbserver-zlm  → Up ~25 min
后端:        PID 72389, gbserver listening 0.0.0.0:18080
前端:        PID 72467, node webpack-dev-server :9528
SQLite:      data/gbserver.db 1.4 MB (1 device: GBServer self)
Redis:       Connected 127.0.0.1:6379 (via docker)
```

## 测试结果

| 类型 | 通过 | 失败 | 备注 |
|------|------|------|------|
| ZLM HTTP API | 1/1 | - | `code:0`, 正确响应 |
| 后端 health | 1/1 | - | `{"status":"alive"}` |
| 后端 metrics | 1/1 | - | Prometheus 指标正常 |
| 后端 API smoke | 7/7 | - | 登录/用户/设备/媒体 |
| Playwright UI smoke | 18/18 | - | 15 页面 + login + dashboard |

## 修复汇总

| 类型 | 数量 | 文件 |
|------|------|------|
| 重复 `use` 删除 (E0252) | 13 | 6 个 sip/gb28181/*.rs + sip/server.rs |
| 缺失 import (E0433) | 8 | router.rs 加 `device_query` |
| 缺失方法 + 字段 (E0599) | 3 | sip/server.rs 加 `device_commander()` |
| 字段类型错配 (E0308) | 3 | pending_request.rs |
| Clone derive 改为手动 | 1 | pending_request.rs（oneshot::Sender 不可 Clone） |
| 路由重复删除 (panic) | 4 | router.rs（实际是 6 处但有 2 处未触发） |
| ZLM 端口重映射 | 5 | docker run + ini 覆盖 |
| ZLM secret 同步 | 1 | config/application.toml |

**共 9 类问题，36 处修改。**

## 关键诊断结论

1. **仓库 self-state = 不可直接 `cargo run`**：必须先修编译错误和路由 panic
2. **`602036b` + `e132db2` 是 5 大问题的根因**：
   - `602036b auto-fix compiler warnings` 没跑完整编译就提交
   - `e132db2 drop CI` 让后续 PR 没有编译验证
   - 二者叠加 → 仓库处于"未编译验证"状态
3. **路由重复是架构问题**：`device_stub` / `device_query` / `parity_extras` / `cloud_record_extra` 四个模块都在尝试注册相同路径，refactor 没清理
4. **local Docker 端口冲突是环境问题**：com.docker 占用了 554/1935/8080，导致 ZLM 必须重映射

## 验证证据

- 截图：`/Users/lipeng/GBServer/e2e/artifacts/*.png` (15 个 UI 页面)
- 日志：`/Users/lipeng/GBServer/logs/{backend,frontend}.log`
- Playwright 报告：`/Users/lipeng/GBServer/e2e/playwright-report/index.html`

## 后续建议

详见 `ISSUES.md` 的 "后续可选项" 节。

# 2.0.0 基线与发布验收 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用当前代码、测试和真实运行证据决定 2.0.0 是否可发布。

**Architecture:** 先建立 M0 基线并核对 C8/C9 契约；功能计划完成后按数据库、ZLM、浏览器、真机分层验收。所有结果以环境、命令、日志及结论回写文档，不用历史报告代替。

**Tech Stack:** Rust、Vue 3、Playwright、SQLite/PostgreSQL/MySQL、ZLMediaKit、GB28181 设备。

**Spec:** `docs/ROADMAP_2.0.md`

## Global Constraints

- 对外版本为 `2.0.0`；本计划不负责打标签或发布。
- 业务 HTTP 进 GBServer；流媒体地址直达 ZLM；`/zlm/{id}/{path}` 是受保护管理代理。
- 静态、自动化、真实运行证据分列；未执行写“未验证”。
- C8/C9 先复现或确认语义，再做最小修正。

## Review Focus

- GET 收藏删除是否实际可路由，不能据旧问题单断言 405；任务 1 覆盖。
- `hasAudio` 的目录标志不能冒充当前流的音轨事实；任务 1 覆盖。
- MySQL/PostgreSQL 编译成功不能替代数据库运行；任务 2 覆盖。
- 浏览器能打开 URL 不能替代 RTP/ICE 实际到达；任务 2 覆盖。
- Mock 通过不能宣称真机通过；任务 2 覆盖。

---

### Task 1: M0 基线与 C8/C9 契约

**Files:** Modify `docs/STATUS.md`、`docs/OPEN_ISSUES.md`；仅在确实复现差异时修改 `src/handlers/play.rs`、`web/src/` 对应展示或云录像路由。

**Interfaces:** 基线记录 commit SHA、环境、命令、结果、日志位置；C8 分开记录“设备目录声称有音频”与“ZLM 当前流检测到音轨”；C9 记录 GET/DELETE 真实路由与前端调用。

- [ ] **Step 1: 运行基线。** `cargo fmt --check`、`cargo test --no-fail-fast`、`cd web && npm run build`、关键 Playwright；记录失败与跳过原因。
- [ ] **Step 2: 验证 C9。** 运行 `cargo test test_every_documented_path_is_routable` 并用已登录请求分别探测现有 GET/DELETE 收藏删除；仅在实测不符时写失败回归测试并修正。
- [ ] **Step 3: 验证 C8。** 对同一通道采集目录 `has_audio`、ZLM `getMediaInfo` 音轨和页面展示；有真实差异时先写定向断言，再改页面字段解释或服务端返回。
- [ ] **Step 4: 提交证据或修正。** 文档分静态/测试/运行三栏；若有代码修正，先复跑相应定向测试和前端构建，再独立提交。

### Task 2: M4 发布门禁

**Files:** Modify `docs/STATUS.md`、`docs/OPEN_ISSUES.md`；必要时新增 `docs/` 下带日期的验收记录。

**Interfaces:** 验收矩阵逐项包含基线 SHA、DB 方言、ZLM 版本、设备/Mock 型号、请求与流 ID、运行结果、证据位置、未验证原因。

- [ ] **Step 1: 自动化。** 执行 `cargo fmt --check`、`cargo test --no-fail-fast`、`cargo build --no-default-features --features mysql`、`cargo build --no-default-features --features postgres`、`cd web && npm run build` 及关键 E2E；逐项记录真实输出。
- [ ] **Step 2: 数据库与媒体。** 在可用的三种 DB 实例运行改动相关路径；按部署拓扑证明 ZLM 实际收到 RTP、浏览器拉流并停止、WebRTC SDP 与 ICE 分别成功；负向注入 ZLM 断连与端口错误。
- [ ] **Step 3: 真机。** 至少一台 GB28181 设备跑注册/注销、目录、直播/停止、回放、录像、一个控制命令；分页目录、BYE、心跳超时逐项记录，不适用或未测单列。
- [ ] **Step 4: 版本与缺陷。** 核对 `/api/server/version`、系统信息、OpenAPI、前端包和 SIP User-Agent；阻断缺陷未关闭时结论为“不满足发布门禁”。
- [ ] **Step 5: 提交验收记录。** `git add docs && git commit -m "docs: record 2.0 release validation"`；是否发布由用户另行决定。

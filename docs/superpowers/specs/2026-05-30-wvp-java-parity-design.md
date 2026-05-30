# WVP-Pro Java Backend Protocol-Production Parity Design

Date: 2026-05-30

## 1. Background

The Rust backend in this repository is intended to become a production-grade replacement for the official WVP-Pro Java backend. The selected upstream baseline is:

- Repository: `https://github.com/648540858/wvp-GB28181-pro`
- Branch: `master`
- Commit observed during design: `b760458`
- Upstream version: WVP-Pro `2.7.4`
- Upstream stack: Spring Boot `3.4.4`, Java `21`

The target is not API-only compatibility. The target is protocol-production parity: the Rust backend must interoperate with real GB28181 devices, ZLMediaKit media servers, upstream GB platforms, and JT808/JT1078 terminals in the same operational scenarios that WVP-Pro supports.

## 2. Parity definition

A feature is considered complete only when all applicable layers are aligned:

1. **API compatibility**: path, method, query/body parameters, response shape, error semantics, and frontend behavior match WVP-Pro closely enough for the WVP frontend and API clients.
2. **Protocol behavior**: SIP, GB28181, ZLM hook, SendRtp, JT808/JT1078, and Redis/event behavior complete the real command-response lifecycle.
3. **State management**: sessions, dialogs, SSRC, streams, subscriptions, device status, media-node status, and Redis-backed cluster state are cleaned up and recovered correctly.
4. **Verification**: behavior is covered by unit tests, simulator tests, integration tests, API contract checks, and selected real-device or real-platform validation.

Returning a WVP-compatible success body without executing the underlying protocol is not considered production parity.

## 3. Current assessment

The current Rust implementation is not an empty shell. It already includes:

- Axum API routing and WVP-style response wrappers.
- SQLx database modules for many WVP tables/domains.
- JWT and API key authentication.
- A Vue 2 frontend copied or adapted from WVP-style frontend structure.
- SIP/GB28181 server code, ZLM client/hooks, cascade registration, record-plan scheduling, and JT1078 server/manager modules.

However, most protocol-heavy domains remain **partial** rather than fully aligned. Many endpoints exist but still rely on compatibility success responses, default data, fallback URLs, immediate returns after fire-and-forget commands, or local ZLM file/stream data instead of completing the upstream WVP-Pro protocol lifecycle.

Estimated current completion by production-parity lens:

| Area | Estimated completion |
|---|---:|
| Management/CRUD/API surface compatibility | 60-75% |
| Frontend usability | 55-70% |
| GB28181 core production flows | 35-50% |
| ZLM/media-node production flows | 45-60% |
| Platform cascade production flows | 25-40% |
| JT808/JT1078 production flows | 25-40% |
| Redis/cluster/RPC/operations parity | 20-35% |
| Overall protocol-production parity | 40-55% |

These are planning estimates, not final audit numbers. Phase 0 will replace them with a repeatable generated gap matrix.

## 4. Gap matrix

| Domain | Current status | Main parity gaps |
|---|---|---|
| User/JWT/API key | Mostly aligned surface | Login, token, and frontend request model are close; role, permission, audit status, and error semantics need verification. |
| CRUD management APIs | Partial | Device, region, group, role, push, proxy, media-node APIs mostly exist; field-level response parity, paging, sorting, empty values, and error responses require contract checks. |
| SIP REGISTER/Keepalive | Partial | Basic registration and heartbeat exist; strict GB ID validation, per-device password rules, transaction semantics, Redis/cluster state, and edge offline behavior need alignment. |
| Catalog/device channel sync | Partial | Catalog query and partial processing exist; multi-packet handling, progress, channel deletion/offline updates, parent/civilCode/businessGroup mapping, and subscription lifecycle need work. |
| Device status/config/control/PTZ | Partial | Many commands can be sent, but several are fire-and-forget and do not wait for parsed device responses. PTZ, preset, cruise, scan, and aux commands need WVP-Pro-compatible semantics. |
| Live play | Partial | ZLM RTP server and SIP INVITE exist; SSRC, StreamContent, media-arrival hook, timeout cleanup, streamId, and invite session behavior need alignment. |
| Playback | Partial/stale | Current flow includes local proxy-style fallbacks. Playback SDP, pause/resume/seek/speed, invite state, and ZLM media arrival must be rebuilt around real GB28181 playback sessions. |
| GB RecordInfo/Download | High-risk partial | Current query tends to send RecordInfo then return local/ZLM results or empty data. Need waiting, multi-packet aggregation, pagination, Download INVITE, progress, and stop semantics. |
| ZLM hook/media node | Partial | Several hooks exist, but WVP-Pro per-hook routes, play/publish auth, send_rtp_stopped, none_reader, stream_not_found, server_started, ABL hooks, and node config sync remain incomplete. |
| Platform cascade | Significant partial | Platform CRUD and basic REGISTER exist; Catalog NOTIFY, upstream INVITE, SendRtp, subscriptions, alarm/mobile-position forwarding, and keepalive state machine are not production-equivalent. |
| JT808/JT1078 | Significant partial | Route surface and command framework exist; live/playback start must always command the terminal, responses need correlation, and media/query/control flows need real terminal validation. |
| Alarm/mobile position/subscriptions | Partial | MESSAGE alarm/position handling exists; alarm subscribe, latest/realtime/history APIs, NOTIFY handling, Redis push, and renewal tasks need parity. |
| Record plan/cloud record | Partial | Scheduler and ZLM recording exist; task lifecycle, retry, online recovery, cleanup, playback/download/zip/list-url behavior require WVP-Pro comparison. |
| Map/channel thinning/MVT | Partial | GeoJSON/thinning exists; MVT endpoints, progress model, SQL field correctness, trajectory data, and administrative hierarchy need alignment. |
| Redis/cluster/RPC | Missing/partial | Current Redis use is mostly cache/counting. WVP-Pro uses Redis for CSEQ/SN/SSRC, device/stream state, GPS/alarm, stream push/proxy messages, platform play/sendRtp, group updates, and RPC. |
| `/api/rtp` and `/api/ps` | Likely missing | WVP-Pro exposes RTP/PS receive/send control APIs; current router does not show equivalent route groups. |
| Security route coverage | Known issue | `/api/alarm/*` and `/api/ws` appear mounted outside the primary auth layer and need explicit public/protected policy. |

## 5. Target architecture

The Rust backend should move from handler-centered compatibility behavior to a layered protocol-service architecture:

```text
HTTP API / WebSocket
        ↓
Domain services
DeviceService / PlayService / PlaybackService / RecordService
PlatformCascadeService / MediaServerService / Jt1078Service
        ↓
Protocol orchestration
SIP Transaction & Dialog / InviteSession / QueryWaiter / SubscriptionTask
JT808 Session / CommandCorrelation / MediaSession
        ↓
Transport and media
SIP UDP/TCP / ZLM RTP Server / ZLM Hook / SendRtp / Redis Events
        ↓
DB + Redis state
```

Handlers should stay thin: parse request state, call a service, and return `WVPResult<T>` or `AppError`. Protocol state and waiting logic should live below handlers and be directly testable.

Recommended module boundaries:

```text
src/services/
  device_service.rs
  play_service.rs
  playback_service.rs
  record_service.rs
  media_server_service.rs
  platform_service.rs
  jt1078_service.rs

src/protocol/
  pending_request.rs
  invite_session_store.rs
  subscription_scheduler.rs

src/sip/gb28181/
  commander.rs
  response_router.rs
  platform_commander.rs

src/tests/support/
  sip_device_simulator.rs
  zlm_hook_simulator.rs
  jt1078_terminal_simulator.rs
```

The exact file layout can be adjusted to fit the existing codebase, but the boundaries should remain: handlers, domain services, protocol orchestration, transport/media, and state store.

## 6. Core design principles

### 6.1 Every protocol request needs a full lifecycle

Each SIP or JT request that expects a response should follow:

```text
command key → pending request → response resolves → parsed result returned → timeout/error handled → state cleaned
```

This applies to DeviceStatus, DeviceInfo, ConfigDownload, Catalog sync, RecordInfo, Playback/Download INVITE, preset query, JT parameter/query commands, and similar flows.

### 6.2 INVITE, SSRC, and stream IDs are first-class state

Live, playback, download, talk, broadcast, and cascade SendRtp should use a unified session structure:

```text
InviteSession {
  device_id,
  channel_id,
  stream_id,
  ssrc,
  call_id,
  dialog,
  media_server_id,
  rtp_port,
  play_type,
  status,
  timeout_at
}
```

The API should return stream information only after the required SIP and ZLM events have completed, or after a controlled timeout with cleanup.

### 6.3 ZLM hook events drive media state

ZLM hooks are part of the control plane. They must update stream/media state, resolve pending invite/download/record tasks, update Redis where configured, and broadcast WebSocket events.

The implementation should support the WVP-Pro hook set: server keepalive, server started, play auth, publish auth, stream changed, stream none reader, stream not found, send RTP stopped, RTP server timeout, and MP4 record. ABL hook support should be added if production deployments use it.

### 6.4 Platform cascade is its own service

Platform cascade should not be treated as generic device CRUD. It needs a dedicated service for REGISTER/UNREGISTER, keepalive, Catalog NOTIFY, DeviceInfo/DeviceStatus/RecordInfo responses, upstream INVITE handling, SendRtp, BYE/CANCEL cleanup, and alarm/mobile-position/catalog subscription forwarding.

### 6.5 JT1078 requires session, command correlation, and media session layers

JT1078 parity requires three layers:

```text
JtTerminalSession
JtCommandWaiter
JtMediaSession
```

`live/start` and `playback/start` must issue real terminal commands, correlate responses, wait for ZLM media arrival, and clean up correctly.

### 6.6 Redis should become an optional state bus

The code should support both single-node and Redis-backed clustered operation using an abstraction such as:

```text
StateStore
  InMemoryStateStore
  RedisStateStore
```

The Redis implementation should eventually cover CSEQ/SN/SSRC, device online state, stream state, invite/session state, GPS/alarm, push/proxy messages, platform SendRtp, WebSocket fanout, and cross-node RPC.

## 7. Implementation phases

### Phase 0: Baseline and automated gap audit

Goal: make WVP-Pro comparison repeatable.

Tasks:

1. Record upstream baseline `648540858/wvp-GB28181-pro@b760458`.
2. Extract Java controller routes and Rust router routes.
3. Generate a route/domain gap report with statuses: `missing`, `partial`, `protocol-gap`, `response-gap`, `aligned`, `not-in-scope`.
4. Refresh stale parity documentation and clearly mark old conclusions that are no longer accurate.

Acceptance:

- A repeatable route/domain diff can be generated.
- Every gap has status, priority, evidence, and a planned phase.

Estimate: 2-4 days.

### Phase 1: SIP/GB28181 request waiter and session foundation

Goal: build the request/session lifecycle needed by later protocol features.

Tasks:

1. Implement `PendingRequest` keyed by `device_id + sn`, `call_id`, `cseq`, or branch.
2. Implement `InviteSessionStore` for live/playback/download/talk/cascade sessions.
3. Route SIP MESSAGE, NOTIFY, 200/4xx responses, BYE, CANCEL, and timeouts into pending request/session resolution.
4. Add SIP simulator tests for REGISTER, Keepalive, Catalog, RecordInfo, INVITE 200 OK, and BYE.

Acceptance:

- DeviceInfo, DeviceStatus, or ConfigDownload can complete command → response → parsed API return.
- Invite sessions can be resolved by SIP and ZLM events.
- Timeout cleanup is tested.

Estimate: 1-2 weeks.

### Phase 2: Device query, Catalog, subscriptions, and PTZ

Goal: make device management and control production-usable.

Tasks:

1. Rework DeviceStatus, DeviceInfo, and Config to wait for responses.
2. Complete Catalog multi-packet sync, progress tracking, channel updates, and administrative/parent relationships.
3. Complete Catalog, MobilePosition, and Alarm subscription lifecycle.
4. Align PTZ, preset, cruise, scan, and auxiliary controls with WVP-Pro command semantics.

Acceptance:

- Simulated and selected real devices can register, sync catalog, answer device queries, accept PTZ, and publish subscribed position/alarm events.
- Device-related compatibility-empty handlers are removed, renamed, or isolated.

Estimate: 2-3 weeks.

### Phase 3: Live, playback, RecordInfo, Download, and talk/broadcast

Goal: make video and record flows production-equivalent.

Tasks:

1. Rework live play around SSRC allocation, ZLM RTP server, SIP INVITE, ACK, ZLM media arrival, timeout, and cleanup.
2. Rework playback start/stop/pause/resume/seek/speed using real playback SDP and invite sessions.
3. Rework RecordInfo query to wait for multi-packet device responses and aggregate/paginate results.
4. Rework Download to use GB28181 Download INVITE, progress tracking, and controlled stop.
5. Separate talk/broadcast sessions from live sessions and clean up BYE/RTP state correctly.

Acceptance:

- Live play, stop, playback controls, RecordInfo, Download progress/stop, and talk/broadcast work with simulator and at least one real device environment.
- Main flows no longer rely on placeholder `127.0.0.1/live/...` proxy paths.

Estimate: 3-5 weeks.

### Phase 4: ZLM/media-node production parity

Goal: align ZLM hook and media-node behavior with WVP-Pro.

Tasks:

1. Support WVP-Pro hook routes or a compatible dispatcher for all relevant `on_*` hooks.
2. Implement play/publish auth, stream_changed, none_reader, stream_not_found, send_rtp_stopped, rtp_server_timeout, record_mp4, server_started, and keepalive behavior.
3. Complete media-node save/check/delete/load, hook auto-configuration, RTP port ranges, secret validation, online/offline status, and least-load selection.
4. Unify stream status for GB streams, push streams, proxy streams, and SendRtp streams.

Acceptance:

- ZLM startup auto-configures hooks.
- ZLM stream events resolve pending invite/download/session state.
- Media-node offline/online transitions do not leave stale sessions.

Estimate: 2-3 weeks.

### Phase 5: Platform cascade production parity

Goal: operate as a lower-level platform for WVP-Pro or standard GB upstream platforms.

Tasks:

1. Complete REGISTER refresh, UNREGISTER, keepalive, retry/backoff, and multi-platform state.
2. Respond to upstream Catalog, DeviceInfo, DeviceStatus, and RecordInfo queries for shared channels.
3. Implement upstream INVITE → local play → ZLM SendRtp → upstream media delivery.
4. Clean up SendRtp on BYE/CANCEL and send_rtp_stopped.
5. Forward catalog, mobile-position, and alarm subscriptions where configured.

Acceptance:

- Java WVP-Pro can register this Rust backend as a lower-level platform.
- Upstream platform can query catalog, play shared channels, stop streams, and receive selected subscriptions.

Estimate: 3-5 weeks.

### Phase 6: JT808/JT1078 production parity

Goal: make JT terminal workflows real rather than route-only.

Tasks:

1. Complete terminal registration/auth, heartbeat, TCP/UDP transport, offline detection, and DB mapping.
2. Add command correlation by msg_id, serial number, and phone.
3. Rework live/start, live/stop, pause/continue/switch around real terminal commands and ZLM media arrival.
4. Complete record list, playback start/control/stop, download, media upload/list/delete.
5. Complete PTZ, text, phonebook, fence/route, config query/set, attribute, driver, and media attribute flows where WVP-Pro supports them.

Acceptance:

- Simulator and at least one real JT terminal can register, start/stop live video, perform playback/control, query records, and run selected controls.
- API route coverage includes WVP-Pro live pause/continue/switch paths.
- Default placeholder coordinates, driver data, and media attributes are not returned as primary production data.

Estimate: 4-6 weeks.

### Phase 7: Redis cluster, RPC, WebSocket, operations, and edge APIs

Goal: complete production deployment features.

Tasks:

1. Implement Redis-backed `StateStore` for CSEQ/SN/SSRC, device/stream/session state, GPS/alarm, push/proxy, platform SendRtp, and WebSocket fanout.
2. Add cross-node RPC for device control, play/stop, stream state, platform play, SendRtp, and cloud-record operations.
3. Add or align `/api/rtp`, `/api/ps`, log download, system info, health/readiness, and metrics behavior.
4. Fix public/protected routing for `/api/alarm/*` and `/api/ws` or document them as intentionally public.
5. Record audit logs with real response statuses.

Acceptance:

- Single-node and two-node Redis-backed deployments pass core protocol smoke tests.
- WebSocket events and stream states remain consistent across nodes.
- Security route exposure matches intended policy.

Estimate: 2-4 weeks.

## 8. Recommended first sprint

The first implementation sprint should not randomly patch endpoints. It should establish the protocol foundation by completing:

1. Phase 0 route/domain audit.
2. Phase 1 `PendingRequest` and `InviteSessionStore` core.
3. Three end-to-end closed loops:
   - DeviceInfo query waits for a simulated response and returns parsed data.
   - Live play waits for SIP response and ZLM media-arrival hook before returning stream content.
   - RecordInfo waits for multi-packet simulated device responses and returns aggregated records.

These three flows validate the architecture needed for later playback, download, cascade, and JT1078 work.

## 9. Test strategy

| Test layer | Purpose | Coverage |
|---|---|---|
| Unit | Validate small protocol components | XML parsing, SDP, SSRC, PTZ encoding, JT808/JT1078 codecs, route diff parser |
| Service | Validate lifecycle logic | PendingRequest, InviteSession, SubscriptionTask, MediaState, RecordInfo aggregation |
| Protocol simulator | Validate protocol interop without hardware | SIP device, upstream GB platform, ZLM hook, JT terminal |
| Integration | Validate system collaboration | PostgreSQL/MySQL, Redis, ZLM, backend, WebSocket |
| API contract | Validate WVP-Pro compatibility | path, method, query/body, response shape, error code |
| Frontend smoke | Validate UI behavior | login, device, channel, play, record, cascade, JT, media node pages |
| Real device/platform | Validate production behavior | GB camera/NVR, upstream WVP-Pro, JT terminal |
| Regression | Prevent backsliding | automated phase-specific core flows |

## 10. Risks and mitigations

### Upstream WVP-Pro changes

Mitigation: freeze `b760458` for this parity cycle. Later upstream changes should be handled as a separate upgrade cycle using the same route/domain diff tooling.

### API-only false positives

Mitigation: mark every feature as `api-only`, `simulated-protocol`, `real-protocol`, or `production-verified`. A success response alone does not close a gap.

### Lack of real hardware during development

Mitigation: build simulators first, then run selected real-device validation before declaring production parity. Reports must distinguish simulator pass from real-device pass.

### SIP stack complexity

Mitigation: separate parser, transaction/dialog, response routing, and business handlers. High-risk TCP and transaction behavior should be tested independently before large feature changes depend on it.

### Existing stub naming and duplicate handlers

Mitigation: classify current `stub.rs` and `device_stub.rs` functions during Phase 0. Migrate real behavior into clearly named services, isolate compatibility shims, and remove or rename stale duplicate functions.

### Large-scope refactor risk

Mitigation: deliver by closed-loop protocol slices, keep public handlers stable, and avoid a big-bang rewrite.

## 11. Completion criteria

The parity project is complete when:

1. Generated route/domain diff shows all in-scope WVP-Pro domains as `aligned` or intentionally `not-in-scope`.
2. Core GB28181 device, live, playback, RecordInfo, Download, talk, alarm, and mobile-position flows pass simulator and selected real-device tests.
3. ZLM hook and media-node flows pass startup, stream arrival, stream stop, no-reader, not-found, send-rtp-stopped, and record MP4 tests.
4. Platform cascade can register with WVP-Pro as an upstream platform and complete catalog/play/stop/subscription flows.
5. JT1078 can register terminals and complete live/playback/query/control flows for selected terminal profiles.
6. Redis-backed two-node deployment passes core state and WebSocket consistency tests.
7. Frontend smoke tests pass for the major WVP pages.
8. Documentation reflects actual implementation status with no stale 100% or all-stub claims.

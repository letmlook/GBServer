# WVP-Pro Phase 0 Parity Audit

Generated at: 2026-06-10T16:07:00.146Z
Baseline commit: `unknown`
Upstream repository path: `/tmp/wvp-GB28181-pro`
Local repository path: `/Users/lipeng/.codex/worktrees/63cb/GBServer`

## Scope

This report compares the official WVP-Pro Java backend controllers and the official WVP-Pro frontend from the same commit against the Rust backend router and the local frontend in this repository.

## Extracted counts

Java controller routes: 0
Rust Axum routes: 367
Upstream frontend API calls: 0
Local frontend API calls: 244
Upstream frontend pages: 0
Local frontend pages: 24

| Source | Count |
|---|---:|
| Java controller routes | 0 |
| Rust Axum routes | 367 |
| Upstream frontend API calls | 0 |
| Local frontend API calls | 244 |
| Upstream frontend pages | 0 |
| Local frontend pages | 24 |

## Comparisons

### Backend Java controllers → Rust router

| Status | Count |
|---|---:|
| Aligned | 0 |
| Missing | 0 |
| Method mismatch | 0 |
| Extra target entries | 367 |

#### Extra target entries

| Method | Path | Source |
|---|---|---|
| DELETE | `/api/alarm/batch` | src/router.rs |
| DELETE | `/api/alarm/before/{param}` | src/router.rs |
| DELETE | `/api/alarm/clear` | src/router.rs |
| DELETE | `/api/alarm/delete/{param}` | src/router.rs |
| GET | `/api/alarm/detail/{param}` | src/router.rs |
| DELETE | `/api/alarm/device/{param}` | src/router.rs |
| POST | `/api/alarm/handle` | src/router.rs |
| GET | `/api/alarm/list` | src/router.rs |
| GET | `/api/alarm/snap/{param}` | src/router.rs |
| GET | `/api/cloud/record/collect/add` | src/router.rs |
| GET | `/api/cloud/record/collect/delete` | src/router.rs |
| GET | `/api/cloud/record/date/list` | src/router.rs |
| DELETE | `/api/cloud/record/delete` | src/router.rs |
| GET | `/api/cloud/record/download/zip` | src/router.rs |
| GET | `/api/cloud/record/list` | src/router.rs |
| GET | `/api/cloud/record/list-url` | src/router.rs |
| GET | `/api/cloud/record/loadRecord` | src/router.rs |
| GET | `/api/cloud/record/play/path` | src/router.rs |
| GET | `/api/cloud/record/seek` | src/router.rs |
| GET | `/api/cloud/record/speed` | src/router.rs |
| GET | `/api/cloud/record/task/add` | src/router.rs |
| GET | `/api/cloud/record/task/list` | src/router.rs |
| GET | `/api/cloud/record/zip` | src/router.rs |
| POST | `/api/common/channel/add` | src/router.rs |
| GET | `/api/common/channel/civilcode/list` | src/router.rs |
| POST | `/api/common/channel/civilCode/unusual/clear` | src/router.rs |
| GET | `/api/common/channel/civilCode/unusual/list` | src/router.rs |
| GET | `/api/common/channel/front-end/auxiliary` | src/router.rs |
| GET | `/api/common/channel/front-end/fi/focus` | src/router.rs |
| GET | `/api/common/channel/front-end/fi/iris` | src/router.rs |
| GET | `/api/common/channel/front-end/preset/add` | src/router.rs |
| GET | `/api/common/channel/front-end/preset/call` | src/router.rs |
| GET | `/api/common/channel/front-end/preset/delete` | src/router.rs |
| GET | `/api/common/channel/front-end/preset/query` | src/router.rs |
| GET | `/api/common/channel/front-end/ptz` | src/router.rs |
| GET | `/api/common/channel/front-end/scan/set/left` | src/router.rs |
| GET | `/api/common/channel/front-end/scan/set/right` | src/router.rs |
| GET | `/api/common/channel/front-end/scan/set/speed` | src/router.rs |
| GET | `/api/common/channel/front-end/scan/start` | src/router.rs |
| GET | `/api/common/channel/front-end/scan/stop` | src/router.rs |
| GET | `/api/common/channel/front-end/tour/point/add` | src/router.rs |
| GET | `/api/common/channel/front-end/tour/point/delete` | src/router.rs |
| GET | `/api/common/channel/front-end/tour/speed` | src/router.rs |
| GET | `/api/common/channel/front-end/tour/start` | src/router.rs |
| GET | `/api/common/channel/front-end/tour/stop` | src/router.rs |
| GET | `/api/common/channel/front-end/tour/time` | src/router.rs |
| GET | `/api/common/channel/front-end/wiper` | src/router.rs |
| POST | `/api/common/channel/group/add` | src/router.rs |
| POST | `/api/common/channel/group/delete` | src/router.rs |
| POST | `/api/common/channel/group/device/add` | src/router.rs |
| POST | `/api/common/channel/group/device/delete` | src/router.rs |
| GET | `/api/common/channel/industry/list` | src/router.rs |
| GET | `/api/common/channel/list` | src/router.rs |
| GET | `/api/common/channel/map/list` | src/router.rs |
| POST | `/api/common/channel/map/reset-level` | src/router.rs |
| POST | `/api/common/channel/map/save-level` | src/router.rs |
| GET | `/api/common/channel/map/thin/clear` | src/router.rs |
| POST | `/api/common/channel/map/thin/draw` | src/router.rs |
| GET | `/api/common/channel/map/thin/progress` | src/router.rs |
| GET | `/api/common/channel/map/thin/save` | src/router.rs |
| GET | `/api/common/channel/map/thin/tile/{param}/{param}/{param}` | src/router.rs |
| GET | `/api/common/channel/map/tile/{param}/{param}/{param}` | src/router.rs |
| GET | `/api/common/channel/network/identification/list` | src/router.rs |
| GET | `/api/common/channel/one` | src/router.rs |
| GET | `/api/common/channel/parent/list` | src/router.rs |
| POST | `/api/common/channel/parent/unusual/clear` | src/router.rs |
| GET | `/api/common/channel/parent/unusual/list` | src/router.rs |
| GET | `/api/common/channel/play` | src/router.rs |
| GET | `/api/common/channel/play/stop` | src/router.rs |
| GET | `/api/common/channel/playback` | src/router.rs |
| GET | `/api/common/channel/playback/pause` | src/router.rs |
| GET | `/api/common/channel/playback/query` | src/router.rs |
| GET | `/api/common/channel/playback/resume` | src/router.rs |
| GET | `/api/common/channel/playback/seek` | src/router.rs |
| GET | `/api/common/channel/playback/speed` | src/router.rs |
| GET | `/api/common/channel/playback/stop` | src/router.rs |
| POST | `/api/common/channel/region/add` | src/router.rs |
| POST | `/api/common/channel/region/delete` | src/router.rs |
| POST | `/api/common/channel/region/device/add` | src/router.rs |
| POST | `/api/common/channel/region/device/delete` | src/router.rs |
| ... | 287 more | ... |

### Official frontend API → Rust router

| Status | Count |
|---|---:|
| Aligned | 0 |
| Missing | 0 |
| Method mismatch | 0 |
| Extra target entries | 367 |

#### Extra target entries

| Method | Path | Source |
|---|---|---|
| DELETE | `/api/alarm/batch` | src/router.rs |
| DELETE | `/api/alarm/before/{param}` | src/router.rs |
| DELETE | `/api/alarm/clear` | src/router.rs |
| DELETE | `/api/alarm/delete/{param}` | src/router.rs |
| GET | `/api/alarm/detail/{param}` | src/router.rs |
| DELETE | `/api/alarm/device/{param}` | src/router.rs |
| POST | `/api/alarm/handle` | src/router.rs |
| GET | `/api/alarm/list` | src/router.rs |
| GET | `/api/alarm/snap/{param}` | src/router.rs |
| GET | `/api/cloud/record/collect/add` | src/router.rs |
| GET | `/api/cloud/record/collect/delete` | src/router.rs |
| GET | `/api/cloud/record/date/list` | src/router.rs |
| DELETE | `/api/cloud/record/delete` | src/router.rs |
| GET | `/api/cloud/record/download/zip` | src/router.rs |
| GET | `/api/cloud/record/list` | src/router.rs |
| GET | `/api/cloud/record/list-url` | src/router.rs |
| GET | `/api/cloud/record/loadRecord` | src/router.rs |
| GET | `/api/cloud/record/play/path` | src/router.rs |
| GET | `/api/cloud/record/seek` | src/router.rs |
| GET | `/api/cloud/record/speed` | src/router.rs |
| GET | `/api/cloud/record/task/add` | src/router.rs |
| GET | `/api/cloud/record/task/list` | src/router.rs |
| GET | `/api/cloud/record/zip` | src/router.rs |
| POST | `/api/common/channel/add` | src/router.rs |
| GET | `/api/common/channel/civilcode/list` | src/router.rs |
| POST | `/api/common/channel/civilCode/unusual/clear` | src/router.rs |
| GET | `/api/common/channel/civilCode/unusual/list` | src/router.rs |
| GET | `/api/common/channel/front-end/auxiliary` | src/router.rs |
| GET | `/api/common/channel/front-end/fi/focus` | src/router.rs |
| GET | `/api/common/channel/front-end/fi/iris` | src/router.rs |
| GET | `/api/common/channel/front-end/preset/add` | src/router.rs |
| GET | `/api/common/channel/front-end/preset/call` | src/router.rs |
| GET | `/api/common/channel/front-end/preset/delete` | src/router.rs |
| GET | `/api/common/channel/front-end/preset/query` | src/router.rs |
| GET | `/api/common/channel/front-end/ptz` | src/router.rs |
| GET | `/api/common/channel/front-end/scan/set/left` | src/router.rs |
| GET | `/api/common/channel/front-end/scan/set/right` | src/router.rs |
| GET | `/api/common/channel/front-end/scan/set/speed` | src/router.rs |
| GET | `/api/common/channel/front-end/scan/start` | src/router.rs |
| GET | `/api/common/channel/front-end/scan/stop` | src/router.rs |
| GET | `/api/common/channel/front-end/tour/point/add` | src/router.rs |
| GET | `/api/common/channel/front-end/tour/point/delete` | src/router.rs |
| GET | `/api/common/channel/front-end/tour/speed` | src/router.rs |
| GET | `/api/common/channel/front-end/tour/start` | src/router.rs |
| GET | `/api/common/channel/front-end/tour/stop` | src/router.rs |
| GET | `/api/common/channel/front-end/tour/time` | src/router.rs |
| GET | `/api/common/channel/front-end/wiper` | src/router.rs |
| POST | `/api/common/channel/group/add` | src/router.rs |
| POST | `/api/common/channel/group/delete` | src/router.rs |
| POST | `/api/common/channel/group/device/add` | src/router.rs |
| POST | `/api/common/channel/group/device/delete` | src/router.rs |
| GET | `/api/common/channel/industry/list` | src/router.rs |
| GET | `/api/common/channel/list` | src/router.rs |
| GET | `/api/common/channel/map/list` | src/router.rs |
| POST | `/api/common/channel/map/reset-level` | src/router.rs |
| POST | `/api/common/channel/map/save-level` | src/router.rs |
| GET | `/api/common/channel/map/thin/clear` | src/router.rs |
| POST | `/api/common/channel/map/thin/draw` | src/router.rs |
| GET | `/api/common/channel/map/thin/progress` | src/router.rs |
| GET | `/api/common/channel/map/thin/save` | src/router.rs |
| GET | `/api/common/channel/map/thin/tile/{param}/{param}/{param}` | src/router.rs |
| GET | `/api/common/channel/map/tile/{param}/{param}/{param}` | src/router.rs |
| GET | `/api/common/channel/network/identification/list` | src/router.rs |
| GET | `/api/common/channel/one` | src/router.rs |
| GET | `/api/common/channel/parent/list` | src/router.rs |
| POST | `/api/common/channel/parent/unusual/clear` | src/router.rs |
| GET | `/api/common/channel/parent/unusual/list` | src/router.rs |
| GET | `/api/common/channel/play` | src/router.rs |
| GET | `/api/common/channel/play/stop` | src/router.rs |
| GET | `/api/common/channel/playback` | src/router.rs |
| GET | `/api/common/channel/playback/pause` | src/router.rs |
| GET | `/api/common/channel/playback/query` | src/router.rs |
| GET | `/api/common/channel/playback/resume` | src/router.rs |
| GET | `/api/common/channel/playback/seek` | src/router.rs |
| GET | `/api/common/channel/playback/speed` | src/router.rs |
| GET | `/api/common/channel/playback/stop` | src/router.rs |
| POST | `/api/common/channel/region/add` | src/router.rs |
| POST | `/api/common/channel/region/delete` | src/router.rs |
| POST | `/api/common/channel/region/device/add` | src/router.rs |
| POST | `/api/common/channel/region/device/delete` | src/router.rs |
| ... | 287 more | ... |

### Official frontend API → local frontend API

| Status | Count |
|---|---:|
| Aligned | 0 |
| Missing | 0 |
| Method mismatch | 0 |
| Extra target entries | 244 |

#### Extra target entries

| Method | Path | Source |
|---|---|---|
| GET | `/api/cloud/record/date/list` | web/src/api/cloudRecord.js |
| DELETE | `/api/cloud/record/delete` | web/src/api/cloudRecord.js |
| GET | `/api/cloud/record/list` | web/src/api/cloudRecord.js |
| GET | `/api/cloud/record/loadRecord` | web/src/api/cloudRecord.js |
| GET | `/api/cloud/record/play/path` | web/src/api/cloudRecord.js |
| GET | `/api/cloud/record/seek` | web/src/api/cloudRecord.js |
| GET | `/api/cloud/record/speed` | web/src/api/cloudRecord.js |
| GET | `/api/cloud/record/task/add` | web/src/api/cloudRecord.js |
| GET | `/api/cloud/record/task/list` | web/src/api/cloudRecord.js |
| POST | `/api/common/channel/add` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/civilcode/list` | web/src/api/commonChannel.js |
| POST | `/api/common/channel/civilCode/unusual/clear` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/civilCode/unusual/list` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/auxiliary` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/fi/focus` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/fi/iris` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/preset/add` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/preset/call` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/preset/delete` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/preset/query` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/ptz` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/scan/set/left` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/scan/set/right` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/scan/set/speed` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/scan/start` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/scan/stop` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/tour/point/add` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/tour/point/delete` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/tour/speed` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/tour/start` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/tour/stop` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/tour/time` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/front-end/wiper` | web/src/api/commonChannel.js |
| POST | `/api/common/channel/group/add` | web/src/api/commonChannel.js |
| POST | `/api/common/channel/group/delete` | web/src/api/commonChannel.js |
| POST | `/api/common/channel/group/device/add` | web/src/api/commonChannel.js |
| POST | `/api/common/channel/group/device/delete` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/industry/list` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/list` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/map/list` | web/src/api/commonChannel.js |
| POST | `/api/common/channel/map/reset-level` | web/src/api/commonChannel.js |
| POST | `/api/common/channel/map/save-level` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/map/thin/clear` | web/src/api/commonChannel.js |
| POST | `/api/common/channel/map/thin/draw` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/map/thin/progress` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/map/thin/save` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/network/identification/list` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/one` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/parent/list` | web/src/api/commonChannel.js |
| POST | `/api/common/channel/parent/unusual/clear` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/parent/unusual/list` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/play` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/play/stop` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/playback` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/playback/pause` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/playback/query` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/playback/resume` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/playback/seek` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/playback/speed` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/playback/stop` | web/src/api/commonChannel.js |
| POST | `/api/common/channel/region/add` | web/src/api/commonChannel.js |
| POST | `/api/common/channel/region/delete` | web/src/api/commonChannel.js |
| POST | `/api/common/channel/region/device/add` | web/src/api/commonChannel.js |
| POST | `/api/common/channel/region/device/delete` | web/src/api/commonChannel.js |
| POST | `/api/common/channel/reset` | web/src/api/commonChannel.js |
| GET | `/api/common/channel/type/list` | web/src/api/commonChannel.js |
| POST | `/api/common/channel/update` | web/src/api/commonChannel.js |
| GET | `/api/device/config/query/{param}/BasicParam` | web/src/api/device.js |
| GET | `/api/device/control/guard` | web/src/api/device.js |
| GET | `/api/device/control/guard` | web/src/api/device.js |
| GET | `/api/device/control/record` | web/src/api/device.js |
| POST | `/api/device/query/channel/audio` | web/src/api/device.js |
| GET | `/api/device/query/channel/one` | web/src/api/device.js |
| POST | `/api/device/query/channel/stream/identification/update` | web/src/api/device.js |
| POST | `/api/device/query/device/add` | web/src/api/device.js |
| POST | `/api/device/query/device/update` | web/src/api/device.js |
| GET | `/api/device/query/devices` | web/src/api/device.js |
| GET | `/api/device/query/devices/{param}` | web/src/api/device.js |
| GET | `/api/device/query/devices/{param}/channels` | web/src/api/device.js |
| DELETE | `/api/device/query/devices/{param}/delete` | web/src/api/device.js |
| ... | 164 more | ... |

### Official frontend pages → local frontend pages

| Status | Count |
|---|---:|
| Aligned | 0 |
| Missing | 0 |
| Method mismatch | 0 |
| Extra target entries | 24 |

#### Extra target entries

| Method | Path | Source |
|---|---|---|
| PAGE | `/channel` | web/src/router/index.js |
| PAGE | `/channel/record/{param}` | web/src/router/index.js |
| PAGE | `/cloudRecord` | web/src/router/index.js |
| PAGE | `/cloudRecord/detail/{param}/{param}` | web/src/router/index.js |
| PAGE | `/commonChannel/group` | web/src/router/index.js |
| PAGE | `/commonChannel/region` | web/src/router/index.js |
| PAGE | `/dashboard` | web/src/router/index.js |
| PAGE | `/device` | web/src/router/index.js |
| PAGE | `/device/record/{param}/{param}` | web/src/router/index.js |
| PAGE | `/jtDevice` | web/src/router/index.js |
| PAGE | `/jtDevice/record/{param}/{param}` | web/src/router/index.js |
| PAGE | `/live` | web/src/router/index.js |
| PAGE | `/map` | web/src/router/index.js |
| PAGE | `/mediaServer` | web/src/router/index.js |
| PAGE | `/operations/historyLog` | web/src/router/index.js |
| PAGE | `/operations/realLog` | web/src/router/index.js |
| PAGE | `/operations/systemInfo` | web/src/router/index.js |
| PAGE | `/platform` | web/src/router/index.js |
| PAGE | `/play/rtc/{param}` | web/src/router/index.js |
| PAGE | `/play/wasm/{param}` | web/src/router/index.js |
| PAGE | `/proxy` | web/src/router/index.js |
| PAGE | `/push` | web/src/router/index.js |
| PAGE | `/recordPlan` | web/src/router/index.js |
| PAGE | `/user` | web/src/router/index.js |

## Status policy

- `aligned`: same canonical path and method/page exists.
- `missing`: reference entry is absent from the target.
- `method mismatch`: canonical path exists but HTTP methods differ.
- `extra target entries`: target has entries not found in the reference; these may be extensions or obsolete local APIs.

This report is a route and frontend-surface audit. Protocol-production status must be assigned in follow-up Phase 1+ implementation plans and tests.

# WVP-Pro Phase 0 Parity Audit

Generated at: 2026-05-30T16:40:24.498Z
Baseline commit: `b760458`
Upstream repository path: `/tmp/wvp-GB28181-pro`
Local repository path: `/Users/lipeng/GBServer/.claude/worktrees/wvp-phase-0-parity-audit`

## Scope

This report compares the official WVP-Pro Java backend controllers and the official WVP-Pro frontend from the same commit against the Rust backend router and the local frontend in this repository.

## Extracted counts

Java controller routes: 330
Rust Axum routes: 284
Upstream frontend API calls: 250
Local frontend API calls: 244
Upstream frontend pages: 24
Local frontend pages: 24

| Source | Count |
|---|---:|
| Java controller routes | 330 |
| Rust Axum routes | 284 |
| Upstream frontend API calls | 250 |
| Local frontend API calls | 244 |
| Upstream frontend pages | 24 |
| Local frontend pages | 24 |

## Comparisons

### Backend Java controllers → Rust router

| Status | Count |
|---|---:|
| Aligned | 222 |
| Missing | 105 |
| Method mismatch | 3 |
| Extra target entries | 57 |

#### Missing reference entries

- DELETE `/api/alarm/clear` (src/main/java/com/genersoft/iot/vmp/vmanager/alarm/AlarmController.java)
- DELETE `/api/alarm/delete` (src/main/java/com/genersoft/iot/vmp/vmanager/alarm/AlarmController.java)
- GET `/api/alarm/snap/{param}` (src/main/java/com/genersoft/iot/vmp/vmanager/alarm/AlarmController.java)
- GET `/api/cloud/record/collect/add` (src/main/java/com/genersoft/iot/vmp/vmanager/cloudRecord/CloudRecordController.java)
- GET `/api/cloud/record/collect/delete` (src/main/java/com/genersoft/iot/vmp/vmanager/cloudRecord/CloudRecordController.java)
- GET `/api/cloud/record/download/zip` (src/main/java/com/genersoft/iot/vmp/vmanager/cloudRecord/CloudRecordController.java)
- GET `/api/cloud/record/list-url` (src/main/java/com/genersoft/iot/vmp/vmanager/cloudRecord/CloudRecordController.java)
- GET `/api/cloud/record/zip` (src/main/java/com/genersoft/iot/vmp/vmanager/cloudRecord/CloudRecordController.java)
- GET `/api/common/channel/map/thin/tile/{param}/{param}/{param}` (src/main/java/com/genersoft/iot/vmp/gb28181/controller/ChannelController.java)
- GET `/api/common/channel/map/tile/{param}/{param}/{param}` (src/main/java/com/genersoft/iot/vmp/gb28181/controller/ChannelController.java)
- GET `/api/front-end/common/{param}/{param}` (src/main/java/com/genersoft/iot/vmp/gb28181/controller/PtzController.java)
- POST `/api/jt1078/area/circle/add` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/area/circle/delete` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- POST `/api/jt1078/area/circle/edit` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/area/circle/query` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- POST `/api/jt1078/area/circle/update` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/area/polygon/delete` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/area/polygon/query` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- POST `/api/jt1078/area/polygon/set` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- POST `/api/jt1078/area/rectangle/add` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/area/rectangle/delete` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- POST `/api/jt1078/area/rectangle/edit` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/area/rectangle/query` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- POST `/api/jt1078/area/rectangle/update` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- POST `/api/jt1078/confirmation-alarm-message` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/control/temp-position-tracking` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/live/continue` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/live/pause` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/live/switch` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/media/upload/one/delete` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/playback/download` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/record/start` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/record/stop` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/route/delete` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/route/query` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- POST `/api/jt1078/route/set` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- GET `/api/jt1078/snap` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java)
- DELETE `/api/jt1078/terminal/channel/delete` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078TerminalController.java)
- GET `/api/jt1078/terminal/channel/one` (src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078TerminalController.java)
- GET `/api/media/getPlayUrl` (src/main/java/com/genersoft/iot/vmp/gb28181/controller/MediaController.java)
- GET `/api/media/stream_info_by_app_and_stream` (src/main/java/com/genersoft/iot/vmp/gb28181/controller/MediaController.java)
- GET `/api/platform/info/{param}` (src/main/java/com/genersoft/iot/vmp/gb28181/controller/PlatformController.java)
- POST `/api/play/convertStop/{param}` (src/main/java/com/genersoft/iot/vmp/gb28181/controller/PlayController.java)
- GET `/api/play/snap` (src/main/java/com/genersoft/iot/vmp/gb28181/controller/PlayController.java)
- GET `/api/play/ssrc` (src/main/java/com/genersoft/iot/vmp/gb28181/controller/PlayController.java)
- GET `/api/position/latest` (src/main/java/com/genersoft/iot/vmp/gb28181/controller/MobilePositionController.java)
- GET `/api/position/realtime/{param}` (src/main/java/com/genersoft/iot/vmp/gb28181/controller/MobilePositionController.java)
- GET `/api/position/subscribe/{param}` (src/main/java/com/genersoft/iot/vmp/gb28181/controller/MobilePositionController.java)
- DELETE `/api/proxy/del` (src/main/java/com/genersoft/iot/vmp/streamProxy/controller/StreamProxyController.java)
- GET `/api/proxy/one` (src/main/java/com/genersoft/iot/vmp/streamProxy/controller/StreamProxyController.java)
- GET `/api/ps/getTestPort` (src/main/java/com/genersoft/iot/vmp/vmanager/ps/PsController.java)
- GET `/api/ps/receive/close` (src/main/java/com/genersoft/iot/vmp/vmanager/ps/PsController.java)
- GET `/api/ps/receive/open` (src/main/java/com/genersoft/iot/vmp/vmanager/ps/PsController.java)
- GET `/api/ps/send/start` (src/main/java/com/genersoft/iot/vmp/vmanager/ps/PsController.java)
- GET `/api/ps/send/stop` (src/main/java/com/genersoft/iot/vmp/vmanager/ps/PsController.java)
- GET `/api/push/forceClose` (src/main/java/com/genersoft/iot/vmp/streamPush/controller/StreamPushController.java)
- GET `/api/region/one` (src/main/java/com/genersoft/iot/vmp/gb28181/controller/RegionController.java)
- GET `/api/region/page/list` (src/main/java/com/genersoft/iot/vmp/gb28181/controller/RegionController.java)
- GET `/api/region/sync` (src/main/java/com/genersoft/iot/vmp/gb28181/controller/RegionController.java)
- POST `/api/role/add` (src/main/java/com/genersoft/iot/vmp/vmanager/user/RoleController.java)
- DELETE `/api/role/delete` (src/main/java/com/genersoft/iot/vmp/vmanager/user/RoleController.java)
- GET `/api/rtp/receive/close` (src/main/java/com/genersoft/iot/vmp/vmanager/rtp/RtpController.java)
- GET `/api/rtp/receive/open` (src/main/java/com/genersoft/iot/vmp/vmanager/rtp/RtpController.java)
- GET `/api/rtp/send/start` (src/main/java/com/genersoft/iot/vmp/vmanager/rtp/RtpController.java)
- GET `/api/rtp/send/stop` (src/main/java/com/genersoft/iot/vmp/vmanager/rtp/RtpController.java)
- GET `/api/server/config` (src/main/java/com/genersoft/iot/vmp/vmanager/server/ServerController.java)
- GET `/api/server/shutdown` (src/main/java/com/genersoft/iot/vmp/vmanager/server/ServerController.java)
- GET `/api/server/version` (src/main/java/com/genersoft/iot/vmp/vmanager/server/ServerController.java)
- GET `/api/sy/camera/cont-with-child` (src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java)
- GET `/api/sy/camera/control/play` (src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java)
- GET `/api/sy/camera/control/ptz` (src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java)
- GET `/api/sy/camera/control/stop` (src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java)
- GET `/api/sy/camera/list` (src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java)
- GET `/api/sy/camera/list-for-mobile` (src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java)
- GET `/api/sy/camera/list-with-child` (src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java)
- GET `/api/sy/camera/list/address` (src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java)
- GET `/api/sy/camera/list/box` (src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java)
- GET `/api/sy/camera/list/circle` (src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java)
- POST `/api/sy/camera/list/polygon` (src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java)
- GET `/api/sy/camera/meeting/list` (src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java)
- ... 25 more

| Method | Path | Source |
|---|---|---|
| DELETE | `/api/alarm/clear` | src/main/java/com/genersoft/iot/vmp/vmanager/alarm/AlarmController.java |
| DELETE | `/api/alarm/delete` | src/main/java/com/genersoft/iot/vmp/vmanager/alarm/AlarmController.java |
| GET | `/api/alarm/snap/{param}` | src/main/java/com/genersoft/iot/vmp/vmanager/alarm/AlarmController.java |
| GET | `/api/cloud/record/collect/add` | src/main/java/com/genersoft/iot/vmp/vmanager/cloudRecord/CloudRecordController.java |
| GET | `/api/cloud/record/collect/delete` | src/main/java/com/genersoft/iot/vmp/vmanager/cloudRecord/CloudRecordController.java |
| GET | `/api/cloud/record/download/zip` | src/main/java/com/genersoft/iot/vmp/vmanager/cloudRecord/CloudRecordController.java |
| GET | `/api/cloud/record/list-url` | src/main/java/com/genersoft/iot/vmp/vmanager/cloudRecord/CloudRecordController.java |
| GET | `/api/cloud/record/zip` | src/main/java/com/genersoft/iot/vmp/vmanager/cloudRecord/CloudRecordController.java |
| GET | `/api/common/channel/map/thin/tile/{param}/{param}/{param}` | src/main/java/com/genersoft/iot/vmp/gb28181/controller/ChannelController.java |
| GET | `/api/common/channel/map/tile/{param}/{param}/{param}` | src/main/java/com/genersoft/iot/vmp/gb28181/controller/ChannelController.java |
| GET | `/api/front-end/common/{param}/{param}` | src/main/java/com/genersoft/iot/vmp/gb28181/controller/PtzController.java |
| POST | `/api/jt1078/area/circle/add` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/area/circle/delete` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| POST | `/api/jt1078/area/circle/edit` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/area/circle/query` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| POST | `/api/jt1078/area/circle/update` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/area/polygon/delete` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/area/polygon/query` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| POST | `/api/jt1078/area/polygon/set` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| POST | `/api/jt1078/area/rectangle/add` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/area/rectangle/delete` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| POST | `/api/jt1078/area/rectangle/edit` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/area/rectangle/query` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| POST | `/api/jt1078/area/rectangle/update` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| POST | `/api/jt1078/confirmation-alarm-message` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/control/temp-position-tracking` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/live/continue` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/live/pause` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/live/switch` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/media/upload/one/delete` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/playback/download` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/record/start` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/record/stop` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/route/delete` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/route/query` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| POST | `/api/jt1078/route/set` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| GET | `/api/jt1078/snap` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078Controller.java |
| DELETE | `/api/jt1078/terminal/channel/delete` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078TerminalController.java |
| GET | `/api/jt1078/terminal/channel/one` | src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078TerminalController.java |
| GET | `/api/media/getPlayUrl` | src/main/java/com/genersoft/iot/vmp/gb28181/controller/MediaController.java |
| GET | `/api/media/stream_info_by_app_and_stream` | src/main/java/com/genersoft/iot/vmp/gb28181/controller/MediaController.java |
| GET | `/api/platform/info/{param}` | src/main/java/com/genersoft/iot/vmp/gb28181/controller/PlatformController.java |
| POST | `/api/play/convertStop/{param}` | src/main/java/com/genersoft/iot/vmp/gb28181/controller/PlayController.java |
| GET | `/api/play/snap` | src/main/java/com/genersoft/iot/vmp/gb28181/controller/PlayController.java |
| GET | `/api/play/ssrc` | src/main/java/com/genersoft/iot/vmp/gb28181/controller/PlayController.java |
| GET | `/api/position/latest` | src/main/java/com/genersoft/iot/vmp/gb28181/controller/MobilePositionController.java |
| GET | `/api/position/realtime/{param}` | src/main/java/com/genersoft/iot/vmp/gb28181/controller/MobilePositionController.java |
| GET | `/api/position/subscribe/{param}` | src/main/java/com/genersoft/iot/vmp/gb28181/controller/MobilePositionController.java |
| DELETE | `/api/proxy/del` | src/main/java/com/genersoft/iot/vmp/streamProxy/controller/StreamProxyController.java |
| GET | `/api/proxy/one` | src/main/java/com/genersoft/iot/vmp/streamProxy/controller/StreamProxyController.java |
| GET | `/api/ps/getTestPort` | src/main/java/com/genersoft/iot/vmp/vmanager/ps/PsController.java |
| GET | `/api/ps/receive/close` | src/main/java/com/genersoft/iot/vmp/vmanager/ps/PsController.java |
| GET | `/api/ps/receive/open` | src/main/java/com/genersoft/iot/vmp/vmanager/ps/PsController.java |
| GET | `/api/ps/send/start` | src/main/java/com/genersoft/iot/vmp/vmanager/ps/PsController.java |
| GET | `/api/ps/send/stop` | src/main/java/com/genersoft/iot/vmp/vmanager/ps/PsController.java |
| GET | `/api/push/forceClose` | src/main/java/com/genersoft/iot/vmp/streamPush/controller/StreamPushController.java |
| GET | `/api/region/one` | src/main/java/com/genersoft/iot/vmp/gb28181/controller/RegionController.java |
| GET | `/api/region/page/list` | src/main/java/com/genersoft/iot/vmp/gb28181/controller/RegionController.java |
| GET | `/api/region/sync` | src/main/java/com/genersoft/iot/vmp/gb28181/controller/RegionController.java |
| POST | `/api/role/add` | src/main/java/com/genersoft/iot/vmp/vmanager/user/RoleController.java |
| DELETE | `/api/role/delete` | src/main/java/com/genersoft/iot/vmp/vmanager/user/RoleController.java |
| GET | `/api/rtp/receive/close` | src/main/java/com/genersoft/iot/vmp/vmanager/rtp/RtpController.java |
| GET | `/api/rtp/receive/open` | src/main/java/com/genersoft/iot/vmp/vmanager/rtp/RtpController.java |
| GET | `/api/rtp/send/start` | src/main/java/com/genersoft/iot/vmp/vmanager/rtp/RtpController.java |
| GET | `/api/rtp/send/stop` | src/main/java/com/genersoft/iot/vmp/vmanager/rtp/RtpController.java |
| GET | `/api/server/config` | src/main/java/com/genersoft/iot/vmp/vmanager/server/ServerController.java |
| GET | `/api/server/shutdown` | src/main/java/com/genersoft/iot/vmp/vmanager/server/ServerController.java |
| GET | `/api/server/version` | src/main/java/com/genersoft/iot/vmp/vmanager/server/ServerController.java |
| GET | `/api/sy/camera/cont-with-child` | src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java |
| GET | `/api/sy/camera/control/play` | src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java |
| GET | `/api/sy/camera/control/ptz` | src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java |
| GET | `/api/sy/camera/control/stop` | src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java |
| GET | `/api/sy/camera/list` | src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java |
| GET | `/api/sy/camera/list-for-mobile` | src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java |
| GET | `/api/sy/camera/list-with-child` | src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java |
| GET | `/api/sy/camera/list/address` | src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java |
| GET | `/api/sy/camera/list/box` | src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java |
| GET | `/api/sy/camera/list/circle` | src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java |
| POST | `/api/sy/camera/list/polygon` | src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java |
| GET | `/api/sy/camera/meeting/list` | src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java |
| ... | 25 more | ... |

#### Extra target entries

| Method | Path | Source |
|---|---|---|
| DELETE | `/api/alarm/batch` | src/router.rs |
| DELETE | `/api/alarm/before/{param}` | src/router.rs |
| DELETE | `/api/alarm/delete/{param}` | src/router.rs |
| GET | `/api/alarm/detail/{param}` | src/router.rs |
| DELETE | `/api/alarm/device/{param}` | src/router.rs |
| POST | `/api/alarm/handle` | src/router.rs |
| POST | `/api/common/channel/map/save-level` | src/router.rs |
| POST | `/api/device/batch/control` | src/router.rs |
| GET | `/api/device/config/query` | src/router.rs |
| GET | `/api/device/config/query/{param}/BasicParam` | src/router.rs |
| POST | `/api/device/config/update` | src/router.rs |
| GET | `/api/device/control/guard` | src/router.rs |
| GET | `/api/device/control/preset` | src/router.rs |
| GET | `/api/device/control/ptz` | src/router.rs |
| GET | `/api/device/control/reboot` | src/router.rs |
| GET | `/api/device/control/record` | src/router.rs |
| POST | `/api/device/query/channel/audio` | src/router.rs |
| GET | `/api/device/query/channel/one` | src/router.rs |
| POST | `/api/device/query/channel/stream/identification/update` | src/router.rs |
| POST | `/api/device/query/device/add` | src/router.rs |
| POST | `/api/device/query/device/update` | src/router.rs |
| GET | `/api/device/query/devices` | src/router.rs |
| GET | `/api/device/query/devices/{param}` | src/router.rs |
| GET | `/api/device/query/devices/{param}/channels` | src/router.rs |
| DELETE | `/api/device/query/devices/{param}/delete` | src/router.rs |
| GET | `/api/device/query/devices/{param}/sync` | src/router.rs |
| GET | `/api/device/query/streams` | src/router.rs |
| GET | `/api/device/query/sub_channels/{param}/{param}/channels` | src/router.rs |
| GET | `/api/device/query/subscribe/catalog` | src/router.rs |
| GET | `/api/device/query/subscribe/mobile-position` | src/router.rs |
| GET | `/api/device/query/sync_status` | src/router.rs |
| POST | `/api/device/query/transport/{param}/{param}` | src/router.rs |
| GET | `/api/device/query/tree/{param}` | src/router.rs |
| GET | `/api/device/query/tree/channel/{param}` | src/router.rs |
| GET | `/api/health` | src/router.rs |
| POST | `/api/platform/catalog/add` | src/router.rs |
| POST | `/api/platform/catalog/edit` | src/router.rs |
| POST | `/api/play/webrtc` | src/router.rs |
| POST | `/api/proxy/save` | src/router.rs |
| POST | `/api/ptz/front_end_command/{param}/{param}` | src/router.rs |
| DELETE | `/api/push/remove_form_gb` | src/router.rs |
| POST | `/api/push/save_to_gb` | src/router.rs |
| GET | `/api/region/queryChildListInBase` | src/router.rs |
| POST | `/api/talk/ack` | src/router.rs |
| POST | `/api/talk/bye` | src/router.rs |
| GET | `/api/talk/invite/{param}/{param}` | src/router.rs |
| GET | `/api/talk/list` | src/router.rs |
| GET | `/api/talk/start/{param}/{param}` | src/router.rs |
| GET | `/api/talk/status/{param}/{param}` | src/router.rs |
| GET | `/api/talk/stop/{param}/{param}` | src/router.rs |
| GET | `/api/user/logout` | src/router.rs |
| GET | `/api/user/userInfo` | src/router.rs |
| GET | `/api/ws` | src/router.rs |
| POST | `/api/zlm/hook` | src/router.rs |
| GET | `/metrics` | src/router.rs |
| GET | `/zlm/{param}/*path` | src/router.rs |
| POST | `/zlm/{param}/*path` | src/router.rs |

#### Method mismatches

| Path | Reference methods | Target methods |
|---|---|---|
| `/api/play/broadcast/{param}/{param}` | GET, POST | GET |
| `/api/play/broadcast/stop/{param}/{param}` | GET, POST | GET |
| `/api/sy/camera/list/ids` | POST | GET |

### Official frontend API → Rust router

| Status | Count |
|---|---:|
| Aligned | 244 |
| Missing | 6 |
| Method mismatch | 0 |
| Extra target entries | 37 |

#### Missing reference entries

- DELETE `/api/alarm/clear` (web/src/api/alarm.js)
- DELETE `/api/alarm/delete` (web/src/api/alarm.js)
- GET `/api/device/query/statistics/keepalive` (web/src/api/device.js)
- GET `/api/device/query/statistics/register` (web/src/api/device.js)
- GET `/api/device/query/subscribe/alarm` (web/src/api/device.js)
- GET `/vue-admin-template/table/list` (web/src/api/table.js)

| Method | Path | Source |
|---|---|---|
| DELETE | `/api/alarm/clear` | web/src/api/alarm.js |
| DELETE | `/api/alarm/delete` | web/src/api/alarm.js |
| GET | `/api/device/query/statistics/keepalive` | web/src/api/device.js |
| GET | `/api/device/query/statistics/register` | web/src/api/device.js |
| GET | `/api/device/query/subscribe/alarm` | web/src/api/device.js |
| GET | `/vue-admin-template/table/list` | web/src/api/table.js |

#### Extra target entries

| Method | Path | Source |
|---|---|---|
| DELETE | `/api/alarm/batch` | src/router.rs |
| DELETE | `/api/alarm/before/{param}` | src/router.rs |
| DELETE | `/api/alarm/delete/{param}` | src/router.rs |
| GET | `/api/alarm/detail/{param}` | src/router.rs |
| DELETE | `/api/alarm/device/{param}` | src/router.rs |
| POST | `/api/alarm/handle` | src/router.rs |
| POST | `/api/device/batch/control` | src/router.rs |
| GET | `/api/device/config/query` | src/router.rs |
| POST | `/api/device/config/update` | src/router.rs |
| GET | `/api/device/control/preset` | src/router.rs |
| GET | `/api/device/control/ptz` | src/router.rs |
| GET | `/api/device/control/reboot` | src/router.rs |
| GET | `/api/health` | src/router.rs |
| GET | `/api/jt1078/media/upload/one/upload` | src/router.rs |
| GET | `/api/log/file/{param}` | src/router.rs |
| POST | `/api/platform/catalog/add` | src/router.rs |
| POST | `/api/platform/catalog/edit` | src/router.rs |
| POST | `/api/play/webrtc` | src/router.rs |
| GET | `/api/playback/seek/{param}/{param}` | src/router.rs |
| GET | `/api/position/history/{param}` | src/router.rs |
| POST | `/api/ptz/front_end_command/{param}/{param}` | src/router.rs |
| POST | `/api/push/upload` | src/router.rs |
| GET | `/api/region/queryChildListInBase` | src/router.rs |
| POST | `/api/talk/ack` | src/router.rs |
| POST | `/api/talk/bye` | src/router.rs |
| GET | `/api/talk/invite/{param}/{param}` | src/router.rs |
| GET | `/api/talk/list` | src/router.rs |
| GET | `/api/talk/start/{param}/{param}` | src/router.rs |
| GET | `/api/talk/status/{param}/{param}` | src/router.rs |
| GET | `/api/talk/stop/{param}/{param}` | src/router.rs |
| POST | `/api/user/login` | src/router.rs |
| GET | `/api/user/userInfo` | src/router.rs |
| GET | `/api/ws` | src/router.rs |
| POST | `/api/zlm/hook` | src/router.rs |
| GET | `/metrics` | src/router.rs |
| GET | `/zlm/{param}/*path` | src/router.rs |
| POST | `/zlm/{param}/*path` | src/router.rs |

### Official frontend API → local frontend API

| Status | Count |
|---|---:|
| Aligned | 244 |
| Missing | 6 |
| Method mismatch | 0 |
| Extra target entries | 0 |

#### Missing reference entries

- DELETE `/api/alarm/clear` (web/src/api/alarm.js)
- DELETE `/api/alarm/delete` (web/src/api/alarm.js)
- GET `/api/alarm/list` (web/src/api/alarm.js)
- GET `/api/device/query/statistics/keepalive` (web/src/api/device.js)
- GET `/api/device/query/statistics/register` (web/src/api/device.js)
- GET `/api/device/query/subscribe/alarm` (web/src/api/device.js)

| Method | Path | Source |
|---|---|---|
| DELETE | `/api/alarm/clear` | web/src/api/alarm.js |
| DELETE | `/api/alarm/delete` | web/src/api/alarm.js |
| GET | `/api/alarm/list` | web/src/api/alarm.js |
| GET | `/api/device/query/statistics/keepalive` | web/src/api/device.js |
| GET | `/api/device/query/statistics/register` | web/src/api/device.js |
| GET | `/api/device/query/subscribe/alarm` | web/src/api/device.js |

### Official frontend pages → local frontend pages

| Status | Count |
|---|---:|
| Aligned | 22 |
| Missing | 2 |
| Method mismatch | 0 |
| Extra target entries | 2 |

#### Missing reference entries

- PAGE `/alarm` (web/src/router/index.js)
- PAGE `/play/share` (web/src/router/index.js)

| Method | Path | Source |
|---|---|---|
| PAGE | `/alarm` | web/src/router/index.js |
| PAGE | `/play/share` | web/src/router/index.js |

#### Extra target entries

| Method | Path | Source |
|---|---|---|
| PAGE | `/play/rtc/{param}` | web/src/router/index.js |
| PAGE | `/play/wasm/{param}` | web/src/router/index.js |

## Status policy

- `aligned`: same canonical path and method/page exists.
- `missing`: reference entry is absent from the target.
- `method mismatch`: canonical path exists but HTTP methods differ.
- `extra target entries`: target has entries not found in the reference; these may be extensions or obsolete local APIs.

This report is a route and frontend-surface audit. Protocol-production status must be assigned in follow-up Phase 1+ implementation plans and tests.

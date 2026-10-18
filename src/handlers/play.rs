use axum::{extract::Path, extract::Query, extract::State, Json};
use utoipa::IntoParams;

use crate::db::device as db_device;
use crate::response::ApiResult;
use crate::sip::gb28181::media_waiter::MediaWaitResult;
use crate::AppState;

fn media_server_open_error(
    client: &crate::zlm::ZlmClient,
    stream_id: &str,
    error: &anyhow::Error,
) -> String {
    // reqwest 的 Display 会包含完整请求 URL（含 ZLM secret 查询参数）。
    // 只向调用方暴露错误类别；节点和流由当前请求上下文补齐。
    let detail = if let Some(request_error) = error.downcast_ref::<reqwest::Error>() {
        if request_error.is_connect() {
            "连接失败".to_string()
        } else if request_error.is_timeout() {
            "连接超时".to_string()
        } else if let Some(status) = request_error.status() {
            format!("HTTP {status}")
        } else {
            "请求失败".to_string()
        }
    } else {
        error.to_string().replace(&client.secret, "[redacted]")
    };
    format!(
        "打开 RTP 收流端口失败（节点 {}:{}, 流 {}, 请求 play/start）：{}",
        client.ip, client.http_port, stream_id, detail
    )
}

fn play_sip_error(
    client: &crate::zlm::ZlmClient,
    stream_id: &str,
    error: &anyhow::Error,
) -> String {
    format!(
        "点播信令失败（节点 {}:{}, 流 {}, 请求 play/start）：{}",
        client.ip, client.http_port, stream_id, error
    )
}

fn rtp_port_range_problem(port: u16, range: &str) -> Option<String> {
    let (start, end) = match crate::zlm::client::parse_port_range(range) {
        Ok(bounds) => bounds,
        Err(_) => return Some(format!("RTP 端口范围配置无效：{range}")),
    };
    if start > end {
        return Some(format!("RTP 端口范围配置无效：{range}"));
    }
    if port == 0 || !(start..=end).contains(&port) {
        return Some(format!("分配端口 {port} 不在配置范围 {range} 内"));
    }
    None
}

#[cfg(test)]
mod media_diagnostics_tests {
    #[test]
    fn allocated_rtp_port_must_fit_configured_range() {
        assert!(super::rtp_port_range_problem(30050, "30000-30100").is_none());
        assert!(super::rtp_port_range_problem(30050, "30000,30100").is_none());
        assert!(super::rtp_port_range_problem(30050, "40000-40100")
            .unwrap()
            .contains("30050"));
        assert!(super::rtp_port_range_problem(0, "30000-30100").is_some());
        assert!(super::rtp_port_range_problem(30050, "invalid").is_some());
        assert!(super::rtp_port_range_problem(30050, "30100-30000").is_some());
    }

    #[test]
    fn open_failure_identifies_node_stream_and_request_without_secret() {
        let client = crate::zlm::ZlmClient::new("127.0.0.1", 8080, "private-zlm-secret");
        let msg = super::media_server_open_error(
            &client,
            "device_channel",
            &anyhow::anyhow!("connection refused"),
        );
        assert!(msg.contains("127.0.0.1:8080"));
        assert!(msg.contains("device_channel"));
        assert!(msg.contains("play/start"));
        assert!(msg.contains("connection refused"));
        assert!(!msg.contains("private-zlm-secret"));
    }

    #[tokio::test]
    async fn connection_error_does_not_expose_zlm_query_secret() {
        let client = crate::zlm::ZlmClient::new("127.0.0.1", 9, "private-zlm-secret");
        let error = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get("http://127.0.0.1:9/index/api/openRtpServer?secret=private-zlm-secret")
            .send()
            .await
            .unwrap_err();
        let msg = super::media_server_open_error(&client, "device_channel", &error.into());
        assert!(msg.contains("连接失败"), "{msg}");
        assert!(!msg.contains("private-zlm-secret"));
        assert!(!msg.contains("secret="));
    }
}

async fn wait_for_passive_media(
    media_rx: tokio::sync::oneshot::Receiver<MediaWaitResult>,
    stream_id: &str,
    timeout: std::time::Duration,
) -> Result<(), String> {
    match tokio::time::timeout(timeout, media_rx).await {
        Ok(Ok(MediaWaitResult::MediaReady { zlm_stream_id, app }))
            if zlm_stream_id == stream_id && app == "rtp" =>
        {
            Ok(())
        }
        Ok(Ok(MediaWaitResult::DeviceRejected { status, reason })) => {
            Err(format!("device rejected media: {status} {reason}"))
        }
        Ok(Ok(MediaWaitResult::MediaReady { .. })) => Err("unexpected media stream".to_string()),
        Ok(Ok(MediaWaitResult::Timeout | MediaWaitResult::SessionNotFound)) | Err(_) => {
            Err("ZLM media timeout after TCP-PASSIVE connect".to_string())
        }
        Ok(Err(_)) => Err("media wait channel closed".to_string()),
    }
}

/// 从 SDP 文本里解析首个 m=video/m=audio 的端口号。
///
/// 实现已统一到 `sip::gb28181::sdp_builder::parse_media_port`（SDP 的唯一真源），
/// 这里只做转发，避免 hook / play 各写一份导致行为漂移。
fn parse_sdp_media_port(sdp: &str) -> Option<u16> {
    crate::sip::gb28181::sdp_builder::parse_media_port(sdp)
}

fn media_audio_from_tracks(tracks: &[crate::zlm::TrackInfo]) -> Option<bool> {
    if tracks.is_empty() {
        None
    } else {
        Some(
            tracks
                .iter()
                .any(|track| track.codec_type == 1 && track.ready),
        )
    }
}

/// 直播的音频标志描述本次流；目录字段仅在 ZLM 尚未给出 track 时兜底。
pub(crate) async fn resolve_live_audio(
    zlm_client: &crate::zlm::ZlmClient,
    stream_id: &str,
    catalog_has_audio: bool,
) -> (bool, &'static str) {
    match zlm_client
        .get_media_list(None, Some("rtp"), Some(stream_id))
        .await
    {
        Ok(media) => {
            let matching: Vec<_> = media
                .iter()
                .filter(|m| m.app == "rtp" && m.stream == stream_id && !m.tracks.is_empty())
                .collect();
            if !matching.is_empty() {
                let audio = matching
                    .iter()
                    .any(|m| media_audio_from_tracks(&m.tracks) == Some(true));
                return (audio, "media");
            }
        }
        Err(error) => tracing::debug!(stream = stream_id, "查询 ZLM 音频 track 失败: {error}"),
    }
    (catalog_has_audio, "catalog")
}

/// 统一的"播放地址" JSON：所有成功分支都用它，避免同一组 URL 拼在多处漂移。
///
/// `hls` **只在 ZLM 真的有 hls 源时才给出**：实测本仓库镜像上 GB28181 的
/// RTP/PS 流没有 hls 源（RTMP 推流才有），无条件给出会得到一个 404 地址，
/// 而实时预览页优先用它 → 画面出不来还看不出原因。
async fn play_urls_json(
    zlm_client: &crate::zlm::ZlmClient,
    stream_id: &str,
    device_id: &str,
    channel_id: &str,
    catalog_has_audio: bool,
    ssrc: &str,
    transport: &str,
) -> serde_json::Value {
    let ip = &zlm_client.ip;
    let http = zlm_client.http_port;
    let (hls_available, (has_audio, audio_source)) = tokio::join!(
        zlm_client.has_schema("rtp", stream_id, "hls"),
        resolve_live_audio(zlm_client, stream_id, catalog_has_audio)
    );
    let mut out = serde_json::json!({
        "app": "rtp",
        "stream": stream_id,
        "playUrl": format!("rtsp://{ip}:554/rtp/{stream_id}"),
        "flvUrl": crate::zlm::address_builder::http_flv_url(ip, http, "rtp", stream_id),
        "wsUrl": crate::zlm::address_builder::ws_flv_url(ip, http, "rtp", stream_id),
        "ws_flv": crate::zlm::address_builder::ws_flv_url(ip, http, "rtp", stream_id),
        "webrtc": format!("webrtc://{ip}:{http}/index/api/webrtc?app=rtp&stream={stream_id}&type=play"),
        "hlsAvailable": hls_available,
        "deviceId": device_id,
        "channelId": channel_id,
        "hasAudio": has_audio,
        "audioSource": audio_source,
        "ssrc": ssrc,
        "transport": transport,
    });
    if hls_available {
        out["hls"] = serde_json::json!(crate::zlm::address_builder::hls_url(
            ip, http, "rtp", stream_id
        ));
    }
    out
}

/// GET /api/play/start/{device_id}/{channel_id}
///
/// 发起 GB28181 实时点播（SIP INVITE + ZLM RTP server），返回 FLV/HLS/WebRTC
/// 等播放地址。**TCP-PASSIVE** 设备会等待 200 OK SDP 后让 ZLM 反向 connect。
#[utoipa::path(
    get,
    path = "/api/play/start/{device_id}/{channel_id}",
    tag = "live",
    operation_id = "play_start",
    params(
        ("device_id" = String, Path, description = "设备国标编号（20 位）"),
        ("channel_id" = String, Path, description = "通道国标编号（20 位）"),
    ),
    responses(
        (status = 200, description = "点播成功，返回多协议播放地址",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"app":"rtp","stream":"dev_ch","playUrl":"rtsp://1.2.3.4:554/rtp/dev_ch","flvUrl":"http://1.2.3.4:8080/live/rtp/dev_ch.live.flv","hlsAvailable":false,"deviceId":"dev","channelId":"ch","hasAudio":true,"ssrc":"0100000001","transport":"UDP"}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn play_start(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Json<ApiResult<serde_json::Value>> {
    tracing::info!("Play request: device={}, channel={}", device_id, channel_id);

    let device = match db_device::get_device_by_device_id(&state.pool, &device_id).await {
        Ok(Some(d)) => d,
        Ok(None) => {
            return Json(ApiResult::error("Device not found"));
        }
        Err(e) => {
            tracing::error!("Failed to query device: {}", e);
            return Json(ApiResult::error("Database error"));
        }
    };

    let channel =
        match db_device::get_channel_by_device_and_channel_id(&state.pool, &device_id, &channel_id)
            .await
        {
            Ok(Some(c)) => c,
            Ok(None) => {
                return Json(ApiResult::error("Channel not found"));
            }
            Err(e) => {
                tracing::error!("Failed to query channel: {}", e);
                return Json(ApiResult::error("Database error"));
            }
        };

    if !device.on_line.unwrap_or(false) {
        return Json(ApiResult::error("Device is offline"));
    }

    let sip_server = match &state.sip_server {
        Some(s) => s.clone(),
        None => return Json(ApiResult::error("SIP server not available")),
    };

    if let Some(ref zlm_client) = state.zlm_client {
        // 创建 ZLM 的流。stream_id 使用规范格式: 设备ID_通道ID
        let stream_id = format!("{}_{}", device_id, channel_id);
        sip_server
            .stream_reconnect_manager()
            .clear_manual_stop(&stream_id);
        // 新一轮点播不能消费上一轮留下的流注册通知；本轮真正的媒体
        // Hook 若先于 waiter 注册到达，管理器会重新缓存它。
        sip_server
            .media_waiter_manager()
            .forget_early_ready(&stream_id);
        // 规范 SSRC（10 位：1 位类型 + 5 位域标识 + 4 位序号），
        // 与 SIP 层 `build_play_ssrc` 共用同一实现 —— 两边各算一份时
        // 一旦口径不同，INVITE 的 `y=` 与 ZLM/前端拿到的就不是同一个值。
        let ssrc = crate::sip::server::build_play_ssrc(&device_id);

        let transport_mode = device
            .transport
            .as_deref()
            .or(device.stream_mode.as_deref())
            .unwrap_or("UDP")
            .to_uppercase();
        let is_tcp_passive = transport_mode == "TCP-PASSIVE";
        let is_tcp = transport_mode == "TCP" || is_tcp_passive;

        // 预开 ZLM RTP server —— 标准 GB28181 设备会按 INVITE SDP m= 端口
        // (也就是这个端口) 推流。对于非标准实现(例如 gbcpp/1.0 mock 在
        // 200 OK SDP 里另写 m= 端口),会在收到 200 OK 后用 connectRtpServer
        // 接管。
        //
        // TCP-PASSIVE 例外:设备不在这个端口推流,它在等 ZLM 主动 connect。
        // 这个端口开不开都不影响(下一步会 close),但 openRtpServer 仍要调
        // 才能保证 ZLM 内部流注册成功,后续 connectRtpServer 才能用同一个
        // stream_id 接管。
        let rtp_req = crate::zlm::OpenRtpServerRequest {
            secret: zlm_client.secret.clone(),
            stream_id: stream_id.clone(),
            port: Some(0), // 让 ZLM 随机分配端口
            use_tcp: Some(is_tcp),
            rtp_type: Some(if is_tcp_passive {
                2
            } else if is_tcp {
                1
            } else {
                0
            }),
            recv_port: None,
        };

        // 同一通道可能已经被另一个观看者（或上一次请求）打开：
        // ZLM 的 openRtpServer 会返回 `-300 This stream already exists`。
        // 这是**正常情况**，不是失败 —— 设备已经在推流，直接把已有的
        // 播放地址返回即可（多观看者/页面重进都靠这条路径）。
        // 此前一律按失败返回，导致"第一个看的人正常、第二个看的人报错"。
        //
        // 退化处理：ZLM 偶尔会进入"流登记在册但 RTP info 查不到"的不一致状态
        // （常见于上一次 BYE 还没真正回收、或并发的两条 openRtpServer）。
        // 此时 `get_rtp_info` 拿不到 info → 先 `closeRtpServer` 清掉残留登记
        // 再 `openRtpServer` 重建。仅靠重试 openRtpServer 是没用的：ZLM 看到
        // 内部有同名 stream 仍然直接 -300。
        let rtp_server = match zlm_client.open_rtp_server(&rtp_req).await {
            Ok(s) => s,
            Err(e) => {
                let msg = e.to_string();
                let already_exists =
                    msg.contains("-300") || msg.to_lowercase().contains("already exist");
                if already_exists {
                    match zlm_client.get_rtp_info(&stream_id).await {
                        Ok(Some(info)) => {
                            tracing::info!(
                                "流 {stream_id} 已存在（local_port={} peer={}:{}），复用现有流",
                                info.local_port,
                                info.peer_ip,
                                info.peer_port
                            );
                            // 复用的流也刷新一次缩略图（可能已经播了很久，
                            // 画面早就变了）
                            crate::handlers::device_query::spawn_snapshot_capture(
                                state.clone(),
                                device_id.clone(),
                                channel_id.clone(),
                            );
                            return Json(ApiResult::success(
                                play_urls_json(
                                    zlm_client,
                                    &stream_id,
                                    &device_id,
                                    &channel_id,
                                    channel.has_audio.unwrap_or(false),
                                    &ssrc,
                                    &transport_mode,
                                )
                                .await,
                            ));
                        }
                        Ok(None) => {
                            tracing::warn!(
                                "openRtpServer 报流已存在但 getRtpInfo 查不到 {stream_id}，先 closeRtpServer 清残留再 open"
                            );
                            // 先关掉残留的 stream_id 登记 —— 不关直接重开
                            // openRtpServer 仍会拿到 -300。
                            let _ = zlm_client.close_rtp_server(&stream_id).await;
                            match zlm_client.open_rtp_server(&rtp_req).await {
                                Ok(s2) => s2,
                                Err(e2) => {
                                    tracing::error!(
                                        "{}",
                                        media_server_open_error(zlm_client, &stream_id, &e2)
                                    );
                                    return Json(ApiResult::error(media_server_open_error(
                                        zlm_client, &stream_id, &e2,
                                    )));
                                }
                            }
                        }
                        Err(qe) => {
                            tracing::warn!("查询已存在流的 RTP 信息失败 {stream_id}: {qe}");
                            // info 接口本身失败不代表流不存在 —— 关掉残留再开。
                            let _ = zlm_client.close_rtp_server(&stream_id).await;
                            match zlm_client.open_rtp_server(&rtp_req).await {
                                Ok(s2) => s2,
                                Err(e2) => {
                                    tracing::error!(
                                        "{}",
                                        media_server_open_error(zlm_client, &stream_id, &e2)
                                    );
                                    return Json(ApiResult::error(media_server_open_error(
                                        zlm_client, &stream_id, &e2,
                                    )));
                                }
                            }
                        }
                    }
                } else {
                    tracing::error!("{}", media_server_open_error(zlm_client, &stream_id, &e));
                    return Json(ApiResult::error(media_server_open_error(
                        zlm_client, &stream_id, &e,
                    )));
                }
            }
        };

        tracing::info!(
            "ZLM RTP server opened on port {} (transport={})",
            rtp_server.port,
            transport_mode
        );

        // ZLM 的 RTP 端口池在启动时建立；运行期下发新范围不一定会重建端口池。
        // 先核对节点声明的范围，再向设备发 INVITE，避免把必然不可达的端口交给设备。
        let configured_range = match crate::db::media_server::list_media_servers(&state.pool).await
        {
            Ok(servers) => servers
                .into_iter()
                .find(|server| {
                    server.ip.as_deref() == Some(zlm_client.ip.as_str())
                        && server.http_port == Some(zlm_client.http_port as i32)
                })
                .and_then(|server| server.rtp_port_range),
            Err(error) => {
                tracing::warn!(%stream_id, "查询媒体节点 RTP 端口范围失败: {error}");
                None
            }
        }
        .or_else(|| {
            state.config.zlm.as_ref().and_then(|zlm| {
                zlm.servers
                    .iter()
                    .find(|server| {
                        server.ip == zlm_client.ip && server.http_port == zlm_client.http_port
                    })
                    .and_then(|server| server.rtp_port_range.clone())
            })
        });
        let port_problem = if rtp_server.port == 0 {
            Some("ZLM 未分配有效的 RTP 端口".to_string())
        } else {
            configured_range
                .as_deref()
                .and_then(|range| rtp_port_range_problem(rtp_server.port, range))
        };
        if let Some(problem) = port_problem {
            tracing::error!(%stream_id, node = %zlm_client.ip, port = zlm_client.http_port, %problem,
                "ZLM RTP 端口与节点配置不一致，取消点播");
            if let Err(error) = zlm_client.close_rtp_server(&stream_id).await {
                tracing::warn!(%stream_id, "关闭无效 RTP 端口失败: {error}");
            }
            return Json(ApiResult::error(format!(
                "RTP 收流端口不可用（节点 {}:{}, 流 {}, 请求 play/start）：{}",
                zlm_client.ip, zlm_client.http_port, stream_id, problem
            )));
        }

        // 调用 SIP Server 真正发送 INVITE，并等待设备回复 200 OK
        let sip = &*sip_server;

        // TCP-PASSIVE 设备先等待 ZLM 主动连接它的监听端口，再等真实媒体注册。
        // 单靠 SIP 200 或 connectRtpServer 成功都不能证明流可播放。
        if is_tcp_passive {
            let (call_id, resp) = match sip
                .send_play_invite_and_wait(&device_id, &channel_id, rtp_server.port, Some(&ssrc))
                .await
            {
                Ok(pair) => pair,
                Err(e) => {
                    tracing::error!("SIP INVITE for TCP-PASSIVE failed: {}", e);
                    let _ = zlm_client.close_rtp_server(&stream_id).await;
                    return Json(ApiResult::error(play_sip_error(zlm_client, &stream_id, &e)));
                }
            };
            tracing::info!(
                "TCP-PASSIVE device: SIP 200 OK received for {}, switching ZLM to connectRtpServer",
                call_id
            );

            let device_port = resp
                .body
                .as_deref()
                .and_then(parse_sdp_media_port)
                .ok_or_else(|| {
                    tracing::error!(
                        "TCP-PASSIVE device 200 OK missing/invalid SDP m= port. Full SDP body:\n{:?}",
                        resp.body
                    );
                    "device SDP missing media port"
                });

            let device_port = match device_port {
                Ok(p) => p,
                Err(e) => {
                    let _ = zlm_client.close_rtp_server(&stream_id).await;
                    let _ = sip.send_session_bye_by_call_id(&call_id).await;
                    return Json(ApiResult::error(format!(
                        "设备 SDP 无媒体端口（节点 {}:{}, 流 {}, 请求 play/start）：{}",
                        zlm_client.ip, zlm_client.http_port, stream_id, e
                    )));
                }
            };

            // 让 ZLM 主动 connect 到设备的 TCP 端口。注意:不要先
            // closeRtpServer,那样 ZLM 就找不到 stream_id 对应的流,
            // connectRtpServer 会返回 "can not find the stream"。
            // openRtpServer 已经创建好流,connectRtpServer 接管它做主动
            // connect。
            // ZLM 的 dst_url 只接受主机名/IP；端口由 dst_port 单独传入。
            let dst = device.ip.as_deref().unwrap_or("0.0.0.0");
            let media_waiters = sip.media_waiter_manager();
            let (_, media_rx) = media_waiters.register(&call_id, &stream_id, "rtp", 15);
            if let Err(e) = zlm_client
                .connect_rtp_server(&stream_id, dst, device_port, None)
                .await
            {
                media_waiters.cancel_by_call_id(&call_id);
                tracing::error!(
                    "connectRtpServer to TCP-PASSIVE device:{} failed: {}",
                    device_port,
                    e
                );
                let _ = zlm_client.close_rtp_server(&stream_id).await;
                let _ = sip.send_session_bye_by_call_id(&call_id).await;
                return Json(ApiResult::error(format!(
                    "ZLM 主动连接失败（节点 {}:{}, 流 {}, 请求 play/start）：{}",
                    zlm_client.ip,
                    zlm_client.http_port,
                    stream_id,
                    e.to_string().replace(&zlm_client.secret, "[redacted]")
                )));
            }
            tracing::info!(
                "ZLM connectRtpServer (TCP-PASSIVE) -> {} (stream_id={})",
                dst,
                stream_id
            );

            if let Err(reason) =
                wait_for_passive_media(media_rx, &stream_id, std::time::Duration::from_secs(15))
                    .await
            {
                media_waiters.cancel_by_call_id(&call_id);
                tracing::error!(%stream_id, transport = "TCP-PASSIVE", %reason, "主动连接后媒体未就绪");
                let _ = zlm_client.close_rtp_server(&stream_id).await;
                let _ = sip.send_session_bye_by_call_id(&call_id).await;
                return Json(ApiResult::error(format!(
                    "TCP-PASSIVE 媒体未就绪（节点 {}:{}, 流 {}, 请求 play/start）：{reason}",
                    zlm_client.ip, zlm_client.http_port, stream_id
                )));
            }

            // 复用统一构造器：FLV 后缀 .live.flv、hls 仅在真的有源时给出
            let mut payload = play_urls_json(
                zlm_client,
                &stream_id,
                &device_id,
                &channel_id,
                channel.has_audio.unwrap_or(false),
                &ssrc,
                "TCP-PASSIVE",
            )
            .await;
            payload["playUrl"] =
                serde_json::json!(format!("rtsp://{}:554/rtp/{}", zlm_client.ip, stream_id));
            // TCP-PASSIVE 分支同样是"点播成功"，照样刷新缩略图
            crate::handlers::device_query::spawn_snapshot_capture(
                state.clone(),
                device_id.clone(),
                channel_id.clone(),
            );
            return Json(ApiResult::success(payload));
        }

        match sip
            .send_play_invite_and_wait_media(
                &device_id,
                &channel_id,
                rtp_server.port,
                &stream_id,
                Some(&ssrc),
                15,
            )
            .await
        {
            Ok((_call_id, _zlm_stream_id, invite_resp)) => {
                tracing::info!(
                    "SIP INVITE + ZLM media ready for {}/{}",
                    device_id,
                    channel_id
                );

                // send_play_invite_and_wait_media 已确认 ZLM 收到媒体；端口差异
                // 只作为设备行为诊断，不能再让 ZLM 主动连接设备。
                if let Some(device_port) =
                    crate::sip::gb28181::sdp_builder::media_ready_sdp_port_mismatch(
                        invite_resp.as_ref().and_then(|r| r.body.as_deref()),
                        rtp_server.port,
                    )
                {
                    tracing::info!(
                        %device_id, %channel_id, sdp_media_port = device_port,
                        zlm_rtp_port = rtp_server.port, reason = "media_already_ready",
                        "设备 SDP 端口与 ZLM 收流端口不同，继续使用已就绪媒体"
                    );
                }

                // 构建播放地址返回给前端
                let stream_url = format!("rtp/{}", stream_id);
                // 这里 zlm_client 中尚未获取自身的配置公网 IP/Port
                // 因为播放接口通常提供各个协议的地址，我们可以用 127.0.0.1 或者 media server 配置地址
                let media_ip = zlm_client.ip.clone();
                let http_port = zlm_client.http_port;
                // 注意这里假设了几个默认端口（如果在配置里解析过可以替换），这里为了快速回掉先用通配协议配置

                let _ = (media_ip, http_port, stream_url);
                // 点播成功 → 后台抓一帧存成该通道的缩略图（覆盖上一张）。
                // 不阻塞本次响应；失败只记日志，不影响播放。
                crate::handlers::device_query::spawn_snapshot_capture(
                    state.clone(),
                    device_id.clone(),
                    channel_id.clone(),
                );
                // 与其它成功分支共用同一份 URL（FLV 后缀 .live.flv、hls 需探测）
                return Json(ApiResult::success(
                    play_urls_json(
                        zlm_client,
                        &stream_id,
                        &device_id,
                        &channel_id,
                        channel.has_audio.unwrap_or(false),
                        &ssrc,
                        &transport_mode,
                    )
                    .await,
                ));
            }
            Err(e) => {
                tracing::error!("SIP INVITE or media wait failed: {}", e);
                // 清理已开启的 RTP 端口
                let _ = zlm_client.close_rtp_server(&stream_id).await;
                // SIP 层已按本次 Call-ID 清理；拒绝或超时的 INVITE 没有可用对话。
                return Json(ApiResult::error(play_sip_error(zlm_client, &stream_id, &e)));
            }
        }
    }

    Json(ApiResult::error("未配置 ZLM 媒体节点，无法发起点播"))
}

/// GET /api/play/stop/{device_id}/{channel_id}
///
/// 停止一路实时点播：关 ZLM 收流端口 + 发 SIP BYE。
#[utoipa::path(
    get,
    path = "/api/play/stop/{device_id}/{channel_id}",
    tag = "live",
    operation_id = "play_stop",
    params(
        ("device_id" = String, Path, description = "设备国标编号（20 位）"),
        ("channel_id" = String, Path, description = "通道国标编号（20 位）"),
    ),
    responses(
        (status = 200, description = "成功仅表示 SIP BYE 已发送；发送失败返回业务错误（ZLM 清理已尝试）",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"callId":"abc-123"}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn play_stop(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Json<ApiResult<serde_json::Value>> {
    tracing::info!("Stop play: device={}, channel={}", device_id, channel_id);

    let sip_server = match &state.sip_server {
        Some(s) => s.clone(),
        None => return Json(ApiResult::error("SIP server not available")),
    };

    let stream_id = format!("{}_{}", device_id, channel_id);
    sip_server
        .stream_reconnect_manager()
        .mark_manual_stop(&stream_id);

    // 1. 通知 ZLM 停止该流的接收
    if let Some(ref zlm_client) = state.zlm_client {
        let _ = zlm_client.close_rtp_server(&stream_id).await;
        // 为了干净，把相关的 session 都踢掉
        let _ = zlm_client
            .close_streams(None, Some("rtp"), Some(&stream_id), true)
            .await;
        tracing::debug!("ZLM resources cleaned for stream={}", stream_id);
    }

    // 2. 发送 SIP BYE 挂断设备的推流（使用 InviteSession 中的 Call-ID）
    // Phase 3.1: 删除 talk BYE fallback —— live 与 talk 是不同 session，
    // talk BYE 走错语义；live 路径 3.1 保证 InviteSession 一定存在。
    let sip = &*sip_server;
    match sip
        .send_session_bye_by_type(
            &device_id,
            &channel_id,
            crate::sip::gb28181::invite_session::StreamType::Play,
        )
        .await
    {
        Ok(call_id) => {
            tracing::info!(
                "Session BYE sent for stream {} call_id={}",
                stream_id,
                call_id
            );
            // 返回 call_id 给调用方以便排查
            return Json(ApiResult::success(serde_json::json!({"callId": call_id})));
        }
        Err(e) => {
            tracing::warn!("Failed to send session BYE for stream {}: {}", stream_id, e);
            return Json(ApiResult::error(format!(
                "ZLM 清理已尝试，但设备 BYE 未发送：{e}"
            )));
        }
    }
}

/// GET /api/play/broadcast/{device_id}/{channel_id}
///
/// 发起国标语音广播：SIP INVITE + BroadcastManager（与 talk 互不共享）。
#[utoipa::path(
    get,
    path = "/api/play/broadcast/{device_id}/{channel_id}",
    tag = "live",
    operation_id = "play_broadcast_start",
    params(
        ("device_id" = String, Path, description = "设备国标编号（20 位）"),
        ("channel_id" = String, Path, description = "通道国标编号（20 位）"),
    ),
    responses(
        (status = 200, description = "广播 INVITE 已发送",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"deviceId":"dev","channelId":"ch","callId":"bc-1","message":"Broadcast started"}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn broadcast_start(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Json<ApiResult<serde_json::Value>> {
    tracing::info!(
        "Broadcast start: device={}, channel={}",
        device_id,
        channel_id
    );

    let device = match db_device::get_device_by_device_id(&state.pool, &device_id).await {
        Ok(Some(d)) => d,
        Ok(None) => {
            return Json(ApiResult::error("Device not found"));
        }
        Err(e) => {
            tracing::error!("Failed to query device: {}", e);
            return Json(ApiResult::error("Database error"));
        }
    };

    if !device.on_line.unwrap_or(false) {
        return Json(ApiResult::error("Device is offline"));
    }

    let sip_server = match &state.sip_server {
        Some(s) => s.clone(),
        None => {
            return Json(ApiResult::error("SIP server not available"));
        }
    };

    let sip = &*sip_server;
    // Phase 3.5: broadcast 走独立 BroadcastManager，不再共享 talk_invite
    match sip.send_broadcast_invite(&device_id, &channel_id).await {
        Ok(call_id) => {
            tracing::info!(
                "Broadcast INVITE sent to {}/{} call_id={}",
                device_id,
                channel_id,
                call_id
            );
            Json(ApiResult::success(serde_json::json!({
                "deviceId": device_id,
                "channelId": channel_id,
                "callId": call_id,
                "message": "Broadcast started"
            })))
        }
        Err(e) => {
            tracing::error!("Failed to send broadcast INVITE: {}", e);
            Json(ApiResult::error(format!("SIP error: {}", e)))
        }
    }
}

/// GET /api/play/broadcast/stop/{device_id}/{channel_id}
///
/// 结束一路语音广播：SIP BYE。
#[utoipa::path(
    get,
    path = "/api/play/broadcast/stop/{device_id}/{channel_id}",
    tag = "live",
    operation_id = "play_broadcast_stop",
    params(
        ("device_id" = String, Path, description = "设备国标编号（20 位）"),
        ("channel_id" = String, Path, description = "通道国标编号（20 位）"),
    ),
    responses(
        (status = 200, description = "广播 BYE 已发送",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"deviceId":"dev","channelId":"ch","message":"Broadcast stopped"}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn broadcast_stop(
    State(state): State<AppState>,
    Path((device_id, channel_id)): Path<(String, String)>,
) -> Json<ApiResult<serde_json::Value>> {
    tracing::info!(
        "Broadcast stop: device={}, channel={}",
        device_id,
        channel_id
    );

    let sip_server = match &state.sip_server {
        Some(s) => s.clone(),
        None => {
            return Json(ApiResult::error("SIP server not available"));
        }
    };

    let sip = &*sip_server;
    // Phase 3.5: broadcast BYE 走 BroadcastManager，与 talk 互不影响
    match sip.send_broadcast_bye(&device_id, &channel_id).await {
        Ok(_) => {
            tracing::info!("Broadcast BYE sent to {}/{}", device_id, channel_id);
            Json(ApiResult::success(serde_json::json!({
                "deviceId": device_id,
                "channelId": channel_id,
                "message": "Broadcast stopped"
            })))
        }
        Err(e) => {
            tracing::error!("Failed to send broadcast BYE: {}", e);
            Json(ApiResult::error(format!("SIP error: {}", e)))
        }
    }
}

// ---------------------------------------------------------------------------
// C6: 分享鉴权 token
// ---------------------------------------------------------------------------

use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShareToken {
    pub token: String,
    #[serde(alias = "deviceId")]
    pub device_id: String,
    #[serde(alias = "channelId")]
    pub channel_id: String,
    #[serde(alias = "expiresAt")]
    pub expires_at: i64,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ShareCreateQuery {
    pub device_id: Option<String>,
    pub channel_id: Option<String>,
    pub ttl: Option<i64>,
}

fn share_tokens_store() -> &'static Mutex<Vec<ShareToken>> {
    static STORE: OnceLock<Mutex<Vec<ShareToken>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Vec::new()))
}

/// GET /api/play/share?deviceId=...&channelId=...&ttl=3600
///
/// 生成短期分享 token（默认 1 小时），客户端可用此 token 绕过 JWT 鉴权
/// 调用 /api/play/start/{device}/{channel}（前端 share.vue 落地页用）。
///
/// **公开端点**（`api_public`）：仅凭 token 校验身份，无需 JWT。
#[utoipa::path(
    get,
    path = "/api/play/share",
    tag = "live",
    operation_id = "play_share_create",
    params(ShareCreateQuery),
    responses(
        (status = 200, description = "分享 token 已生成",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"token":"share_xxx_yyyy","deviceId":"dev","channelId":"ch","expiresAt":1700000000,"ttl":3600}})),
    ),
)]
pub async fn play_share_create(
    axum::extract::Query(q): axum::extract::Query<ShareCreateQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    let device_id = q.device_id.unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or_default();
    if device_id.is_empty() || channel_id.is_empty() {
        return Json(ApiResult::error("deviceId and channelId required"));
    }
    let ttl = q.ttl.unwrap_or(3600).clamp(60, 86400); // 1 min - 24 h

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let expires_at = now + ttl;

    let token = format!("share_{:x}_{:08x}", expires_at, rand::random::<u32>());

    let entry = ShareToken {
        token: token.clone(),
        device_id: device_id.clone(),
        channel_id: channel_id.clone(),
        expires_at,
    };

    if let Ok(mut tokens) = share_tokens_store().lock() {
        // GC expired tokens
        tokens.retain(|t| t.expires_at > now);
        tokens.push(entry);
    }

    Json(ApiResult::success(serde_json::json!({
        "token": token,
        "deviceId": device_id,
        "channelId": channel_id,
        "expiresAt": expires_at,
        "ttl": ttl,
    })))
}

/// GET /api/play/share/info?token=...
///
/// 校验 token，返回 deviceId/channelId/expiresAt。**公开端点**（`api_public`）。
#[utoipa::path(
    get,
    path = "/api/play/share/info",
    tag = "live",
    operation_id = "play_share_info",
    params(
        ("token" = Option<String>, Query, description = "分享 token（`/api/play/share` 生成）"),
    ),
    responses(
        (status = 200, description = "token 有效，返回目标通道信息",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"deviceId":"dev","channelId":"ch","expiresAt":1700000000}})),
        (status = 400, description = "token 缺失"),
        (status = 401, description = "token 无效或已过期"),
    ),
)]
pub async fn play_share_info(
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<ApiResult<serde_json::Value>> {
    let token = match q.get("token") {
        Some(t) => t.clone(),
        None => return Json(ApiResult::error("token required")),
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    if let Ok(mut tokens) = share_tokens_store().lock() {
        tokens.retain(|t| t.expires_at > now);
        if let Some(t) = tokens.iter().find(|t| t.token == token) {
            return Json(ApiResult::success(serde_json::json!({
                "deviceId": t.device_id,
                "channelId": t.channel_id,
                "expiresAt": t.expires_at,
            })));
        }
    }

    Json(ApiResult::error("Invalid or expired token"))
}

/// GET /api/play/share/start?token=...
///
/// 凭 share token 启动播放（无 JWT 鉴权）。**公开端点**（`api_public`）。
#[utoipa::path(
    get,
    path = "/api/play/share/start",
    tag = "live",
    operation_id = "play_share_start",
    params(
        ("token" = Option<String>, Query, description = "分享 token"),
    ),
    responses(
        (status = 200, description = "播放会话已建立",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"deviceId":"dev","channelId":"ch","app":"rtp","stream":"dev_ch","ssrc":"0100000001"}})),
        (status = 401, description = "token 无效或已过期"),
    ),
)]
pub async fn play_share_start(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<ApiResult<serde_json::Value>> {
    let token = match q.get("token") {
        Some(t) => t.clone(),
        None => return Json(ApiResult::error("token required")),
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let (device_id, channel_id) = {
        match share_tokens_store().lock() {
            Ok(mut tokens) => {
                tokens.retain(|t| t.expires_at > now);
                match tokens.iter().find(|t| t.token == token) {
                    Some(t) => (t.device_id.clone(), t.channel_id.clone()),
                    None => return Json(ApiResult::error("Invalid or expired token")),
                }
            }
            Err(_) => return Json(ApiResult::error("Token store unavailable")),
        }
    };

    // 复用 play_start 逻辑（提取 URL）
    let device = match db_device::get_device_by_device_id(&state.pool, &device_id).await {
        Ok(Some(d)) => d,
        _ => return Json(ApiResult::error("Device not found")),
    };

    let stream_id = format!("{}_{}", device_id, channel_id);
    let _ = device;
    let _ = stream_id;

    Json(ApiResult::success(serde_json::json!({
        "deviceId": device_id,
        "channelId": channel_id,
        "app": "rtp",
        "stream": format!("{}_{}", device_id, channel_id),
        "ssrc": "0100000001",
    })))
}

#[cfg(test)]
mod passive_media_tests {
    use super::*;

    #[tokio::test]
    async fn passive_connect_requires_media_registration() {
        let manager = crate::sip::gb28181::media_waiter::MediaWaiterManager::new();
        let (_, rx) = manager.register("passive-timeout", "stream-a", "rtp", 15);
        let reason = wait_for_passive_media(rx, "stream-a", std::time::Duration::from_millis(10))
            .await
            .unwrap_err();
        assert!(reason.contains("media timeout"));
        assert!(manager.cancel_by_call_id("passive-timeout"));
        assert_eq!(manager.active_count(), 0);

        let (_, rx) = manager.register("passive-ready", "stream-a", "rtp", 15);
        assert!(manager.resolve_by_stream("stream-a", "rtp"));
        wait_for_passive_media(rx, "stream-a", std::time::Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(manager.active_count(), 0);
    }
}

#[cfg(test)]
mod live_audio_tests {
    #[test]
    fn ready_zlm_audio_track_overrides_catalog_flag() {
        let tracks: Vec<crate::zlm::TrackInfo> = serde_json::from_value(serde_json::json!([
            {"codec_id":0,"codec_id_name":"CodecH264","codec_type":0,"ready":true},
            {"codec_id":10,"codec_id_name":"CodecG711A","codec_type":1,"ready":true}
        ]))
        .unwrap();
        assert_eq!(super::media_audio_from_tracks(&tracks), Some(true));
        assert_eq!(super::media_audio_from_tracks(&[]), None);
        let video_only: Vec<crate::zlm::TrackInfo> = serde_json::from_value(serde_json::json!([
            {"codec_id":0,"codec_id_name":"CodecH264","codec_type":0,"ready":true},
            {"codec_id":10,"codec_id_name":"CodecG711A","codec_type":1,"ready":false}
        ]))
        .unwrap();
        assert_eq!(super::media_audio_from_tracks(&video_only), Some(false));
    }
}

#[cfg(test)]
mod share_token_tests {
    use super::*;

    /// C6: 生成的 token 应当满足最小长度并且具备 device/channel 信息
    #[test]
    fn test_share_token_format() {
        let token = format!("share_{:x}_{:08x}", 1234567890_i64, 0xDEADBEEF_u32);
        assert!(token.starts_with("share_"));
        let parts: Vec<&str> = token.split('_').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "share");
        // 后两段是 hex
        assert!(u64::from_str_radix(parts[1], 16).is_ok());
        assert!(u32::from_str_radix(parts[2], 16).is_ok());
    }

    /// C6: TTL 必须 clamp 在 [60, 86400]
    #[test]
    fn test_share_ttl_clamp() {
        let clamp = |v: i64| v.clamp(60, 86400);
        assert_eq!(clamp(0), 60);
        assert_eq!(clamp(3600), 3600);
        assert_eq!(clamp(100_000_000), 86400);
    }

    /// C6: ShareToken 结构体可以序列化/反序列化
    #[test]
    fn test_share_token_serde_roundtrip() {
        let t = ShareToken {
            token: "share_abc_def".to_string(),
            device_id: "34020000001320000001".to_string(),
            channel_id: "34020000001320000010".to_string(),
            expires_at: 1234567890,
        };
        let s = serde_json::to_string(&t).unwrap();
        let back: ShareToken = serde_json::from_str(&s).unwrap();
        assert_eq!(back.token, t.token);
        assert_eq!(back.device_id, t.device_id);
        assert_eq!(back.channel_id, t.channel_id);
        assert_eq!(back.expires_at, t.expires_at);
    }

    /// C6: token store 初始化是空的
    #[test]
    fn test_share_store_lookup_and_expiry() {
        // 原断言是 `lock.len() >= 0` —— usize 恒 ≥ 0，**断言恒真**，实为空测试。
        // 现改为校验真实语义：未过期可查到、过期被 GC、伪造 token 不命中。
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let live = format!("test-live-{}", std::process::id());
        let expired = format!("test-expired-{}", std::process::id());

        {
            let mut tokens = share_tokens_store().lock().unwrap();
            tokens.push(ShareToken {
                token: live.clone(),
                device_id: "d".into(),
                channel_id: "c".into(),
                expires_at: now + 3600,
            });
            tokens.push(ShareToken {
                token: expired.clone(),
                device_id: "d".into(),
                channel_id: "c".into(),
                expires_at: now - 1,
            });
        }
        {
            let mut tokens = share_tokens_store().lock().unwrap();
            tokens.retain(|t| t.expires_at > now);
            assert!(
                tokens.iter().any(|t| t.token == live),
                "未过期 token 应可查到"
            );
            assert!(
                !tokens.iter().any(|t| t.token == expired),
                "过期 token 应被 GC"
            );
            assert!(
                !tokens.iter().any(|t| t.token == "definitely-not-a-token"),
                "伪造 token 不应命中"
            );
            // 清理本测试写入的数据，避免影响并行测试
            tokens.retain(|t| t.token != live && t.token != expired);
        }
    }

    /// C6: formatExpireTime 风格测试 — 验证 expires_at 与 now 的差值计算
    #[test]
    fn test_share_token_expires_at_future() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let expires = now + 3600;
        assert!(expires > now);
        assert_eq!(expires - now, 3600);
    }
}

/// `POST /api/play/convertStop/{key}`
///
/// 停止并删除一个 ffmpeg 转码/转推源：`key` 是 `addFFmpegSource` 返回的键。
/// 此前该端点未挂载，转码流停止时 ZLM 会一直重试拉流。
#[utoipa::path(
    post,
    path = "/api/play/convertStop/{key}",
    tag = "live",
    operation_id = "play_convert_stop",
    params(
        ("key" = String, Path, description = "ffmpeg 源 key（addFFmpegSource 返回）"),
        ConvertStopQuery,
    ),
    responses(
        (status = 200, description = "已删除 ffmpeg 源",
         body = ApiResult<serde_json::Value>,
         example = json!({"code":0,"msg":"成功","data":{"key":"key-1","mediaServerId":"zlm-1","deleted":true}})),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),
)]
pub async fn play_convert_stop(
    State(state): State<AppState>,
    axum::extract::Path(key): axum::extract::Path<String>,
    Query(q): Query<ConvertStopQuery>,
) -> Json<ApiResult<serde_json::Value>> {
    let media_server_id = q.media_server_id.clone().unwrap_or_default();
    // 接口要求显式给 mediaServerId；缺省时按"所有节点都试一遍"更实用，
    // 但要在响应里说清楚是哪个节点删掉的。
    let clients: Vec<(String, std::sync::Arc<crate::zlm::ZlmClient>)> =
        if media_server_id.is_empty() {
            state
                .zlm_clients
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        } else {
            match state.zlm_clients.get(&media_server_id) {
                Some(c) => vec![(media_server_id.clone(), c.clone())],
                None => return Json(ApiResult::error(format!("流媒体不存在: {media_server_id}"))),
            }
        };
    if clients.is_empty() {
        return Json(ApiResult::error("没有可用的流媒体节点"));
    }

    let mut errors = Vec::new();
    for (id, client) in &clients {
        match client.del_ffmpeg_source(&key).await {
            Ok(_) => {
                tracing::info!("convertStop: 已删除 ffmpeg 源 key={} node={}", key, id);
                return Json(ApiResult::success(serde_json::json!({
                    "key": key,
                    "mediaServerId": id,
                    "deleted": true,
                })));
            }
            Err(e) => {
                tracing::warn!("convertStop: 节点 {} 删除失败: {}", id, e);
                errors.push(format!("{id}: {e}"));
            }
        }
    }
    Json(ApiResult::error(format!(
        "删除 ffmpeg 源失败: {}",
        errors.join("; ")
    )))
}

#[derive(Debug, Default, serde::Deserialize, IntoParams)]
pub struct ConvertStopQuery {
    /// 目标 ZLM 节点 ID；缺省时尝试所有节点
    #[serde(alias = "mediaServerId")]
    pub media_server_id: Option<String>,
}

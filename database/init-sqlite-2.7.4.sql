-- GBServer SQLite schema (Phase 1: 核心 6 表最小集合)
-- 与 PostgreSQL/MySQL schema 字段保持一致，类型按 SQLite 方言转换：
--   serial/bigserial   → INTEGER PRIMARY KEY AUTOINCREMENT
--   character varying  → VARCHAR / TEXT
--   bool               → INTEGER (0/1)
--   double precision   → REAL
--   int8               → INTEGER
--   text               → TEXT
-- 不支持 COMMENT ON COLUMN，注释以行内 -- 形式提供。
-- 备注：SQLite 不支持 PG 原生分区表，position_history 等时序表在应用层分表。

-- ============================================
-- 1. gb_device — 国标设备基础信息
-- ============================================
CREATE TABLE IF NOT EXISTS gb_device
(
    id                                  INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id                           VARCHAR(50)  NOT NULL,
    name                                VARCHAR(255),
    manufacturer                        VARCHAR(255),
    model                               VARCHAR(255),
    firmware                            VARCHAR(255),
    transport                           VARCHAR(50),
    stream_mode                         VARCHAR(50),
    on_line                             INTEGER     DEFAULT 0,
    register_time                       VARCHAR(50),
    keepalive_time                      VARCHAR(50),
    ip                                  VARCHAR(50),
    create_time                         VARCHAR(50),
    update_time                         VARCHAR(50),
    port                                INTEGER,
    expires                             INTEGER,
    subscribe_cycle_for_catalog         INTEGER     DEFAULT 0,
    subscribe_cycle_for_mobile_position INTEGER     DEFAULT 0,
    mobile_position_submission_interval INTEGER     DEFAULT 5,
    subscribe_cycle_for_alarm           INTEGER     DEFAULT 0,
    host_address                        VARCHAR(50),
    charset                             VARCHAR(50),
    ssrc_check                          INTEGER     DEFAULT 0,
    geo_coord_sys                       VARCHAR(50),
    media_server_id                     VARCHAR(50) DEFAULT 'auto',
    custom_name                         VARCHAR(255),
    sdp_ip                              VARCHAR(50),
    local_ip                            VARCHAR(50),
    password                            VARCHAR(255),
    as_message_channel                  INTEGER     DEFAULT 0,
    heart_beat_interval                 INTEGER,
    heart_beat_count                    INTEGER,
    position_capability                 INTEGER,
    broadcast_push_after_ack            INTEGER     DEFAULT 0,
    server_id                           VARCHAR(50)
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_device_device ON gb_device(device_id);

-- ============================================
-- 2. gb_device_channel — 设备通道
-- ============================================
CREATE TABLE IF NOT EXISTS gb_device_channel
(
    id                           INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id                    VARCHAR(50),
    name                         VARCHAR(255),
    manufacturer                 VARCHAR(50),
    model                        VARCHAR(50),
    owner                        VARCHAR(50),
    civil_code                   VARCHAR(50),
    block                        VARCHAR(50),
    address                      VARCHAR(50),
    parental                     INTEGER,
    parent_id                    VARCHAR(50),
    safety_way                   INTEGER,
    register_way                 INTEGER,
    cert_num                     VARCHAR(50),
    certifiable                  INTEGER,
    err_code                     INTEGER,
    end_time                     VARCHAR(50),
    secrecy                      INTEGER,
    ip_address                   VARCHAR(50),
    port                         INTEGER,
    password                     VARCHAR(255),
    status                       VARCHAR(50),
    longitude                    REAL,
    latitude                     REAL,
    ptz_type                     INTEGER,
    position_type                INTEGER,
    room_type                    INTEGER,
    use_type                     INTEGER,
    supply_light_type            INTEGER,
    direction_type               INTEGER,
    resolution                   VARCHAR(255),
    business_group_id            VARCHAR(255),
    download_speed               VARCHAR(255),
    svc_space_support_mod        INTEGER,
    svc_time_support_mode        INTEGER,
    create_time                  VARCHAR(50) NOT NULL,
    update_time                  VARCHAR(50) NOT NULL,
    sub_count                    INTEGER,
    stream_id                    VARCHAR(255),
    has_audio                    INTEGER     DEFAULT 0,
    gps_time                     VARCHAR(50),
    stream_identification        VARCHAR(50),
    channel_type                 INTEGER     DEFAULT 0 NOT NULL,
    map_level                    INTEGER     DEFAULT 0,
    gb_device_id                 VARCHAR(50),
    gb_name                      VARCHAR(255),
    gb_manufacturer              VARCHAR(255),
    gb_model                     VARCHAR(255),
    gb_owner                     VARCHAR(255),
    gb_civil_code                VARCHAR(255),
    gb_block                     VARCHAR(255),
    gb_address                   VARCHAR(255),
    gb_parental                  INTEGER,
    gb_parent_id                 VARCHAR(255),
    gb_safety_way                INTEGER,
    gb_register_way              INTEGER,
    gb_cert_num                  VARCHAR(50),
    gb_certifiable               INTEGER,
    gb_err_code                  INTEGER,
    gb_end_time                  VARCHAR(50),
    gb_secrecy                   INTEGER,
    gb_ip_address                VARCHAR(50),
    gb_port                      INTEGER,
    gb_password                  VARCHAR(50),
    gb_status                    VARCHAR(50),
    gb_longitude                 REAL,
    gb_latitude                  REAL,
    gb_business_group_id         VARCHAR(50),
    gb_ptz_type                  INTEGER,
    gb_position_type             INTEGER,
    gb_room_type                 INTEGER,
    gb_use_type                  INTEGER,
    gb_supply_light_type         INTEGER,
    gb_direction_type            INTEGER,
    gb_resolution                VARCHAR(255),
    gb_download_speed            VARCHAR(255),
    gb_svc_space_support_mod     INTEGER,
    gb_svc_time_support_mode     INTEGER,
    record_plan_id               INTEGER,
    data_type                    INTEGER     NOT NULL,
    data_device_id               INTEGER     NOT NULL,
    gps_speed                    REAL,
    gps_altitude                 REAL,
    gps_direction                REAL,
    enable_broadcast             INTEGER     DEFAULT 0
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_gb_unique_channel ON gb_device_channel(gb_device_id);
CREATE INDEX IF NOT EXISTS idx_data_type ON gb_device_channel(data_type);
CREATE INDEX IF NOT EXISTS idx_data_device_id ON gb_device_channel(data_device_id);

-- ============================================
-- 3. gb_user — 平台用户
-- ============================================
CREATE TABLE IF NOT EXISTS gb_user
(
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    username    VARCHAR(255),
    password    VARCHAR(255),
    role_id     INTEGER,
    create_time VARCHAR(50),
    update_time VARCHAR(50),
    push_key    VARCHAR(50)
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_user_username ON gb_user(username);

-- ============================================
-- 4. gb_user_role — 角色
-- ============================================
CREATE TABLE IF NOT EXISTS gb_user_role
(
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        VARCHAR(50),
    authority   VARCHAR(50),
    create_time VARCHAR(50),
    update_time VARCHAR(50)
);

-- ============================================
-- 5. gb_media_server — 媒体服务器节点（ZLM）
-- ============================================
CREATE TABLE IF NOT EXISTS gb_media_server
(
    id                  VARCHAR(255) PRIMARY KEY,
    ip                  VARCHAR(50),
    hook_ip             VARCHAR(50),
    sdp_ip              VARCHAR(50),
    stream_ip           VARCHAR(50),
    http_port           INTEGER,
    http_ssl_port       INTEGER,
    rtmp_port           INTEGER,
    rtmp_ssl_port       INTEGER,
    rtp_proxy_port      INTEGER,
    rtsp_port           INTEGER,
    rtsp_ssl_port       INTEGER,
    flv_port            INTEGER,
    flv_ssl_port        INTEGER,
    mp4_port            INTEGER,
    mp4_ssl_port        INTEGER,
    ws_flv_port         INTEGER,
    ws_flv_ssl_port     INTEGER,
    jtt_proxy_port      INTEGER,
    auto_config         INTEGER     DEFAULT 0,
    secret              VARCHAR(50),
    type                VARCHAR(50) DEFAULT 'zlm',
    rtp_enable          INTEGER     DEFAULT 0,
    rtp_port_range      VARCHAR(50),
    send_rtp_port_range VARCHAR(50),
    record_assist_port  INTEGER,
    default_server      INTEGER     DEFAULT 0,
    create_time         VARCHAR(50),
    update_time         VARCHAR(50),
    hook_alive_interval INTEGER,
    record_path         VARCHAR(255),
    record_day          INTEGER     DEFAULT 7,
    transcode_suffix    VARCHAR(255),
    server_id           VARCHAR(50)
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_media_server_unique_ip_http_port
    ON gb_media_server(ip, http_port, server_id);

-- ============================================
-- 5b. gb_media_server_white_list — 媒体服务器 IP 白名单（Phase 4.2）
-- ============================================
CREATE TABLE IF NOT EXISTS gb_media_server_white_list
(
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    media_server_id VARCHAR(255) NOT NULL,
    cidr            VARCHAR(50)  NOT NULL,
    create_time     VARCHAR(50)
);
CREATE INDEX IF NOT EXISTS idx_media_server_white_list_server_id
    ON gb_media_server_white_list(media_server_id);

-- ============================================
-- 6. gb_stream_proxy — 拉流代理 / 转推配置
-- ============================================
CREATE TABLE IF NOT EXISTS gb_stream_proxy
(
    id                         INTEGER PRIMARY KEY AUTOINCREMENT,
    type                       VARCHAR(50),
    app                        VARCHAR(255),
    stream                     VARCHAR(255),
    src_url                    VARCHAR(255),
    timeout                    INTEGER,
    ffmpeg_cmd_key             VARCHAR(255),
    rtsp_type                  VARCHAR(50),
    media_server_id            VARCHAR(50),
    enable_audio               INTEGER DEFAULT 0,
    enable_mp4                 INTEGER DEFAULT 0,
    pulling                    INTEGER DEFAULT 0,
    enable                     INTEGER DEFAULT 0,
    create_time                VARCHAR(50),
    name                       VARCHAR(255),
    update_time                VARCHAR(50),
    stream_key                 VARCHAR(255),
    server_id                  VARCHAR(50),
    enable_disable_none_reader INTEGER DEFAULT 0,
    relates_media_server_id    VARCHAR(50)
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_stream_proxy_app_stream ON gb_stream_proxy(app, stream);

-- ============================================
-- 7. gb_stream_push — 推流会话记录（Batch B 缺表修复）
-- ============================================
CREATE TABLE IF NOT EXISTS gb_stream_push
(
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    app                VARCHAR(255),
    stream             VARCHAR(255),
    create_time        VARCHAR(50),
    media_server_id    VARCHAR(50),
    server_id          VARCHAR(50),
    push_time          VARCHAR(50),
    status             INTEGER     DEFAULT 0,
    update_time        VARCHAR(50),
    pushing            INTEGER     DEFAULT 0,
    self               INTEGER     DEFAULT 0,
    start_offline_push INTEGER     DEFAULT 1
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_stream_push_app_stream ON gb_stream_push(app, stream);

-- ============================================
-- 8. gb_user_api_key — API Key
-- ============================================
CREATE TABLE IF NOT EXISTS gb_user_api_key
(
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER,
    app         VARCHAR(255),
    api_key     TEXT,
    expired_at  INTEGER,
    remark      VARCHAR(255),
    enable      INTEGER     DEFAULT 1,
    create_time VARCHAR(50),
    update_time VARCHAR(50)
);

-- ============================================
-- 9. gb_platform — 上级国标平台注册信息
-- ============================================
CREATE TABLE IF NOT EXISTS gb_platform
(
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    enable                INTEGER     DEFAULT 0,
    name                  VARCHAR(255),
    server_gb_id          VARCHAR(50),
    server_gb_domain      VARCHAR(50),
    server_ip             VARCHAR(50),
    server_port           INTEGER,
    device_gb_id          VARCHAR(50),
    device_ip             VARCHAR(50),
    device_port           VARCHAR(50),
    username              VARCHAR(255),
    password              VARCHAR(50),
    expires               VARCHAR(50),
    keep_timeout          VARCHAR(50),
    transport             VARCHAR(50),
    civil_code            VARCHAR(50),
    manufacturer          VARCHAR(255),
    model                 VARCHAR(255),
    address               VARCHAR(255),
    character_set         VARCHAR(50),
    ptz                   INTEGER     DEFAULT 0,
    rtcp                  INTEGER     DEFAULT 0,
    status                INTEGER     DEFAULT 0,
    catalog_group         INTEGER,
    register_way          INTEGER,
    secrecy               INTEGER,
    create_time           VARCHAR(50),
    update_time           VARCHAR(50),
    as_message_channel    INTEGER     DEFAULT 0,
    catalog_with_platform INTEGER     DEFAULT 1,
    catalog_with_group    INTEGER     DEFAULT 1,
    catalog_with_region   INTEGER     DEFAULT 1,
    auto_push_channel     INTEGER     DEFAULT 1,
    send_stream_ip        VARCHAR(50),
    server_id             VARCHAR(50)
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_platform_unique_server_gb_id ON gb_platform(server_gb_id);

-- ============================================
-- 10. gb_cloud_record — 云端录像记录
-- ============================================
CREATE TABLE IF NOT EXISTS gb_cloud_record
(
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    app             VARCHAR(255),
    stream          VARCHAR(255),
    call_id         VARCHAR(255),
    start_time      INTEGER,
    end_time        INTEGER,
    media_server_id VARCHAR(50),
    server_id       VARCHAR(50),
    file_name       VARCHAR(255),
    folder          VARCHAR(500),
    file_path       VARCHAR(500),
    collect         INTEGER     DEFAULT 0,
    file_size       INTEGER,
    time_len        REAL
);

-- ============================================
-- 11. gb_device_alarm — 设备报警
-- ============================================
CREATE TABLE IF NOT EXISTS gb_device_alarm
(
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id         VARCHAR(50)  NOT NULL,
    channel_id        VARCHAR(50)  NOT NULL,
    alarm_priority    VARCHAR(50),
    alarm_method      VARCHAR(50),
    alarm_time        VARCHAR(50),
    alarm_description VARCHAR(255),
    longitude         REAL,
    latitude          REAL,
    alarm_type        VARCHAR(50),
    create_time       VARCHAR(50)  NOT NULL
);

-- ============================================
-- 12. gb_device_mobile_position — 移动位置
-- ============================================
CREATE TABLE IF NOT EXISTS gb_device_mobile_position
(
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id     VARCHAR(50)  NOT NULL,
    channel_id    VARCHAR(50)  NOT NULL,
    device_name   VARCHAR(255),
    time          VARCHAR(50),
    longitude     REAL,
    latitude      REAL,
    altitude      REAL,
    speed         REAL,
    direction     REAL,
    report_source VARCHAR(50),
    create_time   VARCHAR(50)
);

-- ============================================
-- 13. gb_common_group — 通用分组
-- ============================================
CREATE TABLE IF NOT EXISTS gb_common_group
(
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id        VARCHAR(50)  NOT NULL,
    name             VARCHAR(255) NOT NULL,
    parent_id        INTEGER,
    parent_device_id VARCHAR(50) DEFAULT NULL,
    business_group   VARCHAR(50)  NOT NULL,
    create_time      VARCHAR(50)  NOT NULL,
    update_time      VARCHAR(50)  NOT NULL,
    civil_code       VARCHAR(50) DEFAULT NULL,
    alias            VARCHAR(255) DEFAULT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS uk_common_group_device_platform ON gb_common_group(device_id);

-- ============================================
-- 14. gb_record_plan + gb_record_plan_item
-- ============================================
CREATE TABLE IF NOT EXISTS gb_record_plan
(
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    snap        INTEGER     DEFAULT 0,
    name        VARCHAR(255) NOT NULL,
    create_time VARCHAR(50),
    update_time VARCHAR(50)
);

CREATE TABLE IF NOT EXISTS gb_record_plan_item
(
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    "start"     INTEGER,
    stop        INTEGER,
    week_day    INTEGER,
    plan_id     INTEGER,
    create_time VARCHAR(50),
    update_time VARCHAR(50)
);

-- ============================================
-- 初始数据 — 默认管理员账户（与 PG/MySQL 一致）
-- admin / admin  (MD5: 21232f297a57a5a743894a0e4a801fc3)
-- ============================================
INSERT INTO gb_user_role (id, name, authority, create_time, update_time)
VALUES (1, 'admin', '0', '2021-04-13 14:14:57', '2021-04-13 14:14:57');

INSERT INTO gb_user (id, username, password, role_id, create_time, update_time, push_key)
VALUES (1, 'admin', '21232f297a57a5a743894a0e4a801fc3', 1,
        '2021-04-13 14:14:57', '2021-04-13 14:14:57',
        '3e80d1762a324d5b0ff636e0bd16f1e3');
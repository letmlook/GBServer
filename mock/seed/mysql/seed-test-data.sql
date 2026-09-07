-- GBServer 测试种子数据（MySQL 兼容）
-- 表名 / 列名按 SQLite 实际 schema 编写

INSERT IGNORE INTO gb_user_role (id, name, authority, create_time, update_time) VALUES
  (1, 'admin', '0', '2026-08-23 20:00:00', '2026-08-23 20:00:00'),
  (2, 'user',  '1', '2026-08-23 20:00:00', '2026-08-23 20:00:00');

INSERT IGNORE INTO gb_user (id, username, password, role_id, push_key, create_time, update_time) VALUES
  (1, 'admin',  '21232f297a57a5a743894a0e4a801fc3', 1, 'admin-push-key',  '2026-08-23 20:00:00', '2026-08-23 20:00:00'),
  (2, 'viewer', '21232f297a57a5a743894a0e4a801fc3', 2, 'viewer-push-key', '2026-08-23 20:00:00', '2026-08-23 20:00:00'),
  (3, 'tester', '21232f297a57a5a743894a0e4a801fc3', 2, 'tester-push-key', '2026-08-23 20:00:00', '2026-08-23 20:00:00');

INSERT IGNORE INTO gb_common_region (id, device_id, name, parent_id, parent_device_id, create_time, update_time) VALUES
  (1, '34020000000000000000', '北京市', NULL, NULL, '2026-08-23 20:00:00', '2026-08-23 20:00:00'),
  (2, '34020000000000000001', '海淀区', 1, '34020000000000000000', '2026-08-23 20:00:00', '2026-08-23 20:00:00'),
  (3, '34020000000000000002', '朝阳区', 1, '34020000000000000000', '2026-08-23 20:00:00', '2026-08-23 20:00:00');

INSERT IGNORE INTO gb_common_group (id, device_id, name, parent_id, parent_device_id, business_group, create_time, update_time) VALUES
  (1, '34020000002000000010', '重点监控组', NULL, NULL, 'BusinessGroup', '2026-08-23 20:00:00', '2026-08-23 20:00:00'),
  (2, '34020000002000000011', '车载终端组', 1, '34020000002000000010', 'BusinessGroup', '2026-08-23 20:00:00', '2026-08-23 20:00:00');

INSERT IGNORE INTO gb_device (
  device_id, name, manufacturer, model, firmware,
  transport, stream_mode, on_line,
  register_time, keepalive_time, ip, port, expires,
  subscribe_cycle_for_catalog, subscribe_cycle_for_mobile_position,
  media_server_id, password, channel_count, server_id,
  create_time, update_time
) VALUES
  ('34020000001320000001', '测试摄像机-01', 'MockVendor', 'MOCK-IPC-100', '1.0.0-mock',
   'UDP', 'TCP-ACTIVE', 1,
   '2026-08-23 20:00:00', '2026-08-23 20:30:00', '127.0.0.1', 15060, 3600,
   300, 60, 'zlmediakit-1', 'admin123', 4, 'gbserver-001',
   '2026-08-23 20:00:00', '2026-08-23 20:00:00'),
  ('34020000001320000002', '测试摄像机-02', 'MockVendor', 'MOCK-IPC-100', '1.0.0-mock',
   'UDP', 'TCP-ACTIVE', 0,
   '2026-08-22 10:00:00', '2026-08-22 10:25:00', '127.0.0.1', 15061, 3600,
   0, 0, 'zlmediakit-1', 'admin123', 4, 'gbserver-001',
   '2026-08-22 10:00:00', '2026-08-22 10:00:00'),
  ('34020000001320000003', '测试 NVR',     'MockVendor', 'MOCK-NVR-200', '2.0.0-mock',
   'UDP', 'TCP-PASSIVE', 1,
   '2026-08-23 19:00:00', '2026-08-23 20:30:00', '127.0.0.1', 15062, 3600,
   600, 30, 'zlmediakit-1', 'admin123', 16, 'gbserver-001',
   '2026-08-23 19:00:00', '2026-08-23 19:00:00');

INSERT IGNORE INTO gb_device_channel (
  device_id, name, manufacturer, model, owner, civil_code,
  address, parental, parent_id, safety_way, register_way,
  cert_num, certifiable, err_code, ptz_type, status,
  longitude, latitude, channel_type, data_type, data_device_id,
  create_time, update_time
) VALUES
  ('34020000001320000001', '测试摄像机-01-通道1', 'MockVendor', 'MOCK-IPC-100', 'Admin', '110108',
   '海淀-中关村大街1号', 0, '34020000001320000001', 0, 1,
   'CERT000001', 0, 0, 2, 'ON',
   116.397128, 39.916527, 0, 0, 1,
   '2026-08-23 20:00:00', '2026-08-23 20:00:00'),
  ('34020000001320000001', '测试摄像机-01-通道2', 'MockVendor', 'MOCK-IPC-100', 'Admin', '110108',
   '海淀-中关村大街2号', 0, '34020000001320000001', 0, 1,
   'CERT000002', 0, 0, 2, 'ON',
   116.398128, 39.917527, 0, 0, 1,
   '2026-08-23 20:00:00', '2026-08-23 20:00:00'),
  ('34020000001320000003', '测试 NVR-通道1',      'MockVendor', 'MOCK-NVR-200', 'Admin', '110105',
   '朝阳-CBD1号', 0, '34020000001320000003', 0, 1,
   'CERT000003', 0, 0, 2, 'ON',
   116.487128, 39.918527, 0, 0, 3,
   '2026-08-23 20:00:00', '2026-08-23 20:00:00'),
  ('34020000001320000003', '测试 NVR-通道2',      'MockVendor', 'MOCK-NVR-200', 'Admin', '110105',
   '朝阳-CBD2号', 0, '34020000001320000003', 0, 1,
   'CERT000004', 0, 0, 2, 'ON',
   116.488128, 39.919527, 0, 0, 3,
   '2026-08-23 20:00:00', '2026-08-23 20:00:00');

INSERT IGNORE INTO gb_media_server (
  id, ip, hook_ip, sdp_ip, stream_ip,
  http_port, rtmp_port, rtsp_port,
  secret, type, default_server, status,
  create_time, update_time
) VALUES
  ('zlmediakit-1', '127.0.0.1', '127.0.0.1', '127.0.0.1', '127.0.0.1',
   8080, 1935, 554,
   'EWHpCV2W2SYa8SnpE2whVlUUlJJKhaiw', 'zlm', 1, 1,
   '2026-08-23 20:00:00', '2026-08-23 20:00:00');

INSERT IGNORE INTO gb_platform (
  enable, name, server_gb_id, server_gb_domain, server_ip, server_port,
  device_gb_id, device_ip, device_port,
  username, password, expires, keep_timeout, transport,
  civil_code, manufacturer, model, address,
  status, register_way, create_time, update_time
) VALUES
  (1, 'Mock-级联平台-1', '34020000002000000099', '3402000000', '127.0.0.1', 5062,
   '34020000001320000001', '127.0.0.1', '5060',
   'admin', 'admin123', '3600', '60', 'UDP',
   '340200', 'MockVendor', 'MOCK-UP', 'Beijing',
   1, 1, '2026-08-23 20:00:00', '2026-08-23 20:00:00');

INSERT IGNORE INTO gb_jt_terminal (
  phone_number, maker_id, model, plate_color, plate_no,
  longitude, latitude, status,
  media_server_id, create_time, update_time
) VALUES
  ('13912345678', 'MOCK', 'MOCK-V100', 2, '京A12345',
   116.397128, 39.916527, 1,
   'zlmediakit-1', '2026-08-23 20:00:00', '2026-08-23 20:00:00'),
  ('13987654321', 'MOCK', 'MOCK-V200', 2, '京B54321',
   116.487128, 39.918527, 1,
   'zlmediakit-1', '2026-08-23 20:00:00', '2026-08-23 20:00:00');

INSERT IGNORE INTO gb_device_alarm (
  device_id, channel_id, alarm_priority, alarm_method,
  alarm_time, alarm_description, alarm_type, handled,
  longitude, latitude, create_time
) VALUES
  ('34020000001320000001', '34020000001320000001', '1', 'phone',
   '2026-08-23 19:00:00', '通道1 视频丢失', 'video_lost', 0,
   116.397128, 39.916527, '2026-08-23 19:00:00'),
  ('34020000001320000003', '34020000001320000003', '2', 'email',
   '2026-08-23 18:30:00', '磁盘即将满', 'disk_full', 0,
   116.487128, 39.918527, '2026-08-23 18:30:00'),
  ('13912345678', '13912345678-1', '3', 'sms',
   '2026-08-23 17:00:00', '超速报警', 'over_speed', 1,
   116.397128, 39.916527, '2026-08-23 17:00:00');
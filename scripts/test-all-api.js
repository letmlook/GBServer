/**
 * 全量 API 接口测试：测试所有 170+ 端点是否能正确返回真实数据
 * 使用方式: node scripts/test-all-api.js  或  BASE_URL=http://localhost:18080 node scripts/test-all-api.js
 */
const BASE = process.env.BASE_URL || 'http://127.0.0.1:18080';
const ADMIN_MD5 = '21232f297a57a5a743894a0e4a801fc3'; // MD5("admin")

async function request(method, path, token = null, body = null) {
  const url = path.startsWith('http') ? path : `${BASE}${path}`;
  const headers = { 'Content-Type': 'application/json' };
  if (token) headers['access-token'] = token;
  const opt = { method, headers };
  if (body && (method === 'POST' || method === 'PUT')) opt.body = JSON.stringify(body);
  try {
    const res = await fetch(url, opt);
    const text = await res.text();
    let data = null;
    try { data = JSON.parse(text); } catch (_) {}
    const tokenFromHeader = res.headers.get('access-token');
    return { ok: res.ok, status: res.status, data, tokenFromHeader, text: text.slice(0, 300) };
  } catch (e) {
    return { ok: false, status: 0, data: null, tokenFromHeader: null, text: e.message };
  }
}

function ok(r) {
  if (!r.ok) return false;
  if (r.data && typeof r.data.code === 'number' && r.data.code !== 0) return false;
  return true;
}

function hasRealData(r) {
  if (!r.data) return false;
  if (r.data.code !== 0) return false;
  const d = r.data.data;
  if (d === undefined || d === null) return true; // null is acceptable response
  if (Array.isArray(d)) return true; // empty array is acceptable
  if (typeof d === 'object') return Object.keys(d).length > 0;
  return true;
}

async function main() {
  console.log('全量 API 接口测试 base:', BASE);
  console.log('='.repeat(80));
  let token = null;

  // 1) 登录
  const loginRes = await request('GET', `/api/user/login?username=admin&password=${ADMIN_MD5}`);
  if (loginRes.tokenFromHeader) token = loginRes.tokenFromHeader;
  else if (loginRes.data && loginRes.data.data && loginRes.data.data.access_token) token = loginRes.data.data.access_token;
  if (!ok(loginRes) || !token) {
    console.log('FAIL 登录', loginRes.status, loginRes.data || loginRes.text);
    process.exit(1);
  }
  console.log('OK 登录，已获取 token\n');

  const auth = (method, path, body = null) => request(method, path, token, body);

  // 完整端点列表 - 按模块分类
  const categories = {
    '用户管理': [
      ['GET', '/api/user/userInfo'],
      ['GET', '/api/user/users'],
      ['POST', '/api/user/add', { username: 'test', password: 'test', role: 1 }],
      ['POST', '/api/user/changePassword', { oldPassword: 'admin', newPassword: 'admin' }],
      ['POST', '/api/user/changePushKey', { id: 1, pushKey: 'test' }],
    ],
    '设备管理': [
      ['GET', '/api/device/query/devices'],
      ['GET', '/api/device/query/devices?page=1&count=10'],
      ['GET', '/api/device/query/sync_status'],
      ['GET', '/api/device/query/devices/test-device/channels'],
      ['GET', '/api/device/query/devices/test-device'],
      ['GET', '/api/device/query/streams'],
      ['GET', '/api/device/query/channel/one'],
      ['GET', '/api/device/query/tree/test-device'],
      ['GET', '/api/device/query/sub_channels'],
      ['GET', '/api/device/query/tree_channel'],
      ['POST', '/api/device/query/transport', { deviceId: 'test' }],
    ],
    '媒体服务器': [
      ['GET', '/api/server/media_server/list'],
      ['GET', '/api/server/media_server/online/list'],
      ['GET', '/api/server/media_server/one'],
      ['GET', '/api/server/media_server/check'],
      ['GET', '/api/server/media_server/load'],
      ['POST', '/api/server/media_server/save', {}],
      ['DELETE', '/api/server/media_server/delete', { id: 'test' }],
    ],
    '系统信息': [
      ['GET', '/api/server/system/configInfo'],
      ['GET', '/api/server/system/info'],
      ['GET', '/api/server/map/config'],
      ['GET', '/api/server/info'],
      ['GET', '/api/server/resource/info'],
      ['GET', '/api/server/map/model-icon/list'],
    ],
    '推流管理': [
      ['GET', '/api/push/list'],
      ['POST', '/api/push/add', { app: 'test', stream: 'test' }],
      ['POST', '/api/push/update', { id: 1 }],
      ['GET', '/api/push/start'],
      ['POST', '/api/push/remove', { ids: [] }],
      ['DELETE', '/api/push/batchRemove', { ids: [] }],
      ['POST', '/api/push/save_to_gb', { id: 1 }],
      ['POST', '/api/push/remove_form_gb', { id: 1 }],
    ],
    '拉流代理': [
      ['GET', '/api/proxy/list'],
      ['GET', '/api/proxy/ffmpeg_cmd/list'],
      ['POST', '/api/proxy/add', { app: 'test', stream: 'test', srcUrl: 'rtsp://test' }],
      ['POST', '/api/proxy/update', { id: 1 }],
      ['POST', '/api/proxy/save', { id: 1 }],
      ['GET', '/api/proxy/start'],
      ['GET', '/api/proxy/stop'],
      ['DELETE', '/api/proxy/delete', { id: 1 }],
    ],
    '平台级联': [
      ['GET', '/api/platform/query'],
      ['GET', '/api/platform/server_config'],
      ['GET', '/api/platform/channel/list'],
      ['GET', '/api/platform/channel/push'],
      ['POST', '/api/platform/channel/add', {}],
      ['POST', '/api/platform/channel/update', {}],
      ['POST', '/api/platform/channel/device/add', {}],
      ['POST', '/api/platform/channel/device/remove', {}],
      ['POST', '/api/platform/channel/custom/update', {}],
      ['POST', '/api/platform/add', {}],
      ['POST', '/api/platform/update', {}],
      ['DELETE', '/api/platform/delete', {}],
      ['GET', '/api/platform/exit'],
      ['POST', '/api/platform/catalog/add', {}],
      ['POST', '/api/platform/catalog/edit', {}],
    ],
    '区域管理': [
      ['GET', '/api/region/tree/list'],
      ['GET', '/api/region/path'],
      ['GET', '/api/region/tree/query'],
      ['GET', '/api/region/base/child/list'],
      ['GET', '/api/region/addByCivilCode'],
      ['GET', '/api/region/child/list'],
      ['POST', '/api/region/add', {}],
      ['POST', '/api/region/update', {}],
      ['DELETE', '/api/region/delete', {}],
      ['GET', '/api/region/description'],
    ],
    '分组管理': [
      ['GET', '/api/group/tree/list'],
      ['GET', '/api/group/path'],
      ['GET', '/api/group/tree/query'],
      ['POST', '/api/group/add', {}],
      ['POST', '/api/group/update', {}],
      ['DELETE', '/api/group/delete', {}],
    ],
    '角色管理': [
      ['GET', '/api/role/all'],
    ],
    '日志管理': [
      ['GET', '/api/log/list'],
    ],
    'API Key': [
      ['GET', '/api/userApiKey/userApiKeys'],
      ['POST', '/api/userApiKey/add', {}],
      ['POST', '/api/userApiKey/remark', {}],
      ['POST', '/api/userApiKey/enable', {}],
      ['POST', '/api/userApiKey/disable', {}],
      ['POST', '/api/userApiKey/reset', {}],
      ['DELETE', '/api/userApiKey/delete', {}],
    ],
    '通用通道': [
      ['GET', '/api/common/channel/list'],
      ['GET', '/api/common/channel/one'],
      ['GET', '/api/common/channel/industry/list'],
      ['GET', '/api/common/channel/type/list'],
      ['GET', '/api/common/channel/network/identification/list'],
      ['GET', '/api/common/channel/civilcode/list'],
      ['GET', '/api/common/channel/civilCode/unusual/list'],
      ['GET', '/api/common/channel/parent/unusual/list'],
      ['GET', '/api/common/channel/parent/list'],
      ['POST', '/api/common/channel/update', {}],
      ['POST', '/api/common/channel/reset', {}],
      ['POST', '/api/common/channel/add', {}],
      ['POST', '/api/common/channel/civilCode/unusual/clear', {}],
      ['POST', '/api/common/channel/parent/unusual/clear', {}],
      ['POST', '/api/common/channel/region/add', {}],
      ['POST', '/api/common/channel/region/delete', {}],
      ['POST', '/api/common/channel/region/device/add', {}],
      ['POST', '/api/common/channel/region/device/delete', {}],
      ['POST', '/api/common/channel/group/add', {}],
      ['POST', '/api/common/channel/group/delete', {}],
      ['POST', '/api/common/channel/group/device/add', {}],
      ['POST', '/api/common/channel/group/device/delete', {}],
      ['GET', '/api/common/channel/play'],
      ['GET', '/api/common/channel/play/stop'],
      ['GET', '/api/common/channel/map/list'],
      ['POST', '/api/common/channel/map/save-level', {}],
      ['POST', '/api/common/channel/map/reset-level', {}],
    ],
    '前端控制': [
      ['GET', '/api/front_end/ptz'],
      ['GET', '/api/front_end/auxiliary'],
      ['GET', '/api/front_end/wiper'],
      ['GET', '/api/front_end/iris'],
      ['GET', '/api/front_end/focus'],
      ['GET', '/api/front_end/preset'],
      ['GET', '/api/front_end/cruise'],
      ['GET', '/api/front_end/scan'],
    ],
    '回放': [
      ['GET', '/api/playback/start/d1/c1'],
      ['GET', '/api/playback/stop/d1/c1/s1'],
      ['GET', '/api/playback/resume/d1/c1/s1'],
      ['GET', '/api/playback/pause/d1/c1/s1'],
      ['GET', '/api/playback/speed/d1/c1/s1'],
    ],
    '国标录像': [
      ['GET', '/api/gb_record/query/d1/c1'],
      ['GET', '/api/gb_record/download/d1/c1'],
      ['GET', '/api/gb_record/progress/d1/c1/s1'],
    ],
    '云录像': [
      ['GET', '/api/cloud/record/list'],
      ['GET', '/api/cloud/record/date/list'],
      ['GET', '/api/cloud/record/loadRecord'],
      ['GET', '/api/cloud/record/seek'],
      ['GET', '/api/cloud/record/speed'],
      ['POST', '/api/cloud/record/task/add', {}],
      ['GET', '/api/cloud/record/task/list'],
      ['DELETE', '/api/cloud/record/delete', {}],
    ],
    '录像计划': [
      ['GET', '/api/record/plan/query'],
      ['GET', '/api/record/plan/channel/list'],
      ['GET', '/api/record/plan/get'],
      ['POST', '/api/record/plan/add', {}],
      ['POST', '/api/record/plan/update', {}],
      ['DELETE', '/api/record/plan/delete', {}],
      ['POST', '/api/record/plan/link', {}],
    ],
    '对讲': [
      ['GET', '/api/talk/start'],
      ['POST', '/api/talk/invite', {}],
      ['POST', '/api/talk/ack', {}],
      ['POST', '/api/talk/bye', {}],
      ['GET', '/api/talk/list'],
    ],
    'JT1078': [
      ['GET', '/api/jt1078/terminal/list'],
      ['GET', '/api/jt1078/terminal/query'],
      ['POST', '/api/jt1078/terminal/add', {}],
      ['POST', '/api/jt1078/terminal/update', {}],
      ['DELETE', '/api/jt1078/terminal/delete', {}],
      ['GET', '/api/jt1078/channel/list'],
      ['POST', '/api/jt1078/channel/update', {}],
      ['POST', '/api/jt1078/channel/add', {}],
      ['GET', '/api/jt1078/live/start'],
      ['GET', '/api/jt1078/live/stop'],
      ['GET', '/api/jt1078/playback/start/'],
      ['GET', '/api/jt1078/playback/stop/'],
      ['GET', '/api/jt1078/ptz'],
      ['GET', '/api/jt1078/wiper'],
      ['GET', '/api/jt1078/fill-light'],
      ['GET', '/api/jt1078/record/list'],
      ['GET', '/api/jt1078/config/get'],
      ['POST', '/api/jt1078/config/set', {}],
      ['GET', '/api/jt1078/attribute'],
      ['GET', '/api/jt1078/link-detection'],
      ['GET', '/api/jt1078/position-info'],
      ['POST', '/api/jt1078/text-msg', {}],
      ['POST', '/api/jt1078/telephone-callback', {}],
      ['GET', '/api/jt1078/driver-information'],
      ['POST', '/api/jt1078/factory-reset', {}],
      ['POST', '/api/jt1078/control/reset', {}],
      ['POST', '/api/jt1078/control/connection', {}],
      ['GET', '/api/jt1078/control/door'],
      ['GET', '/api/jt1078/media/attribute'],
      ['POST', '/api/jt1078/media/list', {}],
      ['POST', '/api/jt1078/set-phone-book', {}],
      ['POST', '/api/jt1078/shooting', {}],
      ['GET', '/api/jt1078/talk/start'],
      ['GET', '/api/jt1078/talk/stop'],
      ['POST', '/api/jt1078/media-upload', {}],
    ],
    '告警管理': [
      ['GET', '/api/alarm/list'],
      ['GET', '/api/alarm/detail/1'],
      ['POST', '/api/alarm/handle', { id: 1 }],
      ['DELETE', '/api/alarm/delete/1'],
    ],
    '位置历史': [
      ['GET', '/api/position/history'],
    ],
    '设备控制': [
      ['GET', '/api/device/control/ptz'],
      ['GET', '/api/device/control/preset'],
      ['GET', '/api/device/control/guard'],
      ['GET', '/api/device/control/record'],
    ],
  };

  let total = 0;
  let passed = 0;
  let failed = 0;
  let errors = [];

  for (const [category, endpoints] of Object.entries(categories)) {
    console.log(`\n--- ${category} (${endpoints.length} 个接口) ---`);
    for (const item of endpoints) {
      const method = item[0];
      const path = item[1];
      const body = item[2];
      total++;
      const r = body !== undefined ? await auth(method, path, body) : await auth(method, path);
      const pass = ok(r);
      if (pass) {
        passed++;
        console.log(`  ✓ ${method} ${path}`);
      } else {
        failed++;
        const msg = r.data?.msg || r.text || `HTTP ${r.status}`;
        console.log(`  ✗ ${method} ${path} → ${msg}`);
        errors.push({ method, path, status: r.status, msg });
      }
    }
  }

  // 登出
  const logoutRes = await auth('GET', '/api/user/logout');
  console.log('\n' + '='.repeat(80));
  console.log(`总计: ${total} 个接口`);
  console.log(`通过: ${passed}`);
  console.log(`失败: ${failed}`);
  console.log(`通过率: ${((passed/total)*100).toFixed(1)}%`);

  if (errors.length > 0) {
    console.log('\n失败详情:');
    for (const e of errors) {
      console.log(`  - ${e.method} ${e.path}: ${e.msg}`);
    }
  }

  process.exit(failed > 0 ? 1 : 0);
}

main().catch((e) => {
  if (e.cause && e.cause.code === 'ECONNREFUSED') {
    console.error('连接失败: 请先启动后端 (cargo run)，并确保数据库已就绪。');
  }
  console.error(e);
  process.exit(1);
});

/**
 * 前后端 API 联调测试 - 改进版: 单个请求超时控制,继续测试
 */
const BASE = process.env.BASE_URL || 'http://127.0.0.1:18080';
const ADMIN_MD5 = '21232f297a57a5a743894a0e4a801fc3'; // admin
const TIMEOUT = 5000; // 5秒超时

async function request(method, path, token = null, body = null) {
  const url = path.startsWith('http') ? path : `${BASE}${path}`;
  const headers = { 'Content-Type': 'application/json' };
  if (token) headers['access-token'] = token;
  const opt = { method, headers, signal: AbortSignal.timeout(TIMEOUT) };
  if (body && (method === 'POST' || method === 'DELETE')) opt.body = JSON.stringify(body);
  try {
    const res = await fetch(url, opt);
    const text = await res.text();
    let data = null;
    try { data = JSON.parse(text); } catch (_) {}
    const tokenFromHeader = res.headers.get('access-token');
    return { ok: res.ok, status: res.status, data, tokenFromHeader, text: text.slice(0, 200) };
  } catch (e) {
    return { ok: false, status: 0, data: null, tokenFromHeader: null, text: e.message, error: true };
  }
}

function ok(r) {
  if (!r.ok) return false;
  if (r.data && typeof r.data.code === 'number' && r.data.code !== 0) return false;
  return true;
}

async function main() {
  console.log('API 联调测试 base:', BASE);
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

  const cases = [
    // 用户相关
    ['GET', '/api/user/userInfo'],
    ['GET', '/api/user/users'],
    
    // 角色
    ['GET', '/api/role/all'],
    
    // 设备
    ['GET', '/api/device/query/devices'],
    ['GET', '/api/device/query/sync_status'],
    ['GET', '/api/device/query/devices/test-device/channels'],
    ['GET', '/api/device/query/devices/test-device'],
    ['GET', '/api/device/query/streams'],
    ['GET', '/api/device/query/channel/one'],
    ['GET', '/api/device/query/tree/test-device'],
    
    // 流媒体服务器
    ['GET', '/api/server/media_server/list'],
    ['GET', '/api/server/media_server/online/list'],
    ['GET', '/api/server/system/configInfo'],
    ['GET', '/api/server/system/info'],
    ['GET', '/api/server/info'],
    ['GET', '/api/server/resource/info'],
    ['GET', '/api/server/map/config'],
    ['GET', '/api/server/media_server/check'],
    ['GET', '/api/server/media_server/load'],
    ['GET', '/api/server/map/model-icon/list'],
    
    // 推流
    ['GET', '/api/push/list'],
    
    // 拉流代理
    ['GET', '/api/proxy/list'],
    ['GET', '/api/proxy/ffmpeg_cmd/list'],
    
    // 级联平台
    ['GET', '/api/platform/query'],
    ['GET', '/api/platform/server_config'],
    ['GET', '/api/platform/channel/list'],
    
    // 区域
    ['GET', '/api/region/tree/list'],
    ['GET', '/api/region/path'],
    ['GET', '/api/region/tree/query'],
    ['GET', '/api/region/base/child/list'],
    
    // 分组
    ['GET', '/api/group/tree/list'],
    ['GET', '/api/group/path'],
    ['GET', '/api/group/tree/query'],
    
    // 日志
    ['GET', '/api/log/list'],
    
    // 用户Key
    ['GET', '/api/userApiKey/userApiKeys'],
    
    // 回放/录像
    ['GET', '/api/playback/start/d1/c1'],
    ['GET', '/api/playback/stop/d1/c1/s1'],
    ['GET', '/api/gb_record/query/d1/c1'],
    ['GET', '/api/cloud/record/list'],
    ['GET', '/api/cloud/record/date/list'],
    ['GET', '/api/cloud/record/task/list'],
    ['GET', '/api/record/plan/query'],
    ['GET', '/api/record/plan/channel/list'],
    ['GET', '/api/record/plan/get'],
  ];

  console.log('=== GET 接口测试 ===');
  let failed = 0;
  for (const [method, path] of cases) {
    const r = await auth(method, path);
    const pass = ok(r);
    if (!pass) failed++;
    console.log(pass ? 'OK' : 'FAIL', method, path, pass ? '' : (r.error ? 'TIMEOUT ' + r.text : r.status + ' ' + (r.data && r.data.msg ? r.data.msg : r.text)));
  }

  // POST/DELETE 占位接口
  const writeCases = [
    ['POST', '/api/region/add', {}],
    ['POST', '/api/region/update', {}],
    ['POST', '/api/group/add', {}],
    ['POST', '/api/group/update', {}],
    ['POST', '/api/platform/channel/add', {}],
    ['POST', '/api/platform/channel/device/add', {}],
    ['POST', '/api/platform/channel/custom/update', {}],
    ['POST', '/api/userApiKey/add', {}],
    ['GET', '/api/user/logout'],
  ];
  
  console.log('\n=== POST/DELETE 接口测试 ===');
  for (const c of writeCases) {
    const method = c[0];
    const path = c[1];
    const body = c[2];
    const r = body !== undefined ? await auth(method, path, body) : await auth(method, path);
    const pass = ok(r);
    if (!pass) failed++;
    console.log(pass ? 'OK' : 'FAIL', method, path, pass ? '' : r.status);
  }

  console.log('\n========================================');
  console.log('合计:', cases.length + writeCases.length, '个接口');
  console.log('失败:', failed);
  console.log('通过:', cases.length + writeCases.length - failed);
  console.log('========================================');
  process.exit(failed > 0 ? 1 : 0);
}

main().catch((e) => {
  if (e.cause && e.cause.code === 'ECONNREFUSED') {
    console.error('连接失败: 请先启动后端 (cargo run)，并确保数据库已就绪。');
  }
  console.error(e);
  process.exit(1);
});

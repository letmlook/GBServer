#!/usr/bin/env python3
"""方言运行期冒烟：对真实 PostgreSQL / MySQL 实例跑读路径 + 写路径。

## 为什么需要它

`cargo test` 跑的是默认的 **SQLite**，而三种方言里只有 PostgreSQL 严格校验
占位符/列类型、MySQL 又有一批自己的语法限制（`RETURNING` / `IF NOT EXISTS`
/ `CAST(.. AS TEXT|INTEGER)` 都不支持）。本脚本就是"换一个方言再跑一遍全部接口"
的自动化版本 —— 第四十三 / 四十五轮靠它分别抓出 5 类和 4 类只在 pg / mysql
上炸的缺陷（见 docs/WVP_PARITY.md 对应小节）。

## 用法

```bash
# 1) 起库（postgres 默认就绪；mysql 用 profile）
docker compose up -d postgres
docker compose --profile mysql up -d mysql

# 2) 用对应 feature 构建并起一个实例（端口自定，注意 SIP/JT1078 端口别冲突）
cargo build --no-default-features --features postgres   # 或 mysql
GBSERVER__DATABASE__URL='postgres://postgres:postgrespw@127.0.0.1:5432/gbserver' \
  GBSERVER__SERVER__PORT=18081 \
  GBSERVER__SIP__PORT=25060 GBSERVER__SIP__TCP_PORT=25061 \
  GBSERVER__JT1078__TCP_PORT=60001 GBSERVER__JT1078__UDP_PORT=60002 \
  ./target/debug/gbserver &

# 3) 取 token 并跑冒烟
TOKEN=$(curl -s 'http://127.0.0.1:18081/api/user/login?username=admin&password=21232f297a57a5a743894a0e4a801fc3' \
  | python3 -c 'import json,sys;print(json.load(sys.stdin)["data"]["accessToken"])')
python3 scripts/dialect_smoke.py --base http://127.0.0.1:18081/api --token "$TOKEN"
```

## 预期"失败"（不是缺陷）

* `GET /log/list?format=csv` 返回的是 **CSV 文本**而不是 JSON（脚本按 JSON 判定）；
* `proxy/start` 指向一个不存在的 RTSP 源时，真实 ZLM 会回 404；
* 第二次运行时 `platform/add` / `region/add` 等会因为上一次留下的数据报"已存在"。
"""
import json, urllib.request, urllib.error

import argparse
import os

_ap = argparse.ArgumentParser(description="方言运行期冒烟（postgres / mysql）")
_ap.add_argument("--base", default=os.environ.get("SMOKE_BASE", "http://127.0.0.1:18081/api"),
                 help="后端 API 前缀，例如 http://127.0.0.1:18082/api")
_ap.add_argument("--token", default=os.environ.get("SMOKE_TOK", ""),
                 help="access-token；也可用环境变量 SMOKE_TOK")
_ap.add_argument("--token-file", default="/tmp/tok_pg.txt", help="从文件读 token（--token 优先）")
_args = _ap.parse_args()

BASE = _args.base
TOK = _args.token or open(_args.token_file).read().strip()
FAIL = []


def call(method, path, body=None, label=None, verbose=False):
    url = BASE + path
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header('access-token', TOK)
    if data:
        req.add_header('Content-Type', 'application/json')
    try:
        with urllib.request.urlopen(req, timeout=25) as r:
            raw = r.read().decode('utf-8', 'replace')
            status = r.status
    except urllib.error.HTTPError as e:
        raw = e.read().decode('utf-8', 'replace')
        status = e.code
    except Exception as e:
        return {'st': 'EXC', 'code': None, 'msg': str(e), 'data': None, 'raw': ''}
    try:
        j = json.loads(raw)
    except Exception:
        return {'st': status, 'code': 'HTML', 'msg': raw[:60], 'data': None, 'raw': raw[:200]}
    res = {'st': status, 'code': j.get('code'), 'msg': j.get('msg'), 'data': j.get('data'), 'raw': raw}
    if verbose:
        print('   raw:', raw[:400])
    return res


def check(method, path, body=None, label=None, expect_data=None, verbose=False):
    lbl = label or f"{method} {path}"
    r = call(method, path, body, verbose=verbose)
    ok = (r['st'] == 200 and r['code'] == 0)
    if ok and expect_data is not None:
        try:
            ok = expect_data(r['data'])
        except Exception as e:
            ok = False
            r['msg'] = f'data assert failed: {e}'
    print(('OK  ' if ok else 'BAD '), lbl, '->', r['st'], r['code'], str(r['msg'])[:100])
    if not ok:
        FAIL.append((lbl, r['st'], r['code'], str(r['msg'])[:200]))
    return r


# ---------------- 读路径（原有 + 修正后的真实路由） ----------------
READS = [
    ('GET', '/user/users?page=1&count=10', None),
    ('GET', '/role/all', None),
    ('GET', '/userApiKey/userApiKeys?page=1&count=10', None),
    ('GET', '/device/query/devices?page=1&count=10', None),
    ('GET', '/device/query/devices?page=1&count=10&status=ON', None),
    ('GET', '/device/query/devices?page=1&count=10&query=3402', None),
    ('GET', '/device/query/sync_status', None),
    ('GET', '/common/channel/list?page=1&count=10&online=true', None),
    ('GET', '/proxy/list?page=1&count=10&pulling=true&query=a', None),
    ('GET', '/proxy/list?page=1&count=10&pulling=false', None),
    ('GET', '/push/list?page=1&count=10&pushing=true&query=a', None),
    ('GET', '/push/list?page=1&count=10&pushing=false', None),
    ('GET', '/alarm/list?page=1&count=10', None),
    ('GET', '/alarm/list?page=1&count=10&startTime=2026-01-01%2000:00:00&endTime=2027-01-01%2000:00:00', None),
    ('GET', '/cloud/record/list?page=1&count=10', None),
    ('GET', '/cloud/record/list?page=1&count=10&startTime=2026-01-01%2000:00:00&endTime=2027-01-01%2000:00:00', None),
    ('GET', '/platform/query?page=1&count=10', None),
    ('GET', '/region/tree/list', None),
    ('GET', '/region/tree/query?parentId=-1', None),
    ('GET', '/region/tree/query?query=x', None),
    ('GET', '/group/tree/list', None),
    ('GET', '/group/tree/query?parentId=-1', None),
    ('GET', '/log/list?page=1&count=5&level=INFO', None),
    ('GET', '/log/list?format=csv&level=INFO', None),
    ('GET', '/server/media_server/list', None),
    ('GET', '/server/media_server/online/list', None),
    ('GET', '/server/media_server/load', None),
    ('GET', '/server/media_server/load?id=zlmediakit-1', None),
    ('GET', '/server/system/info', None),
    ('GET', '/server/resource/info', None),
    ('GET', '/record/plan/query?page=1&count=10', None),
    ('GET', '/jt1078/terminal/list?page=1&count=10', None),
    ('GET', '/jt1078/terminal/list?page=1&count=10&query=139', None),
    ('GET', '/sy/camera/list-with-child?page=1&count=1000', None),
    ('GET', '/sy/camera/list?count=1000', None),
    ('GET', '/server/stream/all?page=1&count=10', None),
    ('GET', '/position/history/list?deviceId=34020000001320000001&page=1&count=10', None),
    ('GET', '/platform/channel/list?platformId=1&page=1&count=10', None),
    ('GET', '/position/history/34020000001320000001', None),
]
for m, p, b in READS:
    check(m, p, b)

# ---------------- JT1078 围栏 / 路线（写 + 读 + 改 + 删） ----------------
PHONE = '13900000009'
r = check('POST', '/jt1078/area/circle/add',
          {'phoneNumber': PHONE, 'label': 'pg-smoke', 'centerLat': 30.1, 'centerLon': 120.2, 'radiusM': 500},
          label='area/circle/add', expect_data=lambda d: d.get('id', 0) > 0)
cid = (r['data'] or {}).get('id')
check('GET', f'/jt1078/area/circle/query?phone={PHONE}', label='area/circle/query(phone)',
      expect_data=lambda d: d.get('count', 0) >= 1)
check('GET', f'/jt1078/area/circle/query?phoneNumber={PHONE}', label='area/circle/query(phoneNumber)',
      expect_data=lambda d: d.get('count', 0) >= 1)
check('POST', '/jt1078/area/circle/edit',
      {'id': cid, 'label': 'pg-smoke-2', 'centerLat': 30.2, 'centerLon': 120.3, 'radius': 600},
      label='area/circle/edit')
check('POST', '/jt1078/area/circle/update',
      {'id': cid, 'label': 'pg-smoke-3', 'centerLat': 30.3, 'centerLon': 120.4, 'radius': 700},
      label='area/circle/update')

r = check('POST', '/jt1078/area/polygon/set',
          {'phoneNumber': PHONE, 'label': 'poly', 'pointsJson': '[{"lat":1,"lon":2}]'},
          label='area/polygon/set', expect_data=lambda d: d.get('id', 0) > 0)
pid = (r['data'] or {}).get('id')
check('GET', f'/jt1078/area/polygon/query?phone={PHONE}', label='area/polygon/query',
      expect_data=lambda d: d.get('count', 0) >= 1)

r = check('POST', '/jt1078/area/rectangle/add',
          {'phoneNumber': PHONE, 'label': 'rect', 'ltLat': 1.0, 'ltLon': 2.0, 'rbLat': 3.0, 'rbLon': 4.0},
          label='area/rectangle/add', expect_data=lambda d: d.get('id', 0) > 0)
rid = (r['data'] or {}).get('id')
check('GET', f'/jt1078/area/rectangle/query?phone={PHONE}', label='area/rectangle/query',
      expect_data=lambda d: d.get('count', 0) >= 1)
check('POST', '/jt1078/area/rectangle/edit',
      {'id': rid, 'label': 'rect2', 'ltLat': 1.1, 'ltLon': 2.1, 'rbLat': 3.1, 'rbLon': 4.1},
      label='area/rectangle/edit')

r = check('POST', '/jt1078/route/set',
          {'phoneNumber': PHONE, 'label': 'route', 'waypointsJson': '[{"lat":1,"lon":2}]'},
          label='route/set', expect_data=lambda d: d.get('id', 0) > 0)
rtid = (r['data'] or {}).get('id')
check('GET', f'/jt1078/route/query?phone={PHONE}', label='route/query',
      expect_data=lambda d: d.get('count', 0) >= 1)

# JT1078 终端 增/改/查/删
r = check('POST', '/jt1078/terminal/add',
          {'phoneNumber': PHONE, 'terminalId': 'T-PG-1', 'plateNo': '浙A00001', 'channelCount': 1},
          label='terminal/add')
check('GET', f'/jt1078/terminal/query?phoneNumber={PHONE}', label='terminal/query')
check('POST', '/jt1078/terminal/update',
      {'phoneNumber': PHONE, 'plateNo': '浙A00002'}, label='terminal/update')

check('GET', f'/jt1078/area/circle/delete?id={cid}', label='area/circle/delete',
      expect_data=lambda d: d.get('deleted', 0) >= 1)
check('GET', f'/jt1078/area/polygon/delete?id={pid}', label='area/polygon/delete')
check('GET', f'/jt1078/area/rectangle/delete?id={rid}', label='area/rectangle/delete')
check('GET', f'/jt1078/route/delete?id={rtid}', label='route/delete')
check('DELETE', f'/jt1078/terminal/delete?phoneNumber={PHONE}', label='terminal/delete')

# ---------------- 平台：新增 → 目录 → 注销 → 删除 ----------------
r = check('POST', '/platform/add',
          {'name': 'pg-smoke-plat', 'serverGBId': '34020000002000009999', 'serverIp': '127.0.0.1',
           'serverPort': 5060, 'deviceGBId': '34020000001320000001', 'username': 'admin',
           'password': 'admin123', 'expires': 3600, 'keepTimeout': 60, 'transport': 'UDP',
           'characterSet': 'GB2312'},
          label='platform/add', expect_data=lambda d: d.get('id', 0) > 0, verbose=True)
check('GET', '/platform/query?page=1&count=10&query=pg-smoke', label='platform/query(query)')
check('POST', '/platform/update',
      {'serverGBId': '34020000002000009999', 'name': 'pg-smoke-plat-2'}, label='platform/update')
check('GET', '/platform/exit/34020000002000009999', label='platform/exit')
check('DELETE', '/platform/delete?serverGBId=34020000002000009999', label='platform/delete')
check('DELETE', '/platform/delete?serverGbId=34020000002000009999', label='platform/delete(alias)')

# ---------------- 拉流代理：新增 → 启动 → 停止 → 删除 ----------------
APP, STREAM = 'pg-smoke', 's1'
DUMMY = 'rtsp://127.0.0.1:554/nonexistent'
check('POST', '/proxy/add',
      {'app': APP, 'stream': STREAM, 'url': DUMMY, 'name': 'pg-smoke', 'type': 'default',
       'rtspType': '0', 'enable': True},
      label='proxy/add', verbose=True)
check('GET', f'/proxy/list?page=1&count=10&query={STREAM}', label='proxy/list(query)')
check('GET', f'/proxy/start?app={APP}&stream={STREAM}', label='proxy/start')
check('GET', f'/proxy/stop?app={APP}&stream={STREAM}', label='proxy/stop')
check('DELETE', f'/proxy/delete?app={APP}&stream={STREAM}', label='proxy/delete')

# ---------------- 推流：新增 → 列表取 id → 启动 → 停止 → 删除 ----------------
PUSH_STREAM = 'pg-smoke-push1'
check('POST', '/push/add',
      {'app': APP, 'stream': PUSH_STREAM, 'name': 'pg-smoke-push', 'mediaServerId': 'zlmediakit-1'},
      label='push/add', verbose=True)
r = call('GET', f'/push/list?page=1&count=10&query={PUSH_STREAM}')
push_id = None
rows = (r['data'] or {}).get('list') or []
if rows:
    push_id = rows[0].get('id')
print('   push id =', push_id)
check('GET', f'/push/start?id={push_id}', label='push/start(by id)', verbose=True)
check('GET', f'/push/stop?id={push_id}', label='push/stop(by id)')
check('POST', f'/push/remove?id={push_id}', label='push/remove(by id)')

# ---------------- 录像计划：新增 → 改 → 删 ----------------
r = check('POST', '/record/plan/add',
          {'name': 'pg-smoke-plan', 'snap': False,
           'planItemList': [{'start': 60, 'stop': 120, 'weekDay': 1}]},
          label='record/plan/add', verbose=True)
pid2 = (r['data'] or {}).get('id') if isinstance(r['data'], dict) else None
if pid2 is None:
    q = call('GET', '/record/plan/query?page=1&count=50')
    for row in (q['data'] or {}).get('list') or []:
        if row.get('name') == 'pg-smoke-plan':
            pid2 = row.get('id')
check('GET', '/record/plan/query?page=1&count=10', label='record/plan/query')
if pid2:
    check('POST', '/record/plan/update',
          {'id': pid2, 'name': 'pg-smoke-plan-2', 'snap': True,
           'planItemList': [{'start': 30, 'stop': 60, 'weekDay': 2}]},
          label='record/plan/update', verbose=True)
    check('DELETE', f'/record/plan/delete?id={pid2}', label='record/plan/delete')

# ---------------- 区域 / 分组 ----------------
def find_node_id(tree, device_id):
    for n in tree or []:
        if n.get('deviceId') == device_id:
            return n.get('id')
        got = find_node_id(n.get('children'), device_id)
        if got:
            return got
    return None

REG_DEV = '34020000001320009998'
check('POST', '/region/add',
      {'name': 'pg-smoke-region', 'parentId': -1, 'deviceId': REG_DEV},
      label='region/add', verbose=True)
# 重复编码必须是明确的 400，而不是把唯一约束冲突抛成 500
dup = call('POST', '/region/add', {'name': 'dup', 'parentId': -1, 'deviceId': REG_DEV})
print(('OK  ' if dup['code'] == 400 and '已存在' in str(dup['msg']) else 'BAD '),
      'region/add(dup) ->', dup['st'], dup['code'], str(dup['msg'])[:80])
if not (dup['code'] == 400 and '已存在' in str(dup['msg'])):
    FAIL.append(('region/add(dup)', dup['st'], dup['code'], str(dup['msg'])[:120]))

tree = call('GET', '/region/tree/list')['data'] or []
regid = find_node_id(tree, REG_DEV)
print('   region id =', regid)
if regid:
    check('GET', f'/region/one?id={regid}', label='region/one')
    check('POST', '/region/update',
          {'id': regid, 'name': 'pg-smoke-region-2', 'parentId': -1}, label='region/update')
    check('DELETE', f'/region/delete?id={regid}', label='region/delete')

GRP_DEV = '34020000001320009997'
check('POST', '/group/add',
      {'name': 'pg-smoke-group', 'parentId': -1, 'deviceId': GRP_DEV},
      label='group/add', verbose=True)
tree = call('GET', '/group/tree/list')['data'] or []
gid = find_node_id(tree, GRP_DEV)
print('   group id =', gid)
if gid:
    check('GET', f'/group/one?id={gid}', label='group/one')
    check('POST', '/group/update',
          {'id': gid, 'name': 'pg-smoke-group-2', 'parentId': -1},
          label='group/update')
    check('DELETE', f'/group/delete?id={gid}', label='group/delete')

# ---------------- 告警处置 ----------------
al = call('GET', '/alarm/list?page=1&count=1')
if al['code'] == 0 and (al['data'] or {}).get('list'):
    aid = al['data']['list'][0]['id']
    check('POST', '/alarm/handle', {'id': aid, 'handleUser': 'admin', 'handleResult': 'ok'},
          label='alarm/handle', verbose=True)
    check('GET', f'/alarm/detail/{aid}', label='alarm/detail')
else:
    print('SKIP alarm/handle（库里没有告警行）')

# ---------------- 用户 API Key ----------------
r = check('POST', '/userApiKey/add',
          {'userId': 1, 'app': 'pg-smoke', 'remark': 'smoke', 'enable': True},
          label='userApiKey/add', verbose=True)
kdata = r['data'] if isinstance(r['data'], dict) else None
kid = (kdata or {}).get('id')
check('GET', '/userApiKey/userApiKeys?page=1&count=10', label='userApiKey/list')
if kid:
    check('POST', '/userApiKey/disable', {'id': kid}, label='userApiKey/disable')
    check('POST', '/userApiKey/enable', {'id': kid}, label='userApiKey/enable')
    check('DELETE', f'/userApiKey/delete?id={kid}', label='userApiKey/delete')

# ---------------- 媒体服务器 ----------------
check('GET', '/server/media_server/check?id=zlmediakit-1', label='media_server/check')
check('GET', '/server/media_server/one/zlmediakit-1', label='media_server/one')
check('GET', '/server/media_server/load?id=zlmediakit-1', label='media_server/load')

# ---------------- 日志导出 ----------------
r = call('GET', '/log/list?format=csv&level=INFO')
print('CSV 导出:', r['st'], str(r['msg'])[:60], '| 首行:', str(r['raw'])[:60].replace('\n', '\\n'))

print()
print('=== 失败项:', len(FAIL))
for f in FAIL:
    print('  ', f)

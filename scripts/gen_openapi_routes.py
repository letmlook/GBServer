#!/usr/bin/env python3
"""从 handler 注解生成各域 OpenAPI 路由模块，并与 router.rs 交叉校验。

用法：在仓库根目录运行 `python3 /tmp/openapi_gen/gen.py`
"""
import re
import os
import json
from collections import defaultdict

HANDLERS = 'src/handlers'
OUT = 'src/openapi'

DOMAINS = {
    'region':       ['region'],
    'platform':     ['platform'],
    'cloud_record': ['cloud_record_extra'],
    'channel':      ['common_channel', 'front_end', 'position'],
    'device':       ['device', 'device_query', 'device_stub', 'device_control', 'device_batch'],
    'live':         ['play', 'playback', 'talk', 'webrtc'],
    'stream':       ['stream', 'rtp_control'],
    'user':         ['user', 'role', 'alarm', 'parity_extras', 'log'],
    'jt1078':       ['jt1078', 'jt1078_extra'],
    'system':       ['server', 'system'],
    'sy':           ['sy_camera'],
    'misc':         ['stub'],
    'health':       ['health', 'metrics', 'websocket'],
}

# 已迁出 router.rs 的域（其字符串路由已被 documented_routes 取代）
ALREADY_MIGRATED = {'region'}

# router.rs 里的模块名 -> 注解所在模块名（不一致的映射）
MODULE_ALIAS = {'metrics_handler': 'metrics'}

REVERSE_ALIAS = {v: k for k, v in MODULE_ALIAS.items()}

# 注解在 src/handlers/ 之外、由 routes_public.rs 手写登记的 handler
EXTRA_ANNOTATED = {('zlm_hook', 'handle_webhook'), ('router', 'rpc_endpoint')}

ann_re = re.compile(
    r'#\[utoipa::path\(\s*(?P<body>.*?)\n\s*\)\]\s*\n'
    r'(?:#\[[^\]]*\]\s*\n)*\s*pub async fn (?P<fn>\w+)\(',
    re.DOTALL,
)


def parse_ann(body):
    m = re.match(r'\s*(\w+)\s*,', body)
    method = m.group(1) if m else '?'
    pm = re.search(r'path\s*=\s*"([^"]+)"', body)
    return method, (pm.group(1) if pm else None)


def scan(module):
    path = os.path.join(HANDLERS, f'{module}.rs')
    if not os.path.exists(path):
        return []
    text = open(path).read()
    out = []
    for m in ann_re.finditer(text):
        method, p = parse_ann(m.group('body'))
        # 分类依据：注解里是否声明 security(...)。公开端点（登录/健康/分享/
        # WebSocket/hook/RPC）一律不声明 security，因此这是自描述且不依赖
        # router.rs 的判据（router.rs 的字符串路由已在接线后移除）。
        section = 'protected' if 'security(' in m.group('body') else 'public'
        out.append({'fn': m.group('fn'), 'method': method, 'path': p,
                    'module': module, 'section': section})
    return out


def fn_count(module):
    path = os.path.join(HANDLERS, f'{module}.rs')
    if not os.path.exists(path):
        return 0
    return len(re.findall(r'pub async fn \w+\(', open(path).read()))


def section_of_line(raw, lineno):
    """按行号判断路由属于哪个段（public / protected）。"""
    marks = {
        'api_protected': None, 'api_public': None,
        'zlm_protected': None, 'app_route': None,
    }
    for i, ln in enumerate(raw.split('\n'), start=1):
        if 'let api_protected = Router::new()' in ln: marks['api_protected'] = i
        if 'let api_public = Router::new()' in ln: marks['api_public'] = i
        if 'let zlm_protected = Router::new()' in ln: marks['zlm_protected'] = i
        if 'let app = app.route(' in ln and marks['app_route'] is None: marks['app_route'] = i
    # 段边界（按行号从后往前判断，注意 app.route 段在 zlm_protected 之后）
    if marks['app_route'] and lineno >= marks['app_route']:
        # /api/ws、/api/talk/audio/... —— 在 with_state 之后注册，不经过鉴权中间件，
        # JWT 由 handler 内部用 ?token= 校验。**必须判为 public**，否则接入文档路由后
        # 会被 auth_middleware 拦住 WebSocket 握手。
        return 'public'
    if marks['zlm_protected'] and lineno >= marks['zlm_protected']:
        return 'protected'          # zlm 反向代理受鉴权保护
    if marks['api_public'] and lineno >= marks['api_public']:
        return 'public'
    return 'protected'


def parse_router():
    """提取 (module, fn) -> {(METHOD, path, section)}。"""
    raw = open('src/router.rs').read()
    stripped = '\n'.join(re.sub(r'//.*$', '', ln) for ln in raw.split('\n'))
    out = defaultdict(set)
    for m in re.finditer(r'\.route\s*\(', stripped):
        lineno = stripped[:m.start()].count('\n') + 1
        section = section_of_line(raw, lineno)
        i = m.end()
        depth = 1
        while i < len(stripped) and depth:
            if stripped[i] == '(':
                depth += 1
            elif stripped[i] == ')':
                depth -= 1
            i += 1
        args = stripped[m.end():i - 1]
        pm = re.search(r'"([^"]+)"', args)
        if not pm:
            continue
        path = pm.group(1)
        for mm in re.finditer(r'(get|post|delete|put|patch)\(\s*([a-zA-Z_][\w:]*)\s*\)', args):
            verb, handler = mm.group(1), mm.group(2)
            parts = handler.split('::')
            fn = parts[-1]
            mod = parts[-2] if len(parts) >= 2 else 'router'
            out[(mod, fn)].add((verb.upper(), path, section))
    return out


def main():
    router = parse_router()
    report = {}
    all_entries = []
    for domain, modules in DOMAINS.items():
        entries = []
        for mod in modules:
            entries.extend(scan(mod))
        if not entries:
            continue
        # section 已在 scan() 中按注解自描述确定
        report[domain] = {'count': len(entries), 'modules': modules}
        all_entries.extend(entries)

        prot = [e for e in entries if e.get('section', 'protected') == 'protected']
        pub = [e for e in entries if e.get('section') == 'public']

        def emit(fname, kind, items):
            lines = [
                f'//! `{domain}` 域的 {kind} OpenAPI 路由注册。',
                '//!',
                '//! 逐条 `routes!()` 注册（一次只能放一条）；迁移说明与三个坑见',
                '//! `src/openapi/routes_region.rs` 文件头。',
                '//!',
                '//! 本文件内容由 handler 上的 `#[utoipa::path]` 注解决定：新增接口 =',
                '//! 写注解 + 重新跑生成脚本，不要手写路径字符串。',
                '',
                'use utoipa_axum::routes;',
                '',
                'use super::{DocumentedRoutes, RoutesAccumulator};',
                '',
                f'pub fn routes() -> DocumentedRoutes {{',
                '    let mut acc = RoutesAccumulator::default();',
            ]
            for e in sorted(items, key=lambda x: (x['module'], x['fn'])):
                lines.append(f'    acc.add(routes!(crate::handlers::{e["module"]}::{e["fn"]}));')
            lines += ['', '    acc.finish()', '}', '']
            open(os.path.join(OUT, fname), 'w').write('\n'.join(lines))

        if prot:
            emit(f'routes_{domain}.rs', '受保护', prot)
        else:
            stale = os.path.join(OUT, f'routes_{domain}.rs')
            if os.path.exists(stale):
                os.remove(stale)
        if pub:
            emit(f'routes_{domain}_public.rs', '公开', pub)
        else:
            stale = os.path.join(OUT, f'routes_{domain}_public.rs')
            if os.path.exists(stale):
                os.remove(stale)

    # ---------- 生成聚合 ----------
    def agg(name, doc, pairs):
        lines = [
            f'//! {doc}',
            '//!',
            '//! **本文件由生成脚本产出**：新增域时重新生成，不要手改。',
            '',
            'use super::{DocumentedRoutes, RoutesAccumulator};',
            '',
            f'pub fn {name}() -> DocumentedRoutes {{',
            '    let mut acc = RoutesAccumulator::default();',
        ]
        for d in sorted(pairs):
            lines.append(f'    acc.merge(super::{d}::routes());')
        lines += ['', '    acc.finish()', '}', '']
        open(os.path.join(OUT, f'registry_{name}.rs'), 'w').write('\n'.join(lines))

    prot_mods, pub_mods = [], []
    for domain, modules in DOMAINS.items():
        has_prot = any(e['module'] in modules and e.get('section') == 'protected'
                       for e in all_entries)
        has_pub = any(e['module'] in modules and e.get('section') == 'public'
                      for e in all_entries)
        if has_prot:
            prot_mods.append(f'routes_{domain}')
        if has_pub:
            pub_mods.append(f'routes_{domain}_public')
    # 手写模块（src/handlers 与 src/router.rs 之外的公开 handler：zlm hook / rpc）
    pub_mods.append('routes_public')
    agg('protected_routes', '受保护路由聚合（挂鉴权 + 审计中间件）。', prot_mods)
    agg('public_routes', '公开路由聚合（不挂鉴权中间件）。', pub_mods)

    # ---------- 校验 ----------
    problems = []
    seen = set()
    for e in all_entries:
        key = (e['module'], e['fn'])
        if key in seen:
            problems.append(f'DUPLICATE annotation: {e["module"]}::{e["fn"]}')
        seen.add(key)
        if e['module'] in ALREADY_MIGRATED:
            continue
        r = router.get(key) or router.get((REVERSE_ALIAS.get(e['module'], e['module']), e['fn']))
        if r is None:
            problems.append(
                f'NOT IN ROUTER: {e["module"]}::{e["fn"]} (annotation path={e["path"]})'
            )
        else:
            got = {(a, b) for a, b, _ in r}
            if (e['method'].upper(), e['path']) not in got:
                problems.append(
                    f'PATH MISMATCH {e["module"]}::{e["fn"]}: '
                    f'annotation={e["method"].upper()} {e["path"]} router={sorted(got)}'
                )

    by_path = defaultdict(list)
    for e in all_entries:
        if e['path']:
            by_path[(e['method'].upper(), e['path'])].append(f'{e["module"]}::{e["fn"]}')
    for (verb, p), fns in sorted(by_path.items()):
        if len(fns) > 1:
            problems.append(f'DUPLICATE PATH {verb} {p} -> {fns}')

    # ---- 覆盖率指标：每条**被路由**的 handler 是否都有注解 ----
    annotated_keys = {(e['module'], e['fn']) for e in all_entries} | EXTRA_ANNOTATED
    routed_keys = {(MODULE_ALIAS.get(m, m), f) for m, f in router.keys()}
    migrated_keys = {(e['module'], e['fn']) for e in all_entries
                     if e['module'] in ALREADY_MIGRATED}
    undocumented = sorted(routed_keys - annotated_keys - migrated_keys)
    phantom = sorted(annotated_keys - routed_keys - migrated_keys)

    missing = [f'未文档化的路由 {len(undocumented)} 条']
    missing += [f'  {m}::{f}' for m, f in undocumented[:60]]
    missing += [f'多余注解（函数未被路由）{len(phantom)} 条']
    missing += [f'  {m}::{f}' for m, f in phantom[:20]]

    print(json.dumps({d: v['count'] for d, v in report.items()}, ensure_ascii=False))
    print(f'TOTAL annotated: {len(all_entries)}')
    print('--- 未注解完的模块 ---')
    for m in missing:
        print('  ', m)
    print(f'--- 校验问题 {len(problems)} 条 ---')
    for p in problems[:60]:
        print('  ', p)

    json.dump({'report': report, 'problems': problems, 'missing': missing,
               'entries': all_entries},
              open('/tmp/openapi_gen/inventory.json', 'w'), ensure_ascii=False, indent=2)


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""把「三方言 cfg 分支」改写成「运行期 dispatch」。

GBServer 改造前 `db::Pool` 是编译期类型别名，每个 db 函数用
`#[cfg(feature = "mysql"/"postgres"/"sqlite")]` 抄三份 SQL。
改造后 Pool 变成运行期多态枚举，三份分支必须收敛成
「一份 SQL 文本 + 一次 dispatch!(pool, |p| { ... })」。

只处理**三方分支除 SQL 文本外完全一致**的组；其余列入人工清单。
SQL 文本若 mysql 与 pg/sqlite 不同（RETURNING / ON CONFLICT / CAST 等），
自动生成 `match pool.dialect()` 选文本。
"""
import re
import sys
import pathlib
import json
from collections import Counter, defaultdict

CFG_RE = re.compile(r'^(\s*)#\[cfg\((?:all\()?feature = "(mysql|postgres|sqlite)"\)\)?\]\s*$')
PLACEHOLDER_RE = re.compile(r'\$\d+')


# ---------------------------------------------------------------- 扫描工具
def find_block_end(lines, start):
    depth = 0
    for i in range(start, len(lines)):
        line = lines[i]
        in_str = None
        j = 0
        while j < len(line):
            c = line[j]
            if in_str:
                if c == '\\':
                    j += 2
                    continue
                if c == in_str:
                    in_str = None
                j += 1
                continue
            if c in ('"', "'"):
                in_str = c
                j += 1
                continue
            if c == '{':
                depth += 1
            elif c == '}':
                depth -= 1
                if depth == 0:
                    return i
            j += 1
    return len(lines) - 1


def find_stmt_end(lines, start):
    if lines[start].strip().startswith('{'):
        return find_block_end(lines, start)
    depth = 0
    for i in range(start, len(lines)):
        line = lines[i]
        in_str = None
        j = 0
        while j < len(line):
            c = line[j]
            if in_str:
                if c == '\\':
                    j += 2
                    continue
                if c == in_str:
                    in_str = None
                j += 1
                continue
            if c in ('"', "'"):
                in_str = c
                j += 1
                continue
            if c == '(':
                depth += 1
            elif c == ')':
                depth -= 1
            elif c == ';' and depth == 0:
                return i
            j += 1
    return len(lines) - 1


def extract_literals(text):
    """返回 [(start, end, raw_source)]，普通字符串 + raw string，跳过字符字面量。"""
    out = []
    i = 0
    n = len(text)
    while i < n:
        c = text[i]
        if c == 'r' and i + 1 < n and text[i + 1] in '#"':
            hashes = 0
            j = i + 1
            while j < n and text[j] == '#':
                hashes += 1
                j += 1
            if j < n and text[j] == '"':
                closing = '"' + '#' * hashes
                end = text.find(closing, j + 1)
                if end != -1:
                    out.append((i, end + len(closing), text[i:end + len(closing)]))
                    i = end + len(closing)
                    continue
            i += 1
            continue
        if c == '"':
            j = i + 1
            while j < n:
                if text[j] == '\\':
                    j += 2
                    continue
                if text[j] == '"':
                    break
                j += 1
            out.append((i, j + 1, text[i:j + 1]))
            i = j + 1
            continue
        i += 1
    return out


def normalize_sql(s):
    return PLACEHOLDER_RE.sub('?', s)


BIND_RE = re.compile(r'\.bind\s*\(([^;]*?)\)')


def skeleton_loose(text, sql_span):
    """忽略 bind 参数文本（只保留 bind 的存在与位置）的结构骨架。"""
    s = skeleton(text, sql_span)
    return re.sub(r'(\.bind\s*\([^;]*?\)\s*)+', '.bind(@B@)', s)


def bind_args(text):
    return [m.group(1).strip() for m in BIND_RE.finditer(text)]


def skeleton(text, sql_span):
    """把 SQL 字面量与其余字符串、pool 标识符都抹掉，只留结构。"""
    lits = extract_literals(text)
    out = []
    prev = 0
    for idx, (s, e, raw) in enumerate(lits):
        out.append(text[prev:s])
        out.append('@S%d@' % idx if idx != sql_span else '@SQL@')
        prev = e
    out.append(text[prev:])
    s = ''.join(out)
    s = re.sub(r'\bpool\b', '@P@', s)
    return re.sub(r'[ \t]+', ' ', s).strip()


# ---------------------------------------------------------------- 分组
def collect_groups(lines):
    items = []
    for idx, line in enumerate(lines):
        m = CFG_RE.match(line)
        if not m:
            continue
        indent, dialect = m.group(1), m.group(2)
        nxt = idx + 1
        while nxt < len(lines) and lines[nxt].strip() == '':
            nxt += 1
        if nxt >= len(lines):
            continue
        body = lines[nxt].strip()
        if body.startswith('return '):
            kind = 'return'
        elif body.startswith('let '):
            kind = 'let'
        elif body.startswith('{'):
            kind = 'block'
        else:
            kind = 'other'
        end = find_stmt_end(lines, nxt)
        items.append({
            'indent': indent, 'dialect': dialect, 'kind': kind,
            'cfg_line': idx, 'body_line': nxt, 'end_line': end,
        })

    groups = []
    i = 0
    while i < len(items):
        chunk = items[i:i + 3]
        if len(chunk) == 3 and {c['dialect'] for c in chunk} == {'mysql', 'postgres', 'sqlite'} \
                and len({c['kind'] for c in chunk}) == 1 \
                and chunk[0]['indent'] == chunk[1]['indent'] == chunk[2]['indent']:
            ok = True
            for a, b in zip(chunk, chunk[1:]):
                for k in range(a['end_line'] + 1, b['cfg_line']):
                    if lines[k].strip() and not lines[k].strip().startswith('//'):
                        ok = False
            if ok:
                groups.append(chunk)
                i += 3
                continue
        i += 1
    return groups, items


def branch_text(lines, c):
    """分支语句的源码文本（去掉 cfg 行本身）。"""
    return '\n'.join(lines[c['body_line']:c['end_line'] + 1])


def group_sql(chunk, lines):
    out = {}
    for c in chunk:
        text = branch_text(lines, c)
        lits = extract_literals(text)
        out[c['dialect']] = lits[0] if lits else None
    return out


def is_tail(lines, chunk):
    """block 组是否位于函数末尾（其后没有同缩进的语句）。"""
    last = chunk[-1]
    k = last['end_line'] + 1
    while k < len(lines):
        s = lines[k]
        if s.strip() == '':
            k += 1
            continue
        if len(s) - len(s.lstrip()) <= len(last['indent']):
            return True
        return False
    return True


# ---------------------------------------------------------------- 转换
def build_replacement(lines, chunk, sqls):
    """返回 (replacement_lines, note)；失败返回 (None, reason)。"""
    kind = chunk[0]['kind']
    by = {c['dialect']: c for c in chunk}

    # --- 1. 结构一致性：三方分支除 SQL 外必须一致 ---
    skels = {}
    binds = {}
    for d, c in by.items():
        text = branch_text(lines, c)
        lits = extract_literals(text)
        skels[d] = skeleton_loose(text, 0 if not lits else None)
        binds[d] = bind_args(text)
    if len(set(skels.values())) != 1:
        return None, 'skeleton_differs'
    # bind 参数文本三方不一致 → 转换后按 sqlite 模板绑定，记录供人工复核
    if len({tuple(v) for v in binds.values()}) != 1:
        REVIEW.append('bind_args_differ %s:%d %s'
                      % (path_hint, chunk[0]['cfg_line'] + 1,
                         {d: v for d, v in binds.items()}))

    # --- 2. SQL 文本分组：mysql 单独一份，其余一份 ---
    norm = {d: normalize_sql(sqls[d][2]) for d in sqls}
    raw = {d: sqls[d][2] for d in sqls}  # 原始字面量源码（含引号）
    mysql_differs = norm['mysql'] != norm['sqlite']
    pg_differs = norm['postgres'] != norm['sqlite']

    CS = 'crate::db::dialect::current_sql'
    if mysql_differs and pg_differs:
        # 三个方言各一份文本
        sql_stmt = (
            '    let sql = match crate::db::dialect::current() {\n'
            '        crate::db::Dialect::MySql => %s(%s).into_owned(),\n'
            '        crate::db::Dialect::Sqlite => %s(%s).into_owned(),\n'
            '        _ => %s(%s).into_owned(),\n'
            '    };' % (CS, raw['mysql'], CS, raw['sqlite'], CS, raw['postgres'])
        )
    elif mysql_differs:
        sql_stmt = (
            '    let sql = match crate::db::dialect::current() {\n'
            '        crate::db::Dialect::MySql => %s(%s).into_owned(),\n'
            '        _ => %s(%s).into_owned(),\n'
            '    };' % (CS, raw['mysql'], CS, raw['sqlite'])
        )
    elif pg_differs:
        sql_stmt = (
            '    let sql = match crate::db::dialect::current() {\n'
            '        crate::db::Dialect::Postgres => %s(%s).into_owned(),\n'
            '        _ => %s(%s).into_owned(),\n'
            '    };' % (CS, raw['postgres'], CS, raw['sqlite'])
        )
    else:
        sql_stmt = '    let sql = %s(%s).into_owned();' % (CS, raw['sqlite'])

    # --- 3. body 模板：取 mysql 分支（占位符是 ?），把 SQL 字面量换成 &sql，pool 换成 p ---
    tmpl_c = by['mysql']
    tmpl_text = branch_text(lines, tmpl_c)
    lits = extract_literals(tmpl_text)
    if not lits:
        return None, 'no_literal'
    # SQL 由 format! 之类的宏生成时，**不能**提取它的格式串当 SQL
    if re.search(r'\b(format|println|print|write|writeln)!\s*\(', tmpl_text[:lits[0][0]]):
        return None, 'sql_in_format_macro'
    s0, e0, _ = lits[0]
    body = tmpl_text[:s0] + '&sql' + tmpl_text[e0:]
    body = re.sub(r'(?<![.\w])pool\b(?!\s*\.)', 'p', body)

    indent = chunk[0]['indent']
    # 该函数里 pool 的实际引用（handlers 层通常是 state.pool）
    fn_head = None
    for k in range(chunk[0]['body_line'], -1, -1):
        if re.match(r'\s*(pub\s+)?(async\s+)?fn\s', lines[k]):
            fn_head = k
            break
    pref = pool_ref(lines, fn_head) if fn_head is not None else 'pool'

    if kind == 'return':
        body = body.rstrip()
        if body.endswith(';'):
            body = body[:-1]
        # 宏负责 await：body 里去掉末尾的 .await（及可能的 ?）
        if body.endswith('.await'):
            body = body[:-len('.await')].rstrip()
        # body 里的 `return` 会**绕过 dispatch 直接返回外层函数** —— 必须去掉
        body = re.sub(r'\breturn\s+', '', body)
        macro = 'crate::dispatch_exec!' if '.execute(' in body else 'crate::dispatch!'
        return [sql_stmt, '%sreturn %s(%s, |p| {' % (indent, macro, pref),
                '%s    %s' % (indent, body.strip()),
                '%s});' % indent], None

    if kind == 'let':
        m = re.match(r'\s*let\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)$', body, re.S)
        if not m:
            return None, 'let_parse'
        name, expr = m.group(1), m.group(2)
        expr = expr.rstrip()
        if expr.endswith(';'):
            expr = expr[:-1].rstrip()
        trailing_q = expr.endswith('?')
        if trailing_q:
            expr = expr[:-1].rstrip()
        if not expr.endswith('.await'):
            return None, 'let_no_await'
        if '?' in expr[:-len('.await')]:
            # 中间还有 `?` —— 它会 return 外层函数，arm 类型变成 !，无法再赋值
            return None, 'let_inner_question'
        # 宏负责 await：去掉末尾的 .await
        expr = expr[:-len('.await')].rstrip()
        suffix = '?'
        return [sql_stmt,
                '%slet %s = %s(%s, |p| {' % (indent, name,
                    'crate::dispatch_exec!' if '.execute(' in expr else 'crate::dispatch!', pref),
                '%s    %s' % (indent, expr.strip()),
                '%s})%s;' % (indent, suffix)], None

    if kind == 'block':
        if not is_tail(lines, chunk):
            return None, 'block_not_tail'
        # **用替换后的 body**（`pool` → `p`、SQL 字面量 → `&sql`），
        # 不能用原文 tmpl_text —— 否则宏体里留着 `pool`，E0277。
        inner = body.strip()
        assert inner.startswith('{') and inner.endswith('}'), inner[:80]
        inner = inner[1:-1]
        inner_lines = [ln for ln in inner.split('\n')]
        while inner_lines and inner_lines[0].strip() == '':
            inner_lines.pop(0)
        while inner_lines and inner_lines[-1].strip() == '':
            inner_lines.pop()
        if not inner_lines:
            return None, 'block_empty'
        # 宏负责 await：inner 末尾去掉 .await（若有）
        while inner_lines:
            last = inner_lines[-1].strip()
            if last.endswith('.await'):
                inner_lines[-1] = last[:-len('.await')].rstrip()
            break
        body_lines = ['%s    %s' % (indent, ln) for ln in inner_lines]
        macro = ('crate::dispatch_exec!' if '.execute(' in inner else 'crate::dispatch!')
        return ([sql_stmt,
                '%sreturn %s(%s, |p| {' % (indent, macro, pref)]
                + body_lines
                + ['%s});' % indent], None)

    return None, 'kind_' + kind


REVIEW = []



def pool_ref(lines, fn_head):
    """返回该函数里 pool 的**实际引用表达式**。

    - `pool: &Pool` → "pool"
    - `State(state): State<AppState>` → "state.pool"
    - `pool: Option<crate::db::Pool>` → "pool"（调用点再解包）
    """
    # 扫函数签名（前 12 行）
    head = '\n'.join(lines[fn_head:fn_head + 12])
    m = re.search(r'\b(\w+)\s*:\s*&\s*(?:crate::db::)?Pool\b', head)
    if m:
        return m.group(1)
    m = re.search(r'\b(\w+)\s*:\s*State\s*\(\s*AppState\s*\)', head)
    if m:
        return m.group(1) + '.pool'
    m = re.search(r'\b(\w+)\s*:\s*&\s*AppState\b', head)
    if m:
        return m.group(1) + '.pool'
    return 'pool'

def main():
    global path_hint
    apply = '--apply' in sys.argv
    root = pathlib.Path('src')
    manual = defaultdict(list)
    stats = Counter()
    total_groups = 0

    for path in sorted(root.rglob('*.rs')):
        path_hint = str(path)
        text = path.read_text()
        lines = text.split('\n')
        groups, items = collect_groups(lines)
        total_groups += len(groups)
        covered = set()
        edits = []

        for g in groups:
            for c in g:
                covered.add(c['cfg_line'])
            sqls = group_sql(g, lines)
            if any(v is None for v in sqls.values()):
                manual['no_sql_literal'].append(f"{path}:{g[0]['cfg_line']+1}")
                stats['no_sql_literal'] += 1
                continue
            res = build_replacement(lines, g, sqls)
            if res is None:
                print('  !! build_replacement 返回 None：%s:%d kind=%s'
                      % (path, g[0]['cfg_line'] + 1, g[0]['kind']))
                manual['builder_none'].append(f"{path}:{g[0]['cfg_line']+1}")
                stats['builder_none'] += 1
                continue
            repl, note = res
            if repl is None:
                manual[note].append(f"{path}:{g[0]['cfg_line']+1} kind={g[0]['kind']}")
                stats[note] += 1
                continue
            stats['converted_' + g[0]['kind']] += 1
            edits.append((g[0]['cfg_line'], g[-1]['end_line'], repl))

        for it in items:
            if it['cfg_line'] in covered:
                continue
            manual['ungrouped_' + it['kind']].append(
                f"{path}:{it['cfg_line']+1} ({it['dialect']})")
            stats['ungrouped_' + it['kind']] += 1

        if apply and edits:
            # 从后往前替换，避免行号漂移
            new_lines = list(lines)
            for start, end, repl in sorted(edits, key=lambda e: -e[0]):
                new_lines[start:end + 1] = repl
            path.write_text('\n'.join(new_lines))

    print("=== 统计（总三元组 %d） ===" % total_groups)
    for k, v in sorted(stats.items()):
        print(f"  {v:5d}  {k}")
    print()
    for key in sorted(manual):
        print(f"=== {key} ({len(manual[key])}) ===")
        for item in manual[key][:30]:
            print("   ", item)
        if len(manual[key]) > 30:
            print(f"    ... 共 {len(manual[key])} 处")
        print()

    print("=== bind 参数需人工复核 (%d) ===" % len(REVIEW))
    for r in REVIEW[:25]:
        print("   ", r)
    if len(REVIEW) > 25:
        print("    ... 共 %d 处" % len(REVIEW))
    print()

    if apply:
        with open('/tmp/convert_review.json', 'w') as f:
            json.dump(REVIEW, f, ensure_ascii=False, indent=1)
        with open('/tmp/convert_manual.json', 'w') as f:
            json.dump({k: v for k, v in manual.items()}, f, ensure_ascii=False, indent=1)
        print("已写入改动；人工清单见 /tmp/convert_manual.json")


if __name__ == '__main__':
    main()

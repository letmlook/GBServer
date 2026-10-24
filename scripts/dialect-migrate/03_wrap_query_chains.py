#!/usr/bin/env python3
"""把「直接把 pool 传给 sqlx」的查询链包进 dispatch。

与前两版脚本的根本区别：**替换范围 = 扫描出来的链的范围**，
不做"链首..链尾"的启发式猜测 —— 因此结构上不可能吞掉旁边的代码。

扫描用括号平衡（跳过字符串），从 `sqlx::query*` 一直扫到 `.await`。
"""
import re
import sys
import pathlib
from collections import defaultdict

ENTRY_RE = re.compile(r'sqlx::query(?:_as|_scalar)?\b')
METHOD_RE = re.compile(r'\s*\.\w+\s*\(')
POOL_RE = re.compile(r'(?<![.\w])pool\b(?!\s*\.)')


def skip_string(src, i):
    """src[i] 是引号，返回字符串结束后的位置。"""
    q = src[i]
    i += 1
    while i < len(src):
        if src[i] == '\\':
            i += 2
            continue
        if src[i] == q:
            return i + 1
        i += 1
    return i


def scan_call(src, i):
    """src[i] 是 '('，返回配对 ')' 之后的位置。"""
    depth = 0
    while i < len(src):
        c = src[i]
        if c == '(':
            depth += 1
        elif c == ')':
            depth -= 1
            if depth == 0:
                return i + 1
        elif c in '"\'':
            i = skip_string(src, i)
            continue
        i += 1
    return -1


def scan_chain(src, i):
    """src[i] 处是 sqlx::query*，返回整条链的结束位置（`.await` 之后）。

    返回 (end, had_await)；失败返回 None。
    """
    m = ENTRY_RE.match(src, i)
    if not m:
        return None
    j = src.find('(', m.end())
    if j == -1:
        return None
    k = scan_call(src, j)
    if k == -1:
        return None
    # 链式调用
    while True:
        mm = METHOD_RE.match(src, k)
        if not mm:
            break
        k2 = scan_call(src, mm.end() - 1)
        if k2 == -1:
            return None
        k = k2
    # 结尾必须是 .await
    mm = re.match(r'\s*\.await\b', src[k:])
    if not mm:
        return None
    return (k + mm.end(), True)


def sql_literal(chain):
    """取 `sqlx::query*(...)` **第一个参数**里的字面量。

    不能用「整段文本的第一个字面量」—— `sqlx::query(&format!("{}", COLS))`
    的第一个字面量是 format! 的**格式串**，替换掉会破坏 format 调用。
    """
    m = ENTRY_RE.match(chain)
    if not m:
        return None
    j = chain.find('(', m.end())
    if j == -1:
        return None
    # 参数区：从 j+1 到配对的 ) 前
    depth = 0
    k = j
    while k < len(chain):
        c = chain[k]
        if c == '(':
            depth += 1
        elif c == ')':
            depth -= 1
            if depth == 0:
                break
        elif c in '"\'':
            k = skip_string(chain, k)
            continue
        k += 1
    args = chain[j + 1:k]
    # 参数里若出现 format! / println! 等宏，则不提取（SQL 由宏生成）
    if re.search(r'\b(format|println|print|write|writeln)!\s*\(', args):
        return None
    lit = first_literal(args)
    if not lit:
        return None
    return (lit[0] + j + 1, lit[1] + j + 1, lit[2])


def first_literal(text):
    """返回 (start, end, raw) —— 第一个普通字符串字面量。"""
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
                    return (i, end + len(closing), text[i:end + len(closing)])
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
            return (i, j + 1, text[i:j + 1])
        i += 1
    return None



FNSIG_RE = re.compile(r'(?:pub\s+)?(?:async\s+)?fn\s+\w+\s*\([^)]*\)', re.S)


def pool_ref_text(src, pos):
    """按**文本**找 pool 的实际引用（wrap_dispatch 是按整文件文本扫描的）。

    - `pool: &Pool` → "pool"
    - `State(state): State<AppState>` → "state.pool"
    """
    head = src[:pos]
    matches = list(FNSIG_RE.finditer(head))
    if not matches:
        return 'pool'
    sig = matches[-1].group(0)
    m = re.search(r'\b(\w+)\s*:\s*&\s*(?:crate::db::)?Pool\b', sig)
    if m:
        return m.group(1)
    m = re.search(r'\b(\w+)\s*:\s*State\s*\(\s*AppState\s*\)', sig)
    if m:
        return m.group(1) + '.pool'
    m = re.search(r'\b(\w+)\s*:\s*&\s*AppState\b', sig)
    if m:
        return m.group(1) + '.pool'
    return 'pool'


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
    apply = '--apply' in sys.argv
    root = pathlib.Path('src')
    total = 0
    skipped = defaultdict(int)

    for path in sorted(root.rglob('*.rs')):
        src = path.read_text()
        out = []
        i = 0
        n = len(src)
        changed = False
        while i < n:
            m = ENTRY_RE.search(src, i)
            if not m:
                out.append(src[i:])
                break
            start = m.start()
            scanned = scan_chain(src, start)
            if scanned is None:
                out.append(src[i:start + 1])
                i = start + 1
                continue
            end, _ = scanned
            chain = src[start:end]

            # 链里必须真的把 pool 交给 sqlx
            if not re.search(r'\.\w+\s*\(\s*pool\s*\)', chain):
                out.append(src[i:start + 1])
                i = start + 1
                continue

            # 该链是否已经在一个 dispatch 里？
            before = src[:start]
            if before.rfind('crate::dispatch') > before.rfind('}'):
                # 已经在一个未闭合的 dispatch 里（最近的 `}` 在 dispatch 之前）
                skipped['already_dispatch'] += 1
                out.append(src[i:start + 1])
                i = start + 1
                continue

            # 链后紧跟的 `?`（错误上抛）要留在 dispatch 外
            after = src[end:]
            qmark = ''
            mq = re.match(r'\s*\?', after)
            if mq:
                qmark = '?'
                end2 = end + mq.end()
            else:
                end2 = end

            body = chain.rstrip()
            if body.endswith('.await'):
                body = body[:-len('.await')].rstrip()
            # SQL 字面量 → &sql
            has_dialect = 'dialect_sql' in body
            lit = sql_literal(chain)
            _stmt_probe = src[:start]
            _j = _stmt_probe.rfind(';{}')
            if '=>' in _stmt_probe[_j + 1:start]:
                # 处在 match arm 内部 —— 不能在 arm 前面插 `let sql`
                lit = None
            sql_stmt = None
            if lit and not has_dialect:
                body = body[:lit[0]] + '&sql' + body[lit[1]:]
                sql_stmt = ('let sql = crate::db::dialect::current_sql(%s).into_owned();\n'
                            % lit[2])
            body = POOL_RE.sub('p', body)
            # 该函数里 pool 的实际引用（handlers 层通常是 state.pool）
            pref = pool_ref_text(src, start)
            is_exec = '.execute(' in body
            macro = 'crate::dispatch_exec!' if is_exec else 'crate::dispatch!'

            # 语句起点：向前扫到最近的 `;` / `{` / `}` 之后
            j = start - 1
            while j >= 0 and src[j] not in ';{}':
                j -= 1
            stmt_start = j + 1
            out.append(src[i:stmt_start])
            if sql_stmt:
                out.append(sql_stmt)
            out.append(src[stmt_start:start])
            out.append('%s(pool, |p| {\n' % macro)
            out.append(body)
            out.append('\n})')
            if qmark:
                out.append('?')
            i = end2
            total += 1
            changed = True

        if changed and apply:
            path.write_text(''.join(out))

    print("处理了 %d 处查询链" % total)
    if skipped:
        print("跳过:", dict(skipped))
    return 0


if __name__ == '__main__':
    sys.exit(main())

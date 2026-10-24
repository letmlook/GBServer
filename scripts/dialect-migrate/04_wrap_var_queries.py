#!/usr/bin/env python3
"""把「变量形式」的查询包进 dispatch。

形态：
    let mut q = sqlx::query(&sql);
    if let Some(v) = p { q = q.bind(v); }
    ...
    q.fetch_all(pool).await

范围由**同一个变量名**限定：从 `let [mut] var = sqlx::query...` 行，
到第一次出现 `var.<method>(pool)` 的行 —— 结构上不会吞掉旁边的代码。
"""
import re
import sys
import pathlib

LET_Q_RE = re.compile(r'^(\s*)let\s+(mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(sqlx::query)')
USE_RE_TMPL = r'\b%s\s*\.\s*\w+\s*\(\s*pool\s*\)'
POOL_RE = re.compile(r'(?<![.\w])pool\b(?!\s*\.)')


def find_parens(src, i):
    """src[i] == '(' → 返回配对 ')' 之后的位置。"""
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
            q = c
            i += 1
            while i < len(src):
                if src[i] == '\\':
                    i += 2
                    continue
                if src[i] == q:
                    break
                i += 1
        i += 1
    return -1



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
    total = 0
    for path in sorted(pathlib.Path('src').rglob('*.rs')):
        src = path.read_text()
        lines = src.split('\n')
        out = []
        i = 0
        changed = False
        while i < len(lines):
            m = LET_Q_RE.match(lines[i])
            if not m:
                out.append(lines[i])
                i += 1
                continue
            indent, mut, var = m.group(1), m.group(2), m.group(3)
            # 找使用点 var.<method>(pool) —— **必须限制在同一函数内**，
            # 否则会跨到下一个函数，把中间整段代码吞进 dispatch。
            use_re = re.compile(USE_RE_TMPL % re.escape(var))
            fn_head = None
            for k in range(i, -1, -1):
                if re.match(r'\s*(pub\s+)?(async\s+)?fn\s', lines[k]):
                    fn_head = k
                    break
            fn_end = len(lines)
            if fn_head is not None:
                # 先跳过**函数签名**（多行签名的收尾行 `) -> ... {` 缩进与函数头
                # 相同，不能当成函数结束）
                body_from = fn_head + 1
                while body_from < len(lines) and not lines[body_from].rstrip().endswith('{'):
                    body_from += 1
                head_indent = len(lines[fn_head]) - len(lines[fn_head].lstrip())
                for k in range(body_from + 1, len(lines)):
                    ln = lines[k]
                    if ln.strip() == '':
                        continue
                    if len(ln) - len(ln.lstrip()) <= head_indent:
                        fn_end = k
                        break
            end = None
            for j in range(i + 1, min(i + 60, fn_end)):
                if use_re.search(lines[j]):
                    end = j
                    break
            if end is None:
                out.append(lines[i])
                i += 1
                continue
            # 已在 dispatch 内？
            before = '\n'.join(lines[:i])
            if before.rfind('crate::dispatch') > before.rfind('}'):
                out.append(lines[i])
                i += 1
                continue
            block = lines[i:end + 1]
            # 构造调用是否跨行？跨行则取到分号
            if not lines[i].rstrip().endswith(';'):
                k = i
                while k < end and not lines[k].rstrip().endswith(';'):
                    k += 1
                end = k
                block = lines[i:end + 1]
            body = '\n'.join(block)
            body = POOL_RE.sub('p', body)
            pref = pool_ref(lines, fn_head) if fn_head is not None else 'pool'
            # 末尾的 `?`（错误上抛）必须留在 dispatch **外面**；
            # `.await` 由宏负责。
            qmark = ''
            stripped = body.rstrip()
            if stripped.endswith('?'):
                body = stripped[:-1].rstrip()
                qmark = '?'
            if body.endswith('.await'):
                body = body[:-len('.await')].rstrip()
            # 顶层 `let X = <expr>;` 要**提到宏外**（它的值是宏的返回值）
            tail_let = None
            blines = body.split('\n')
            while blines and not blines[-1].strip():
                blines.pop()
            if blines:
                m2 = re.match(r'^(\s*)let\s+([A-Za-z_][A-Za-z0-9_]*)(\s*:\s*[^=]+)?\s*=\s*(.*)$',
                              blines[-1])
                if m2:
                    tail_let = (m2.group(1), m2.group(2), m2.group(3) or '', m2.group(4))
                    blines = blines[:-1]
                    body = '\n'.join(blines)
            is_exec = '.execute(' in body
            macro = 'crate::dispatch_exec!' if is_exec else 'crate::dispatch!'
            if tail_let:
                _ind, _name, _ty, _expr = tail_let
                out.append('%slet %s%s = %s(pool, |p| {' % (indent, _name, _ty, macro))
                for ln in body.split('\n'):
                    out.append('    ' + ln if ln.strip() else ln)
                out.append('%s    %s' % (indent, _expr.strip()))
                out.append('%s})%s;' % (indent, qmark))
            else:
                out.append('%s%s(%s, |p| {' % (indent, macro, pref))
                for ln in body.split('\n'):
                    out.append('    ' + ln if ln.strip() else ln)
                out.append('%s})%s' % (indent, qmark))
            i = end + 1
            total += 1
            changed = True
        if changed and apply:
            path.write_text('\n'.join(out))
    print("处理了 %d 处变量形式查询" % total)
    return 0


if __name__ == '__main__':
    sys.exit(main())

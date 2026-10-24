#!/usr/bin/env python3
"""把 dispatch 宏体末尾的「语句分号」去掉，让块值成为表达式。

`dispatch!` 宏体做 `($q).await` —— 宏体的值必须是 future。宏体若是

    { ...; sqlx::query(&sql).execute(p); }

末尾的分号让**块的值变成 `()`** → 宏去 await 一个 `()`。

改法：末尾若是**非 let 的表达式语句**，去掉分号（块值 = 该表达式的值）。
末尾若是 `let x = ...;`（let 语句必须有分号），保持不动 —— 那种情况
应由「把 let 提到块外」处理（lift_single.py）。
"""
import re
import sys
import pathlib

CALL_RE = re.compile(r'crate::dispatch(?:_exec)?!\s*\(')


def find_end(t, i):
    d = 0
    while i < len(t):
        c = t[i]
        if c in '"\'':
            q = c
            i += 1
            while i < len(t):
                if t[i] == '\\':
                    i += 2
                    continue
                if t[i] == q:
                    break
                i += 1
        elif c == '(':
            d += 1
        elif c == ')':
            d -= 1
            if d == 0:
                return i + 1
        i += 1
    return -1


def main():
    apply = '--apply' in sys.argv
    total = 0
    for path in sorted(pathlib.Path('src').rglob('*.rs')):
        t = path.read_text()
        out = []
        pos = 0
        changed = 0
        while True:
            m = CALL_RE.search(t, pos)
            if not m:
                out.append(t[pos:])
                break
            op = t.index('(', m.start())
            end = find_end(t, op)
            if end == -1:
                out.append(t[pos:m.end()])
                pos = m.end()
                continue
            inner = t[op + 1:end - 1]
            mm = re.match(r'^(.*?),\s*\|(\w+)\|\s*\{(.*)\}$', inner, re.S)
            if not mm:
                out.append(t[pos:m.end()])
                pos = m.end()
                continue
            pool_expr, pvar, body = mm.group(1).strip(), mm.group(2), mm.group(3)
            b = body.rstrip()
            if not b.endswith(';'):
                out.append(t[pos:m.end()])
                pos = m.end()
                continue
            trimmed = b[:-1].rstrip()
            # 末尾是 let 语句 → 不能去分号
            if re.match(r'^\s*let\s', trimmed.split('\n')[-1]):
                out.append(t[pos:m.end()])
                pos = m.end()
                continue
            # 末尾是 `}`（如 if 块）→ 不动
            if trimmed.endswith('}') or trimmed.endswith(')'):
                out.append(t[pos:m.end()])
                pos = m.end()
                continue
            new_inner = '%s, |%s| { %s }' % (pool_expr, pvar, trimmed)
            out.append(t[pos:m.start()])
            out.append(t[m.start():op + 1] + new_inner + t[end - 1:end])
            pos = end
            changed += 1
            total += 1
        if changed:
            ns = ''.join(out)
            if apply:
                path.write_text(ns)
    print('去掉 %d 处宏体末尾分号' % total)
    return 0


if __name__ == '__main__':
    sys.exit(main())

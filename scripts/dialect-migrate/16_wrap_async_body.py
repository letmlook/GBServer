#!/usr/bin/env python3
"""把 dispatch 调用的 body 统一成 `{ async { ... } }` 形态。

`dispatch!` / `dispatch_exec!` 的宏体做 `($q).await` —— 要求 body 的值是
**future**。而 body 若是多语句块（`{ let x = ..; x.fetch_all(p) }`），
块的值是**最后一个表达式**（`Future<..>` ✅ 没问题）；可若是
`{ let x = ..; }` 这种以语句结尾的，块值就是 `()` → 宏去 await 一个 `()`
→ "`()` is not a future"。

统一包一层 `async` 后，body 的值恒为 future：
    |p| { async { <body> } }
宏 `($q).await` 得到 `Result<..>`，三个方言分支类型一致 ✅
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
            if re.match(r'^\s*async\s*\{', body):
                out.append(t[pos:m.end()])      # 已是目标形态
                pos = m.end()
                continue
            new_inner = '%s, |%s| { async { %s } }' % (pool_expr, pvar, body)
            out.append(t[pos:m.start()])
            out.append(t[m.start():op + 1] + new_inner + t[end - 1:end])
            pos = end
            changed += 1
        if changed:
            ns = ''.join(out)
            if apply:
                path.write_text(ns)
            total += changed
            print('  %s: %d 处' % (path, changed))
    print('共包裹 %d 处' % total)
    return 0


if __name__ == '__main__':
    sys.exit(main())

#!/usr/bin/env python3
"""调整 dispatch body 的 async 包裹。

`dispatch!` / `dispatch_exec!` 的宏体做 `($q).await` —— 要求 **body 的值是
future**：

    |p| { sqlx::query(&sql).execute(p) }        ✅ 值是 future
    |p| { let q = ..; q.fetch_all(p) }            ✅ 块的值 = 最后一个表达式 = future
    |p| { let q = ..; }                          ❌ 块的值 = () → 需要包 async

所以：**只有以语句结尾的块**才需要包成 `{ async { .. } }`；
已经包过的要**解开**（否则会多 await 一层，把 future 变成结果值）。
"""
import re
import sys
import pathlib

CALL_RE = re.compile(r'crate::dispatch(?:_exec)?!\s*\(')
ASYNC_BLOCK_RE = re.compile(r'^\s*async\s*\{(.*)\}\s*$', re.S)


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
    unwrapped = 0
    wrapped = 0
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
            am = ASYNC_BLOCK_RE.match(body)
            if am:
                # 已包 async → 解开（除非解开后仍以语句结尾）
                inner_body = am.group(1)
                if inner_body.rstrip().endswith(';'):
                    new_body = body          # 仍需 async 包裹
                    act = 'keep'
                else:
                    new_body = inner_body
                    act = 'unwrap'
            else:
                inner_body = body
                if inner_body.rstrip().endswith(';'):
                    new_body = 'async { %s }' % inner_body
                    act = 'wrap'
                else:
                    new_body = body
                    act = 'same'
            if act == 'keep':
                out.append(t[pos:m.end()])
                pos = m.end()
                continue
            new_inner = '%s, |%s| { %s }' % (pool_expr, pvar, new_body)
            out.append(t[pos:m.start()])
            out.append(t[m.start():op + 1] + new_inner + t[end - 1:end])
            pos = end
            changed += 1
            if act == 'unwrap':
                unwrapped += 1
            elif act == 'wrap':
                wrapped += 1
        if changed:
            ns = ''.join(out)
            if apply:
                path.write_text(ns)
    print('解开 %d 处 async 包裹，新增 %d 处' % (unwrapped, wrapped))
    return 0


if __name__ == '__main__':
    sys.exit(main())

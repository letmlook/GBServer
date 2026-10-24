#!/usr/bin/env python3
"""把 dispatch 宏体末尾的「语句」改成「表达式」，并把 `?` 提到宏外。

`dispatch!` / `dispatch_exec!` 的宏体做 `($q).await` —— 宏体必须是**一个
表达式**。若宏体末尾是语句（`...;`），块的值就是 `()`：

    crate::dispatch_exec!(pool, |p| {
        let mut q = sqlx::query(&sql);
        for b in binds { q = q.bind(b.as_str()); }
        q.execute(p).await?;        ← 以语句结尾 → 块值 = ()
    })

改法：末尾的 `.await?` 去掉（`?` 提到宏外），末尾的 `;` 去掉：

    let r = crate::dispatch_exec!(pool, |p| {
        let mut q = sqlx::query(&sql);
        for b in binds { q = q.bind(b.as_str()); }
        q.execute(p)
    })?;

末尾是 `X.await;`（无 ?）时同样处理，外层补 `?`（原本的 `.await` 隐含了
错误传播）。
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
            had_q = False
            if b.endswith(';'):
                b = b[:-1].rstrip()
            if b.endswith('.await?'):
                b = b[:-len('.await?')].rstrip()
                had_q = True
            elif b.endswith('.await'):
                b = b[:-len('.await')].rstrip()
                had_q = True
            if not had_q:
                out.append(t[pos:m.end()])
                pos = m.end()
                continue
            # 外层补 `?`：找调用结束后的分号
            new_inner = '%s, |%s| { %s }' % (pool_expr, pvar, b)
            out.append(t[pos:m.start()])
            out.append(t[m.start():op + 1] + new_inner + t[end - 1:end])
            # 调用点之后：原本是 `);` → 改成 `)?;`
            rest = t[end:]
            m2 = re.match(r'([ \t]*)\)\s*;', rest)
            if m2:
                out.append(')?;')
                pos = end + m2.end()
            else:
                out.append(')')
                pos = end
            changed += 1
            total += 1
        if changed:
            ns = ''.join(out)
            if apply:
                path.write_text(ns)
    print('改写 %d 处宏体末尾' % total)
    return 0


if __name__ == '__main__':
    sys.exit(main())

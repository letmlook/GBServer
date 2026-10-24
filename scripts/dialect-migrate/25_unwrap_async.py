#!/usr/bin/env python3
"""把 `{ async { ... } }` 形态的 dispatch body 还原成「块值即 future」。

dispatch 宏体做 `($q).await` —— 要求 **body 的值是 future**。

被 async 包过一轮的 body 形如：

    |p| { async {
        let mut q = sqlx::query(&sql);
        for b in binds { q = q.bind(b.as_str()); }
        q.execute(p).await?;
    } }

两处问题：
1. 末尾的 `.await?` —— `?` 需要所在块返回 Result，但 async 块的值是
   `q.execute(p)`（future）→ 类型对不上；
2. async 包裹本身 —— 块的值已经是 future，再包一层反而多 await 一次。

正确形态：

    |p| {
        let mut q = sqlx::query(&sql);
        for b in binds { q = q.bind(b.as_str()); }
        q.execute(p)
    }

末尾的 `.await?` 去掉后，`?` 只可能出现在中间语句里 —— 那时它作用于
**外层 async fn**（返回 Result）✅ 语义与原来一致。
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
    unwrapped = 0
    trimmed = 0
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
            mm = re.match(r'^(.*?),\s*\|(\w+)\|\s*\{\s*async\s*\{(.*)\}\s*\}$', inner, re.S)
            if not mm:
                out.append(t[pos:m.end()])
                pos = m.end()
                continue
            pool_expr, pvar, body = mm.group(1).strip(), mm.group(2), mm.group(3)
            # 末尾的 `.await?` 去掉（保留分号与否都处理）
            b = body.rstrip()
            if b.endswith(';'):
                b = b[:-1].rstrip()
            if b.endswith('.await?'):
                b = b[:-len('.await?')].rstrip()
                trimmed += 1
            new_inner = '%s, |%s| { %s }' % (pool_expr, pvar, b)
            out.append(t[pos:m.start()])
            out.append(t[m.start():op + 1] + new_inner + t[end - 1:end])
            pos = end
            changed += 1
            unwrapped += 1
        if changed:
            ns = ''.join(out)
            if apply:
                path.write_text(ns)
    print('解开 %d 处 async 包裹，其中去掉末尾 .await? %d 处' % (unwrapped, trimmed))
    return 0


if __name__ == '__main__':
    sys.exit(main())

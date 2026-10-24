#!/usr/bin/env python3
"""把 dispatch 宏的 body 参数从 `$q:block` 改成 `$q:expr`。

`$q:block` 匹配不了 `async { ... }`（async 块不是 block 表达式形态），
`$q:expr` 才能 —— 宏体里 `($q).await` 对 async 块同样成立。
"""
import re
import pathlib

n = 0
for p in sorted(pathlib.Path('src').rglob('*.rs')):
    s = p.read_text()
    o = s
    s = s.replace("($pool:expr, |$p:ident| $q:block) => {",
                  "($pool:expr, |$p:ident| $q:expr) => {")
    if s != o:
        p.write_text(s)
        n += 1
        print('  %s' % p)
print('改了 %d 个文件' % n)

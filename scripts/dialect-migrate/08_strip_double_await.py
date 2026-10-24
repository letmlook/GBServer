#!/usr/bin/env python3
"""让 dispatch 宏的 body 统一「不含 `.await`」。

`dispatch!` / `dispatch_exec!` 的宏体做 `($q).await` —— 宏负责 await。
若调用点的 body 里**已经**有一处 `.await`，就会变成
"`Result<u64, _>` is not a future"（await 一个已完成的 Result）。

本脚本扫出所有调用点，把 body 末尾的 `.await`（或 `.await?`）去掉，
`?` 保留（错误上抛必须在宏外）。
"""
import re
import sys
import pathlib

CALL_RE = re.compile(r'crate::dispatch(?:_exec)?!\s*\(')


def find_call_end(text, open_paren):
    """open_paren 指向 '('，返回配对 ')' 之后的位置。"""
    depth = 0
    i = open_paren
    n = len(text)
    while i < n:
        c = text[i]
        if c in '"\'':
            q = c
            i += 1
            while i < n:
                if text[i] == '\\':
                    i += 2
                    continue
                if text[i] == q:
                    break
                i += 1
        elif c == '(':
            depth += 1
        elif c == ')':
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return -1


def strip_trailing_await(text):
    """去掉 body 末尾的 `.await`（保留 `?`）。返回 (新文本, 是否改动)。"""
    # 找最后一个 `.await` 在 body 内的位置
    idx = text.rfind('.await')
    if idx == -1:
        return text, False
    # 后面只允许空白 / `?` / `;` / `)` 等
    tail = text[idx + len('.await'):]
    if re.match(r'^\s*\??\s*;?\s*$', tail):
        return text[:idx] + tail.replace('?', '', 1) if '?' in tail else text[:idx] + tail, True
    return text, False


def main():
    apply = '--apply' in sys.argv
    total = 0
    for path in sorted(pathlib.Path('src').rglob('*.rs')):
        s = path.read_text()
        out = []
        pos = 0
        changed = 0
        while True:
            m = CALL_RE.search(s, pos)
            if not m:
                out.append(s[pos:])
                break
            open_paren = s.index('(', m.start())
            end = find_call_end(s, open_paren)
            if end == -1:
                out.append(s[pos:m.end()])
                pos = m.end()
                continue
            call = s[m.start():end]
            new_call, did = strip_trailing_await(call)
            out.append(s[pos:m.start()])
            out.append(new_call)
            pos = end
            if did:
                changed += 1
            # 调用**之后**的 `.await` 也要去掉：宏体已经 await 过，
            # 返回值是 `Result<...>`，再 await 就是 "not a future"。
            m2 = re.match(r'\s*\.await\b', s[pos:])
            if m2:
                out.append('')
                pos += m2.end()
                changed += 1
        if changed:
            ns = ''.join(out)
            if apply:
                path.write_text(ns)
            total += changed
            print('  %s: %d 处' % (path, changed))
    print('共处理 %d 处' % total)
    return 0


if __name__ == '__main__':
    sys.exit(main())

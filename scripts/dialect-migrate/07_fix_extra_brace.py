#!/usr/bin/env python3
"""修掉「多出来的一层花括号」。

第一轮的 block 转换把 `#[cfg(feature = X)] { let sql = ...; ... }` 里的内容
搬成了「`let sql = ...;` + dispatch」，但**原块的开花括号留在了原地**；
`cfg_to_runtime` 再把 cfg 变成 `if ... {` 时，就多出一层：

    if crate::db::dialect::current().is_mysql() {
        {let sql = pool.sql("...").into_owned();
    }

        crate::dispatch_exec!(pool, |p| { ... })?;
    }

正确形态是把那对多余的花括号去掉（`if` 已经提供了块）。
"""
import re
import sys
import pathlib


def find_pair_end(lines, start):
    """start 行以 '{' 开头且行内还有内容 → 找配对的 '}' 行。"""
    depth = 0
    for i in range(start, len(lines)):
        s = lines[i]
        in_str = None
        j = 0
        while j < len(s):
            c = s[j]
            if in_str:
                if c == '\\':
                    j += 2
                    continue
                if c == in_str:
                    in_str = None
                j += 1
                continue
            if c in '"\'':
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
    return -1


def main():
    apply = '--apply' in sys.argv
    total = 0
    for path in sorted(pathlib.Path('src').rglob('*.rs')):
        lines = path.read_text().split('\n')
        out = []
        i = 0
        changed = False
        while i < len(lines):
            # 形如 `        {let sql = ...` —— 行首 `{` 紧跟 `let`/`let mut`
            m = re.match(r'^(\s*)\{let\s', lines[i])
            if not m:
                out.append(lines[i])
                i += 1
                continue
            end = find_pair_end(lines, i)
            if end == -1:
                out.append(lines[i])
                i += 1
                continue
            indent = m.group(1)
            # 第一行：去掉行首的 '{'
            out.append(indent + lines[i].lstrip()[1:])
            # 中间内容
            for k in range(i + 1, end):
                out.append(lines[k])
            # 配对的 '}'：整行只有 '}' → 丢弃
            if lines[end].strip() == '}':
                pass
            else:
                out.append(lines[end])
            i = end + 1
            changed = True
            total += 1
        if changed and apply:
            path.write_text('\n'.join(out))
    print("修掉 %d 处多余花括号" % total)
    return 0


if __name__ == '__main__':
    sys.exit(main())

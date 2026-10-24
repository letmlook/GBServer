#!/usr/bin/env python3
"""把连续的「运行期方言 if 块」合并成 if / else if / else 链。

改造后代码里常见这种形态（每个方言一个独立 if）：

    if crate::db::dialect::current().is_mysql() {
        return crate::dispatch!(pool, |p| { ... });
    }
    if crate::db::dialect::current().is_postgres() {
        return crate::dispatch!(pool, |p| { ... });
    }

问题有两个：
1. 三个独立 `if` 各自是一个**语句**，函数尾部那个 if 的值类型必须和别的匹配；
   而块里有 `return` → 块的值类型是 `never` → "if may be missing an else clause"。
2. 三个方言的代码块重复。

合并成 if-else 链后，块里的 `return` 要去掉（链本身就成了函数尾表达式）。
"""
import re
import sys
import pathlib

DIALECT_IF = re.compile(
    r'^(\s*)if (!?)crate::db::dialect::current\(\)\.is_(mysql|postgres|sqlite)\(\) \{$')


def find_block_end(lines, start):
    depth = 0
    for i in range(start, len(lines)):
        line = lines[i]
        in_str = None
        j = 0
        while j < len(line):
            c = line[j]
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
    merged = 0
    for path in sorted(pathlib.Path('src').rglob('*.rs')):
        lines = path.read_text().split('\n')
        out = []
        i = 0
        changed = 0
        while i < len(lines):
            m = DIALECT_IF.match(lines[i])
            if not m:
                out.append(lines[i])
                i += 1
                continue
            indent = m.group(1)
            # 收集后续同缩进的方言 if 块
            blocks = []
            j = i
            while True:
                mm = DIALECT_IF.match(lines[j]) if j < len(lines) else None
                if not mm or mm.group(1) != indent:
                    break
                end = find_block_end(lines, j)
                if end == -1:
                    break
                blocks.append((mm.group(2), mm.group(3) == '!', j, end))
                j = end + 1
            if len(blocks) < 2:
                out.append(lines[i])
                i += 1
                continue
            # 构造成 if / else if / else
            for k, (neg, dia, s, e) in enumerate(blocks):
                if k == 0:
                    head = '%sif %scrate::db::dialect::current().is_%s() {' % (
                        indent, '!' if neg else '', dia)
                elif k < len(blocks) - 1:
                    head = '%s} else if %scrate::db::dialect::current().is_%s() {' % (
                        indent, '!' if blocks[k][0] else '', blocks[k][1])
                else:
                    head = '%s} else {' % indent
                out.append(head)
                for ln in lines[s + 1:e]:
                    # 去掉块内开头的 `return `（合并成链后由链本身返回值）
                    out.append(re.sub(r'^(\s*)return\s+', r'\1', ln))
            out.append('%s}' % indent)
            changed += 1
            merged += 1
            i = j
        if changed:
            ns = '\n'.join(out)
            if apply:
                path.write_text(ns)
    print('合并 %d 处方言 if 链' % merged)
    return 0


if __name__ == '__main__':
    sys.exit(main())

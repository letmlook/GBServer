#!/usr/bin/env python3
"""删掉函数末尾**孤立的分号行**。

拆分 body 时把 `Ok(...)` 之类「尾表达式」提到了宏外，却留下了一个
单独的分号行：

    Ok(result.rows_affected())
    ;            ← 尾表达式被当成语句 → 函数返回 ()

编译器提示正是 "remove this semicolon to return this value"。
"""
import re
import sys
import pathlib

SEMI_ONLY = re.compile(r'^[ \t]*;[ \t]*$')


def main():
    apply = '--apply' in sys.argv
    total = 0
    for path in sorted(pathlib.Path('src').rglob('*.rs')):
        lines = path.read_text().split('\n')
        out = []
        removed = 0
        for i, ln in enumerate(lines):
            if (SEMI_ONLY.match(ln) and out
                    and out[-1].rstrip().endswith(')')
                    and not out[-1].rstrip().endswith(');')):
                # 前一行是尾表达式 → 去掉这个孤立分号
                out[-1] = out[-1].rstrip()
                removed += 1
                continue
            out.append(ln)
        if removed:
            p = path.write_text('\n'.join(out))
            total += removed
            print('  %s: %d 处' % (path, removed))
    print('共删掉 %d 处孤立分号' % total)
    return 0


if __name__ == '__main__':
    sys.exit(main())

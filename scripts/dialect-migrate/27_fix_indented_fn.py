#!/usr/bin/env python3
"""恢复被**错误缩进**的顶层函数。

转换过程中，某些顶层函数（`pub async fn update(...)`）被卷进了前一个
方言块的 `{ ... }` 里 → 缩进变成 4 空格 → 它不再是模块级函数，
调用方 `platform_db::update(...)` 就找不到 ❌。

修法：从基线取出该函数的**原始文本**，放回顶层（缩进 0）。
"""
import re
import subprocess
import sys
import pathlib

BASE = '43b178b'
TOP_FN = re.compile(r'^(?:pub(?:\([a-z]+\))?\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)')
INDENTED_FN = re.compile(r'^(\s+)(pub(?:\([a-z]+\))?\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)')


def base_funcs(rel):
    try:
        orig = subprocess.run(['git', 'show', BASE + ':' + rel],
                              capture_output=True, text=True, check=True).stdout
    except subprocess.CalledProcessError:
        return {}
    lines = orig.split('\n')
    out = {}
    i = 0
    while i < len(lines):
        m = TOP_FN.match(lines[i])
        if m:
            start = i
            depth = 0
            j = i
            while j < len(lines):
                depth += lines[j].count('{') - lines[j].count('}')
                if depth == 0 and j > i and lines[j].startswith('}'):
                    break
                j += 1
            out[m.group(1)] = '\n'.join(lines[start:j + 1])
            i = j + 1
            continue
        i += 1
    return out


def main():
    apply = '--apply' in sys.argv
    total = 0
    for path in sorted(pathlib.Path('src').rglob('*.rs')):
        rel = str(path)
        bfuncs = base_funcs(rel)
        if not bfuncs:
            continue
        lines = path.read_text().split('\n')
        out = []
        i = 0
        changed = 0
        while i < len(lines):
            m = INDENTED_FN.match(lines[i])
            if m and m.group(2) and m.group(3) in bfuncs:
                name = m.group(3)
                # 找到这个函数的结束行
                depth = 0
                j = i
                while j < len(lines):
                    depth += lines[j].count('{') - lines[j].count('}')
                    if depth == 0 and j > i and lines[j].strip() == '}':
                        break
                    j += 1
                # 只**去缩进**，保留当前（已转换）的函数内容 ——
                # 用基线版本替换会把 `#[cfg(feature = ...)]` 分支带回来。
                for ln in lines[i:j + 1]:
                    out.append(ln[1:] if ln.startswith(' ') else ln)
                changed += 1
                i = j + 1
                continue
            out.append(lines[i])
            i += 1
        if changed:
            if apply:
                path.write_text('\n'.join(out))
            print('  %s: 恢复 %d 处' % (rel, changed))
            total += changed
    print('共恢复 %d 处被缩进的顶层函数' % total)
    return 0


if __name__ == '__main__':
    sys.exit(main())

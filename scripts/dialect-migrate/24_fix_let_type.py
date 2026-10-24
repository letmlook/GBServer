#!/usr/bin/env python3
"""从改造前的基线恢复 `let` 的类型标注。

三方言合一时，`let parent_db_id: i32 = ...` 的**类型标注**有时会丢
（变成 `let parent_db_id = ...`）。于是 `sqlx::query_scalar("...")` 的输出
类型失去推断依据 → 报 "the trait bound `(): sqlx::Type<MySql>` is not
satisfied"。

改法：从基线（改造前）建 `变量名 → 类型` 的映射，把当前代码里缺失的
标注补回去。
"""
import re
import subprocess
import sys
import pathlib

BASE = '43b178b'
LET_ANN = re.compile(r'\blet\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*:\s*'
                      r'([A-Za-z_][A-Za-z0-9_]*(?:\s*<[^;=]*>)?)\s*=')
LET_PLAIN = re.compile(r'^(\s*)let\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*$')


def base_types(path):
    try:
        orig = subprocess.run(['git', 'show', BASE + ':' + path],
                              capture_output=True, text=True, check=True).stdout
    except subprocess.CalledProcessError:
        return {}
    out = {}
    for m in LET_ANN.finditer(orig):
        out.setdefault(m.group(1), m.group(2).strip())
    return out


def main():
    apply = '--apply' in sys.argv
    total = 0
    for path in sorted(pathlib.Path('src').rglob('*.rs')):
        rel = str(path)
        types = base_types(rel)
        if not types:
            continue
        lines = path.read_text().split('\n')
        out = []
        changed = 0
        for ln in lines:
            m = LET_PLAIN.match(ln)
            if m:
                name = m.group(2)
                if name in types:
                    out.append('%slet %s: %s =' % (m.group(1), name, types[name]))
                    changed += 1
                    total += 1
                    continue
            out.append(ln)
        if changed:
            if apply:
                path.write_text('\n'.join(out))
            print('  %s: %d 处' % (rel, changed))
    print('共补回 %d 处类型标注' % total)
    return 0


if __name__ == '__main__':
    sys.exit(main())

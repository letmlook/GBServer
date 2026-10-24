#!/usr/bin/env python3
"""补回 `sqlx::query_scalar` 的类型参数。

`sqlx::query_scalar("...")` 不带泛型时，输出类型要靠上下文推断；
放进 `dispatch!` 宏后，三个方言分支各自独立推断 → 推断不出 → 报
"the trait bound `(): sqlx::Decode<'_, MySql>` is not satisfied"。

改法：从赋值目标的类型标注取出类型，补成 `sqlx::query_scalar::<_, T>(...)`。
"""
import re
import sys
import pathlib

LET_ANN_RE = re.compile(
    r'(?P<indent>[ \t]*)let\s+(?:mut\s+)?(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*:\s*'
    r'(?P<ty>[A-Za-z_][A-Za-z0-9_:<>,\s]*?)\s*=\s*$')
QS_RE = re.compile(r'sqlx::query_scalar\s*(?P<gen>::\s*<[^>]*>)?\s*\(')


def main():
    apply = '--apply' in sys.argv
    total = 0
    for path in sorted(pathlib.Path('src').rglob('*.rs')):
        lines = path.read_text().split('\n')
        out = []
        changed = 0
        i = 0
        while i < len(lines):
            m = LET_ANN_RE.match(lines[i])
            if not m:
                out.append(lines[i])
                i += 1
                continue
            ty = m.group('ty').strip()
            # 往后找（跳过空行）第一个含 sqlx::query_scalar 的行
            j = i + 1
            while j < len(lines) and lines[j].strip() == '':
                j += 1
            if j >= len(lines):
                out.append(lines[i])
                i += 1
                continue
            qm = QS_RE.search(lines[j])
            if not qm or qm.group('gen'):
                out.append(lines[i])
                i += 1
                continue
            # 只支持简单类型名（含 Vec/Option 之类太复杂 → 跳过）
            if not re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*(\s*<[^;=]*>)?', ty):
                out.append(lines[i])
                i += 1
                continue
            new_line = lines[j][:qm.start()] + 'sqlx::query_scalar::<_, %s>(' % ty + lines[j][qm.end():]
            out.append(lines[i])
            out.append(new_line)
            i = j + 1
            changed += 1
            total += 1
        if changed:
            ns = '\n'.join(out)
            if apply:
                path.write_text(ns)
    print('补全 %d 处 query_scalar 泛型' % total)
    return 0


if __name__ == '__main__':
    sys.exit(main())

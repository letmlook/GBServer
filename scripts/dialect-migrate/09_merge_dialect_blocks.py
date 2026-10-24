#!/usr/bin/env python3
"""把「运行期方言分支块」收敛掉。

第一轮把三份 SQL 统一成了 `?` + `current_sql()`，于是**三个方言块的内容
本应完全相同** —— 那就没必要按方言分支，直接留一份即可。

本脚本做两件事：

1. **统一 SQL 文本**：把方言块里字符串字面量中的 `$1..$n` 改回 `?`，
   并把 `pool.sql(...)` 换成 `crate::db::dialect::current_sql(...)`
   （后者不依赖调用方手里的 pool 变量名）。
2. **合并方言块**：
   - 三个块内容相同 → 只留一份（去掉 if 条件），所有方言都执行；
   - 内容不同（如 `RETURNING id`、CAST 差异）→ 合成 `if / else if / else` 链，
     变量提升到链外（`let V = if ... {} else if ... {} else {};`）。
"""
import re
import sys
import pathlib

DIALECT_IF = re.compile(
    r'^(\s*)if (!?)crate::db::dialect::current\(\)\.is_(mysql|postgres|sqlite)\(\) \{$')
PLACEHOLDER = re.compile(r'\$\d+')
STR_RE = re.compile(r'"(?:[^"\\]|\\.)*"')


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


def unify_sql(text):
    """把字符串字面量里的 `$n` 改回 `?`，并把 `pool.sql(` 换成 `current_sql(`。"""
    def fix_lit(m):
        return m.group(0).replace('$', '?') if PLACEHOLDER.search(m.group(0)) else m.group(0)
    text = STR_RE.sub(fix_lit, text)
    text = re.sub(r'\bpool\.sql\(', 'crate::db::dialect::current_sql(', text)
    return text


def norm(text):
    return re.sub(r'\s+', ' ', text).strip()


def main():
    apply = '--apply' in sys.argv
    total_merged = 0
    total_chain = 0
    for path in sorted(pathlib.Path('src').rglob('*.rs')):
        lines = path.read_text().split('\n')
        out = []
        i = 0
        changed = False
        while i < len(lines):
            m = DIALECT_IF.match(lines[i])
            if not m:
                out.append(lines[i])
                i += 1
                continue
            # 收集后续同缩进的方言块
            indent = m.group(1)
            groups = []
            j = i
            while True:
                mm = DIALECT_IF.match(lines[j]) if j < len(lines) else None
                if not mm or mm.group(1) != indent:
                    break
                end = find_block_end(lines, j)
                if end == -1:
                    break
                groups.append((mm.group(2), mm.group(3) == '!', j, end))
                j = end + 1
            if len(groups) < 2:
                out.append(lines[i])
                i += 1
                continue

            bodies = []
            for dia, neg, s, e in groups:
                body = lines[s + 1:e]
                bodies.append((dia, neg, [unify_sql(b) for b in body]))

            # 规范化后比较（忽略缩进差异）
            keys = [norm('\n'.join(b[2])) for b in bodies]
            if len(set(keys)) == 1:
                # 内容完全相同 → 只留一份，去掉 if 条件
                out.extend(bodies[0][2])
                total_merged += 1
                changed = True
                i = j
                continue

            # 内容不同 → 合成 if / else if / else 链
            # 三个分支里若都只有 `let NAME = ...;` 一条语句 → 提升到链外
            lets = []
            ok_let = True
            for dia, neg, body in bodies:
                meaningful = [b for b in body if b.strip()]
                m2 = re.match(r'^\s*let\s+([A-Za-z_][A-Za-z0-9_]*)(\s*:\s*[^=]+?)?\s*=\s*(.*)$',
                              meaningful[0], re.S) if meaningful else None
                if len(meaningful) == 1 and m2:
                    lets.append((m2.group(1), m2.group(2), m2.group(3)))
                else:
                    ok_let = False
                    break
            if ok_let and len({l[0] for l in lets}) == 1:
                name, ty, _ = lets[0]
                cond = []
                for dia, neg, body in bodies:
                    c = 'crate::db::dialect::current().is_%s()' % dia
                    if neg:
                        c = '!' + c
                    cond.append(c)
                out.append('%slet %s%s = if %s {' % (indent, name, ty or '', cond[0]))
                out.extend(bodies[0][2])
                for k in range(1, len(bodies) - 1):
                    out.append('%s} else if %s {' % (indent, cond[k]))
                    out.extend(bodies[k][2])
                out.append('%s} else {' % indent)
                out.extend(bodies[-1][2])
                out.append('%s}' % indent)
                total_chain += 1
                changed = True
                i = j
                continue

            # 其余情况保留原样
            for dia, neg, s, e in groups:
                out.extend(lines[s:e + 1])
            i = j

        if changed and apply:
            path.write_text('\n'.join(out))
    print("合并同内容方言块: %d   合成 if-else 链: %d" % (total_merged, total_chain))
    return 0


if __name__ == '__main__':
    sys.exit(main())

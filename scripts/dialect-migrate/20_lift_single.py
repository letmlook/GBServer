#!/usr/bin/env python3
"""把「方言 if 块里独有的 let 变量」提到块外。

形态（转换后常见）：

    if crate::db::dialect::current().is_mysql() {
        let r = crate::dispatch_exec!(pool, |p| { ... })?;
    }
    Ok(r.rows_affected())          ← r 不在作用域

SQL 统一之后，各方言分支的代码其实是同一份，**方言条件本身是多余的**。
所以把块里那条 `let` 提到外层，条件去掉：

    let r = crate::dispatch_exec!(pool, |p| { ... })?;
    Ok(r.rows_affected())

只在「块内**只有**一条 let 语句」时做（多语句的块留给人工，避免作用域意外）。
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


def split_stmts(body):
    """按**顶层**分号切分语句（括号/花括号/字符串感知）。"""
    stmts = []
    depth_p = depth_b = 0
    start = 0
    i = 0
    n = len(body)
    while i < n:
        c = body[i]
        if c in '"\'':
            q = c
            i += 1
            while i < n:
                if body[i] == '\\':
                    i += 2
                    continue
                if body[i] == q:
                    break
                i += 1
        elif c == '(':
            depth_p += 1
        elif c == ')':
            depth_p -= 1
        elif c == '{':
            depth_b += 1
        elif c == '}':
            depth_b -= 1
        elif c == ';' and depth_p == 0 and depth_b == 0:
            stmts.append(body[start:i + 1])
            start = i + 1
        i += 1
    tail = body[start:]
    if tail.strip():
        stmts.append(tail)
    return stmts


def main():
    apply = '--apply' in sys.argv
    lifted = 0
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
            end = find_block_end(lines, i)
            if end == -1:
                out.append(lines[i])
                i += 1
                continue
            body = lines[i + 1:end]
            stmts = split_stmts('\n'.join(body))
            # 块内只有一条**语句**（可跨多行）→ 提升
            if len(stmts) != 1:
                out.extend(lines[i:end + 1])
                i = end + 1
                continue
            stmt = stmts[0].split('\n')
            # 该语句必须是 `let X = ...` 且以 `;` 结尾
            joined = '\n'.join(stmt)
            if not joined.strip().endswith(';') or not re.match(r'\s*let\s', stmt[0]):
                out.extend(lines[i:end + 1])
                i = end + 1
                continue
            # 提升：去掉 if 行与收尾 }，语句去一层缩进
            for ln in stmt:
                out.append(ln[4:] if ln.startswith(indent + '    ') else ln)
            changed += 1
            lifted += 1
            i = end + 1
        if changed:
            ns = '\n'.join(out)
            if apply:
                path.write_text(ns)
    print('提升 %d 处' % lifted)
    return 0


if __name__ == '__main__':
    sys.exit(main())

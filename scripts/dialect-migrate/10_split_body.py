#!/usr/bin/env python3
"""把多语句的 dispatch body 拆成「宏外语句 + 单表达式宏体」。

`dispatch!` / `dispatch_exec!` 的宏体做 `($q).await` —— body 只能是
**一个表达式**（不含 `.await`）。但有一批调用点的 body 是多语句：

    crate::dispatch!(pool, |p| {
        let query = sqlx::query_as::<_, Alarm>(&sql)
            .bind(..)
            ..;
        let result: (i64,) = query.fetch_all(p).await?;
        Ok(result.0)
    })

拆法：
* **构造语句**（`let query = sqlx::query_as(..)..;`）—— `query_as` / `query_scalar`
  的类型**不依赖**执行器，可以提到宏外；
* **执行语句**（`let result = query.fetch_all(p).await?;`）—— 拆成
  `let result = crate::dispatch!(pool, |p| { query.fetch_all(p) })?;`
  （`sqlx::query(&sql)` 的 Arguments 依赖执行器，**不能**提到宏外，
  所以执行语句里的 `sqlx::query(..)` 必须留在宏体里）；
* 其余语句原样保留在宏外。
"""
import re
import sys
import pathlib

CALL_RE = re.compile(r'crate::dispatch(!|_exec!)\s*\(')


def find_end(t, i):
    d = 0
    while i < len(t):
        c = t[i]
        if c in '"\'':
            q = c
            i += 1
            while i < len(t):
                if t[i] == '\\':
                    i += 2
                    continue
                if t[i] == q:
                    break
                i += 1
        elif c == '(':
            d += 1
        elif c == ')':
            d -= 1
            if d == 0:
                return i + 1
        i += 1
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
    total = 0
    for path in sorted(pathlib.Path('src').rglob('*.rs')):
        t = path.read_text()
        out = []
        pos = 0
        changed = 0
        while True:
            m = CALL_RE.search(t, pos)
            if not m:
                out.append(t[pos:])
                break
            op = t.index('(', m.start())
            end = find_end(t, op)
            if end == -1:
                out.append(t[pos:m.end()])
                pos = m.end()
                continue
            call = t[m.start():end]
            inner = call[call.index('(') + 1:-1]
            mm = re.match(r'^(.*?),\s*\|(\w+)\|\s*\{(.*)\}$', inner, re.S)
            if not mm:
                out.append(t[pos:m.end()])
                pos = m.end()
                continue
            pool_expr, pvar, body = mm.group(1).strip(), mm.group(2), mm.group(3)
            # body 可能是 `{ async { ... } }` —— 剥掉 async 那层
            am = re.match(r'^\s*async\s*\{(.*)\}\s*$', body, re.S)
            if am:
                body = am.group(1)
            stmts = split_stmts(body)
            # 找含 `.await?` 的执行语句
            exec_idx = None
            for k, s in enumerate(stmts):
                if '.await?' in s:
                    exec_idx = k
                    break
            if exec_idx is None:
                out.append(t[pos:m.end()])
                pos = m.end()
                continue
            ex = stmts[exec_idx].strip()
            # 形如 `let NAME[: T] = <链>.await?;` 或 `<链>.await?;`
            lm = re.match(r'^let\s+(mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*([^=]+?))?\s*=\s*(.*)$',
                          ex.rstrip(';').strip(), re.S)
            if lm and lm.group(2) == '_':
                # `let _ = ...` 丢弃结果 —— 不拆（拆了会产出 `return let _ = ...`）
                out.append(t[pos:m.end()])
                pos = m.end()
                continue
            if not lm:
                out.append(t[pos:m.end()])
                pos = m.end()
                continue
            name = lm.group(2)
            ty = lm.group(3)
            chain = lm.group(4).strip()
            if not chain.endswith('.await?'):
                out.append(t[pos:m.end()])
                pos = m.end()
                continue
            chain = chain[:-len('.await?')].strip()
            # 判断用哪个宏：后续语句里是否用 NAME.rows_affected()
            rest_txt = '\n'.join(stmts[exec_idx + 1:])
            macro = 'dispatch_exec' if (name + '.rows_affected()') in rest_txt else 'dispatch'
            # 构造语句（含 sqlx::query* 且在执行语句之前）留在宏外
            pre = [s for s in stmts[:exec_idx]]
            post = stmts[exec_idx + 1:]
            indent = ''
            for s in pre + post:
                mm3 = re.match(r'^(\s*)', s)
                if mm3 and mm3.group(1):
                    indent = mm3.group(1)
                    break
            new_parts = []
            for s in pre:
                new_parts.append(s)
            new_parts.append('%slet %s%s = crate::%s!(%s, |%s| {\n%s    %s\n%s})?;'
                             % (indent, name, (' ' + ty.strip()) if ty else '', macro,
                                pool_expr, pvar, indent, chain, indent))
            for s in post:
                new_parts.append(s)
            head = t[pos:m.start()]
            # 原调用点可能是 `return crate::dispatch!(...)` —— 拆分后
            # `return` 会悬空（后面跟的是 let 语句）。去掉它：
            # `?` 本身就会 return 外层函数，语义一致。
            if re.search(r'\breturn\s+$', head):
                head = re.sub(r'\breturn\s+$', '', head)
            out.append(head)
            out.append('\n'.join(new_parts))
            pos = end
            changed += 1
        if changed:
            ns = ''.join(out)
            if apply:
                path.write_text(ns)
            total += changed
            print('  %s: %d 处' % (path, changed))
    print('共拆分 %d 处' % total)
    return 0


if __name__ == '__main__':
    sys.exit(main())

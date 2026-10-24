#!/usr/bin/env python3
"""把「编译期 cfg 方言分支」改写成「运行期按 pool 方言分发」。

**不合并各分支的 SQL** —— 每种方言保留自己那份 SQL 文本（那本来就是对的），
只把「选哪一份」的时机从编译期搬到运行期：

```rust
// 改写前
#[cfg(any(feature = "mysql", feature = "sqlite"))]
return sqlx::query_as::<_, T>("... WHERE id = ?").bind(id).fetch_optional(pool).await;
#[cfg(feature = "postgres")]
return sqlx::query_as::<_, T>("... WHERE id = $1").bind(id).fetch_optional(pool).await;

// 改写后
crate::dispatch_dialect!(
    pool,
    |p| { sqlx::query_as::<_, T>("... WHERE id = $1").bind(id).fetch_optional(p).await },
    |p| { sqlx::query_as::<_, T>("... WHERE id = ?").bind(id).fetch_optional(p).await },
    |p| { sqlx::query_as::<_, T>("... WHERE id = ?").bind(id).fetch_optional(p).await },
);
```

体里只改三件事：
1. 执行器实参 `pool` → `p`（`p` 是该方言的**具体** pool 类型）
2. `return EXPR;` → `EXPR`（宏的分支值就是返回值）
3. `#[cfg(...)]` 属性行去掉

用法：
    python3 cfg_runtime.py <file.rs> [...]          # 原地改写
    python3 cfg_runtime.py --check <file.rs> [...]  # 只报告
"""
import re
import sys

CFG_RE = re.compile(r'^\s*#\[cfg\((.*)\)\]\s*$')
EXEC_RE = re.compile(r'\.(execute|fetch_all|fetch_one|fetch_optional)\(\s*&?\s*(?:state\.)?pool\s*\)')
POOL_EXPR_RE = re.compile(r'\.(?:execute|fetch_all|fetch_one|fetch_optional)\(\s*&?\s*((?:state\.)?pool)\s*\)')
DIALECT_ORDER = ['postgres', 'mysql', 'sqlite']


# --------------------------------------------------------------------------- 词法
def scan_block(lines, start):
    depth = 0
    for i in range(start, len(lines)):
        line = lines[i]
        j = 0
        n = len(line)
        while j < n:
            c = line[j]
            if c == 'r' and j + 1 < n and line[j + 1] in '#"':
                k = j + 1
                hashes = 0
                while k < n and line[k] == '#':
                    hashes += 1
                    k += 1
                if k < n and line[k] == '"':
                    term = '"' + '#' * hashes
                    end = line.find(term, k + 1)
                    if end == -1:
                        break
                    j = end + len(term)
                    continue
            if c == '"':
                j += 1
                while j < n:
                    if line[j] == '\\':
                        j += 2
                        continue
                    if line[j] == '"':
                        break
                    j += 1
                j += 1
                continue
            if c == '/' and j + 1 < n and line[j + 1] == '/':
                break
            if c == '{':
                depth += 1
            elif c == '}':
                depth -= 1
                if depth == 0:
                    return i
            j += 1
    raise RuntimeError('unbalanced brace from line %d' % (start + 1))


def statement_end(lines, start):
    """语句形态的 cfg 项：从 start 行开始，找到语句结束的那一行。

    结束判据：括号/花括号/方括号都配平后遇到的第一个 `;`（在同一行或后续行）。
    不能用"遇到空行 / `#[` / `}` 就停" —— 那会把后面**共享的代码**
    （例如 `affected += r.rows_affected();`）一起吞进 cfg 守卫的体里。
    """
    depth = 0
    for i in range(start, len(lines)):
        line = lines[i]
        j = 0
        n = len(line)
        while j < n:
            c = line[j]
            if c == 'r' and j + 1 < n and line[j + 1] in '#"':
                k = j + 1
                hashes = 0
                while k < n and line[k] == '#':
                    hashes += 1
                    k += 1
                if k < n and line[k] == '"':
                    term = '"' + '#' * hashes
                    end = line.find(term, k + 1)
                    if end == -1:
                        break
                    j = end + len(term)
                    continue
            if c == '"':
                j += 1
                while j < n:
                    if line[j] == '\\':
                        j += 2
                        continue
                    if line[j] == '"':
                        break
                    j += 1
                j += 1
                continue
            if c == '/' and j + 1 < n and line[j + 1] == '/':
                break
            if c in '([{':
                depth += 1
            elif c in ')]}':
                depth -= 1
            elif c == ';' and depth <= 0:
                return i
            j += 1
    return len(lines) - 1


def split_cfg_items(lines):
    """返回 [(attr_line, body_start, body_end, cfg_expr)]。"""
    items = []
    i = 0
    while i < len(lines):
        m = CFG_RE.match(lines[i])
        if not m:
            i += 1
            continue
        cfg = m.group(1)
        j = i + 1
        while j < len(lines) and lines[j].strip().startswith('#[') and not CFG_RE.match(lines[j]):
            j += 1
        while j < len(lines) and lines[j].strip() == '':
            j += 1
        if j >= len(lines):
            break
        if lines[j].lstrip().startswith('{'):
            end = scan_block(lines, j)
            items.append((i, j, end, cfg))
            i = end + 1
        else:
            end = statement_end(lines, j)
            items.append((i, j, end, cfg))
            i = end + 1
    return items


def cfg_dialects(expr):
    expr = expr.strip()
    m = re.fullmatch(r'feature\s*=\s*"(\w+)"', expr)
    if m:
        return {m.group(1)}
    m = re.fullmatch(r'not\s*\(\s*feature\s*=\s*"(\w+)"\s*\)', expr)
    if m:
        return {'mysql', 'postgres', 'sqlite'} - {m.group(1)}
    m = re.fullmatch(r'any\s*\((.*)\)', expr, re.S)
    if m:
        out = set()
        for part in re.findall(r'feature\s*=\s*"(\w+)"', m.group(1)):
            out.add(part)
        return out
    return None


# --------------------------------------------------------------------------- 体改写
def parse_stmt_body(body_lines):
    """把体解析成 (kind, name, expr_lines)。

    kind:
      'let'    —— `let X = EXPR;`（语句形态，合并后要把 `let X =` 提到宏外面）
      'expr'   —— `return EXPR;` / 裸 `EXPR;`（表达式形态，合并后整个体就是分支值）
    """
    lines = [l for l in body_lines]
    # 去掉首尾空行
    while lines and lines[0].strip() == '':
        lines.pop(0)
    while lines and lines[-1].strip() == '':
        lines.pop()
    if not lines:
        return ('expr', None, [])

    first = lines[0]
    m = re.match(r'^(\s*)let\s+(mut\s+)?(\w+)\s*(:[^=]*)?=\s*(.*)$', first)
    if m:
        # 只有**整条体就是这一条 `let`** 时，才能把 `let X =` 提到宏外面 ——
        # 否则后面还有语句用着 X（那属于方言各自的局部变量，必须留在分支里）。
        end = statement_end(lines, 0)
        rest = [l for l in lines[end + 1:] if l.strip() != '']
        if not rest:
            indent = m.group(1)
            name = m.group(3)
            typ = m.group(4) or ''
            tail = m.group(5)
            expr = [indent + tail]
            expr += lines[1:end + 1]
            return ('let', name + typ, expr)
        return ('expr', None, lines)
    # `return EXPR;` → `EXPR`（宏分支的值就是返回值；合并后是**尾部表达式**，
    # 末尾不能带分号）
    m = re.match(r'^(\s*)return\s+(.*)$', first)
    if m and not m.group(2).lstrip().startswith(('Ok(', 'Err(')):
        indent, rest = m.group(1), m.group(2)
        expr = [indent + rest]
        expr += lines[1:]
        return ('expr_ret', None, expr)
    return ('expr', None, lines)


def expr_lines_to_block(expr_lines, strip_await=False):
    """把表达式行变成宏分支块的内容（去掉末尾 `;`）。

    strip_await=True 时把末尾的 `.await` / `.await?` 去掉 ——
    `dispatch_exec_dialect!` 宏自己会 await（`execute()` 的返回类型是方言相关的，
    必须在宏里统一映射）。
    """
    out = []
    for l in expr_lines:
        out.append(EXEC_RE.sub(r'.\1(p)', l))
    if strip_await:
        text = '\n'.join(out)
        text = re.sub(r'\.await\?*\s*$', '', text)
        out = text.split('\n')
    for i in range(len(out) - 1, -1, -1):
        if out[i].strip() == '':
            continue
        if out[i].rstrip().endswith(';'):
            out[i] = out[i].rstrip()[:-1]
        break
    return out


def strip_outer_block(body_lines):
    """体是 `{ ... }` 时去掉最外层花括号（宏分支自己会加一层）。"""
    if not body_lines:
        return body_lines
    first = body_lines[0].strip()
    last = body_lines[-1].strip()
    if first == '{' and last == '}':
        return body_lines[1:-1]
    return body_lines


def transform(path, check_only=False):
    src = open(path, encoding='utf-8').read()
    lines = src.split('\n')
    items = split_cfg_items(lines)
    if not items:
        print('%s: 无 cfg 分支' % path)
        return 0

    # 分组：相邻的 cfg 项（中间只允许空行 / 注释）
    groups = []
    cur = [items[0]]
    for prev, nxt in zip(items, items[1:]):
        gap = lines[prev[2] + 1:nxt[0]]
        if all(l.strip() == '' or l.strip().startswith('//') for l in gap):
            cur.append(nxt)
        else:
            groups.append(cur)
            cur = [nxt]
    groups.append(cur)

    # 从后往前替换，避免行号漂移
    changed = 0
    for grp in reversed(groups):
        dialects = set()
        body_map = {}
        raw_map = {}
        ok = True
        kinds = set()
        names = set()
        pool_exprs = set()
        for (attr, bstart, bend, cfg) in grp:
            ds = cfg_dialects(cfg)
            if ds is None:
                ok = False
                break
            dialects |= ds
            raw_body = '\n'.join(lines[bstart:bend + 1])
            mp = POOL_EXPR_RE.search(raw_body)
            if mp:
                pool_exprs.add(mp.group(1))
            kind, name, expr = parse_stmt_body(strip_outer_block(lines[bstart:bend + 1]))
            kinds.add(kind)
            names.add(name)
            body = expr_lines_to_block(expr)
            for d in ds:
                raw_map[d] = expr
                body_map[d] = body
        if not ok:
            print('%s: 跳过无法识别的 cfg 组: %s' % (path, [g[3] for g in grp]))
            continue
        if dialects != {'mysql', 'postgres', 'sqlite'}:
            print('%s: 跳过方言集合不全的组 (%s)' % (path, sorted(dialects)))
            continue
        if len(kinds) != 1:
            print('%s: 跳过形态不一致的组 (kinds=%s)' % (path, kinds))
            continue
        if len(pool_exprs) > 1:
            print('%s: 跳过 pool 表达式不一致的组 (%s)' % (path, pool_exprs))
            continue
        pool_expr = pool_exprs.pop() if pool_exprs else 'pool'

        kind = kinds.pop()
        name = names.pop()
        # `execute()` 的返回类型是**方言相关**的 —— 三个分支不能直接放进同一个
        # match，必须用 `dispatch_exec_dialect!` 统一映射成 `ExecResult`。
        sample = '\n'.join(body_map[DIALECT_ORDER[0]])
        is_exec = '.execute(' in sample
        # 生成宏调用
        indent = re.match(r'^\s*', lines[grp[0][0]]).group(0)
        arms = []
        for d in DIALECT_ORDER:
            body = expr_lines_to_block(body_map[d], strip_await=is_exec)
            arms.append('|p| {\n' + '\n'.join(
                ('    ' + l) if l.strip() else l for l in body) + '\n}')
        macro = 'dispatch_exec_dialect!' if is_exec else 'dispatch_dialect!'
        new = []
        if kind == 'let':
            new.append('%slet %s = crate::%s(' % (indent, name, macro))
        else:
            new.append('%scrate::%s(' % (indent, macro))
        new.append('%s    %s,' % (indent, pool_expr))
        for a in arms:
            new.append('%s    %s,' % (indent, a.replace('\n', '\n' + indent + '    ')))
        # execute 形态：`?` 要提到宏外面（宏体里已经没有 `.await?` 了）
        # 尾部表达式（块的最后一个语句不以 `;` 结尾）→ 合并后**不能**带分号；
        # 语句位置的块 → 要带分号。
        last = ''
        for l in reversed(raw_map[DIALECT_ORDER[0]]):
            if l.strip():
                last = l.strip()
                break
        tail_semi = '' if (kind == 'expr_ret' or not last.endswith(';')) else ';'
        new.append('%s)%s%s' % (indent, '?' if is_exec else '', tail_semi))

        # 替换区间：第一个 cfg 属性行 → 最后一个项的 body 结束行
        start = grp[0][0]
        end = grp[-1][2]
        lines[start:end + 1] = new
        changed += 1

    if not check_only:
        open(path, 'w', encoding='utf-8').write('\n'.join(lines))
    print('%s: 改写 %d 组 cfg 分支' % (path, changed))
    return changed


if __name__ == '__main__':
    args = sys.argv[1:]
    check = '--check' in args
    files = [a for a in args if not a.startswith('--')]
    total = 0
    for f in files:
        total += transform(f, check)
    print('合计改写 %d 组' % total)

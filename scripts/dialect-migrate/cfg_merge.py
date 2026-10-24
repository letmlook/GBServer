#!/usr/bin/env python3
"""把「编译期 cfg 方言分支」合并成「运行期按 dialect 分发」。

**只在归一化后各分支完全一致时才合并** —— 不一致就报出来交给人工，
绝不"猜着改"。归一化规则：

* `$1` / `$2` …        → `?`（占位符统一由 `pool.sql()` 在运行期改写）
* `AS TEXT`           → `AS CHAR`（pg 的 TEXT 与 mysql/sqlite 的 CHAR 语义等价）
* `= true` / `= false`→ `= 1` / `= 0`（布尔字面量）
* 裸 `true` / `false` → `1` / `0`

合并出来的形状：

```rust
// 原来是：#[cfg(mysql)] { let r = EXPR.await?; Ok(r.rows_affected()) }
//                     #[cfg(postgres)] { ...同构... }
let r = crate::dispatch_exec!(pool, |p| { EXPR })?;
Ok(r.rows_affected())
```

用法：
    python3 cfg_merge.py <file.rs> [...]          # 原地改写 + 打印待人工处理的点
    python3 cfg_merge.py --check <file.rs> [...]  # 只报告
"""
import re
import sys

CFG_RE = re.compile(r'^\s*#\[cfg\((.*)\)\]\s*$')


# --------------------------------------------------------------------------- 词法
def scan_block(lines, start):
    """lines[start] 以 '{' 开头，返回配对 '}' 的行号。"""
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
        stripped = lines[j].lstrip()
        if stripped.startswith('{'):
            end = scan_block(lines, j)
            items.append((i, j, end, cfg))
            i = end + 1
        else:
            end = j
            while end + 1 < len(lines):
                nxt = lines[end + 1]
                s = nxt.strip()
                if s == '' or s.startswith('#[') or nxt.startswith('}'):
                    break
                end += 1
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


# --------------------------------------------------------------------------- 归一化
def normalize(text):
    t = text
    t = re.sub(r'\$\d+', '?', t)
    t = t.replace('AS TEXT', 'AS CHAR')
    t = re.sub(r'\btrue\b', '1', t)
    t = re.sub(r'\bfalse\b', '0', t)
    t = re.sub(r'\s+', ' ', t)
    return t.strip()


# --------------------------------------------------------------------------- 主体改写
EXEC_RE = re.compile(r'\.(execute|fetch_all|fetch_one|fetch_optional)\(pool\)')


def rewrite_body(body_lines):
    """把体里的 pool→p、去 .await、去 dialect_sql 的临时借用问题。

    返回 (新体行列表, 需要外提的 await 语句数)
    """
    text = '\n'.join(body_lines)
    text = EXEC_RE.sub(r'.\1(p)', text)
    # `crate::dyn_where::dialect_sql(` / `crate::db::dialect::current_sql(` → `pool.sql(`
    text = text.replace('crate::dyn_where::dialect_sql(', 'pool.sql(')
    text = text.replace('crate::db::dialect::current_sql(', 'pool.sql(')
    return text.split('\n')


def find_await_stmt(text):
    """在体文本里找 `.await` 所在的整条语句。

    返回 (stmt_start, stmt_end, prefix_kind)：
      prefix_kind = 'let' | 'return' | 'tail'
    """
    idx = text.find('.await')
    if idx == -1:
        return None
    # 语句起点：往前找行首的 `let X = ` / `return ` / 裸表达式
    head = text[:idx]
    # 找最后一个换行后的内容
    lines = head.split('\n')
    last = lines[-1]
    stripped = last.lstrip()
    if re.match(r'let\s+\w+\s*=', stripped):
        kind = 'let'
        start_off = head.rfind('\n') + 1
    elif stripped.startswith('return'):
        kind = 'return'
        start_off = head.rfind('\n') + 1
    else:
        kind = 'tail'
        # 裸表达式：往前找到行首
        start_off = head.rfind('\n') + 1
        while start_off < len(head):
            seg = head[start_off:]
            if seg.strip():
                break
            start_off = head.find('\n', start_off) + 1
    # 语句终点：`.await` 之后可能还有 `?` 和 `;`
    tail = text[idx + len('.await'):]
    m = re.match(r'(\??)\s*;?', tail)
    end = idx + len('.await') + (m.end() if m else 0)
    return (start_off, end, kind, m.group(1) if m else '')

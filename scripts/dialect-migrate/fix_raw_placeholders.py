#!/usr/bin/env python3
"""把「dispatch 宏体里仍是裸 `?` 占位符」的 SQL 统一包上运行期方言改写。

## 为什么必须做

改造后 `dispatch!` 的宏体在**三个方言分支里展开同一份代码** —— 宏体里的 SQL
文本必须**三种方言都能解析**。postgres 的预编译语句协议只认 `$1`，把 `?` 原样
发给它会得到 `syntax error`。

而改造前这些语句在 pg 构建里用的是另一份 `$N` 文本（编译期 `#[cfg]` 分支），
转换时两支被合成了一支 —— 于是 pg 上就炸了。

## 改法

把裸字面量包成 `&crate::dyn_where::dialect_sql("...")`：
* 非 postgres 方言：原样返回（零拷贝，`Cow::Borrowed`）；
* postgres：改写成 `$1..$n`。

`dialect_sql` 返回 `Cow<'_, str>`，`&Cow<str>` 会自动 deref 成 `&str`。

**注意生命周期**：临时 `Cow` 必须活到查询执行完 ——
* 字面量在**块��尾表达式**里 → 临时值活到外层语句结束 ✓（可以内联包）；
* 字面量在 **`let` 初始化**里 → 临时值在 `let` 语句结束就析构 ✗（必须先提升成变量）。

用法：
    python3 fix_raw_placeholders.py <file.rs> [...]
"""
import re
import sys
import pathlib

SQL_RE = re.compile(
    r'\b(SELECT|INSERT|UPDATE|DELETE|ALTER|CREATE)\b', re.I)
QUERY_CALL_RE = re.compile(
    r'sqlx::query(?:_as|_scalar)?\s*(?:::<[^>]*>)?\s*\(')
WRAPPED_RE = re.compile(r'(dialect_sql\(|\.sql\()\s*&?\s*$')


def find_literals(src):
    """返回 [(start, end, text)] —— 普通字符串字面量（跳过 raw string 与注释）。"""
    out = []
    i = 0
    n = len(src)
    while i < n:
        c = src[i]
        if c == 'r' and i + 1 < n and src[i + 1] in '#"':
            k = i + 1
            hashes = 0
            while k < n and src[k] == '#':
                hashes += 1
                k += 1
            if k < n and src[k] == '"':
                term = '"' + '#' * hashes
                end = src.find(term, k + 1)
                if end == -1:
                    break
                out.append((i, end + len(term), src[i:end + len(term)]))
                i = end + len(term)
                continue
        if c == '"':
            j = i + 1
            while j < n:
                if src[j] == '\\':
                    j += 2
                    continue
                if src[j] == '"':
                    break
                j += 1
            out.append((i, j + 1, src[i:j + 1]))
            i = j + 1
            continue
        if c == '/' and i + 1 < n and src[i + 1] == '/':
            end = src.find('\n', i)
            i = n if end == -1 else end
            continue
        if c == '/' and i + 1 < n and src[i + 1] == '*':
            end = src.find('*/', i)
            i = n if end == -1 else end + 2
            continue
        i += 1
    return out


def is_sql_literal(text):
    body = text[1:-1]
    if '?' not in body:
        return False
    return bool(SQL_RE.search(body))


def already_wrapped(src, pos):
    head = src[max(0, pos - 60):pos]
    return bool(WRAPPED_RE.search(head))


def fix(path):
    src = pathlib.Path(path).read_text(encoding="utf-8")
    lits = find_literals(src)
    targets = []
    for (s, e, text) in lits:
        if not is_sql_literal(text):
            continue
        if already_wrapped(src, s):
            continue
        # 必须是 sqlx::query*( 的**第一个参数**
        head = src[:s]
        m = None
        for mm in QUERY_CALL_RE.finditer(head):
            m = mm
        if not m:
            continue
        # query*( 与字面量之间只允许空白与 `&`
        between = src[m.end():s]
        if between.strip() not in ('', '&'):
            continue
        targets.append((s, e, text))
    if not targets:
        return 0
    # 从后往前替换
    out = src
    for (s, e, text) in reversed(targets):
        out = out[:s] + '&crate::dyn_where::dialect_sql(' + text + ')' + out[e:]
    pathlib.Path(path).write_text(out, encoding="utf-8")
    return len(targets)


if __name__ == '__main__':
    total = 0
    for f in sys.argv[1:]:
        n = fix(f)
        total += n
        if n:
            print("%s: 包裹 %d 处" % (f, n))
    print("合计 %d 处" % total)

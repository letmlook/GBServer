#!/usr/bin/env python3
"""把残留的 `#[cfg(feature = "mysql"/"postgres"/"sqlite")]` 改写成**运行期**方言判断。

改造前三个后端是互斥 cargo feature，cfg 决定「编译进哪个方言」。
现在三个驱动同时编进同一个二进制，cfg 必须变成运行期的方言判断 ——
否则**三个方言的代码会同时编译并执行**。

块形态（`#[cfg(...)] { ... }`）自动转换；项形态（const/fn/struct）列入人工清单。
"""
import re
import sys
import pathlib
from collections import Counter

CFG_RE = re.compile(r'^(\s*)#\[cfg\((?P<cond>[^)]*(?:\([^)]*\)[^)]*)*)\)\]\s*$')

FEAT_RE = re.compile(r'feature\s*=\s*"(mysql|postgres|sqlite)"')


def dialect_call(feats, negated=False, any_=False):
    """把 cfg 条件翻译成运行期方言判断表达式。"""
    if not feats:
        return None
    # 统一返回**完整的条件表达式**（含 `current().` 前缀）
    if any_:
        # 任一命中 → OR
        parts = []
        for f in feats:
            if f == 'postgres':
                parts.append('crate::db::dialect::current().is_postgres()')
            elif f == 'mysql':
                parts.append('crate::db::dialect::current().is_mysql()')
            else:
                parts.append('crate::db::dialect::current().is_sqlite()')
        expr = ' || '.join(parts)
    else:
        f = feats[0]
        method = {'postgres': 'is_postgres()', 'mysql': 'is_mysql()', 'sqlite': 'is_sqlite()'}[f]
        expr = 'crate::db::dialect::current().%s' % method
    return ('!' + expr) if negated else expr


def parse_cond(cond):
    """返回 (feats, negated, any_) 或 None（无法自动翻译）。"""
    c = cond.strip()
    negated = False
    any_ = False
    if c.startswith('not('):
        negated = True
        c = c[4:].rstrip()
        if c.endswith(')'):
            c = c[:-1]
    if c.startswith('any('):
        any_ = True
        c = c[4:].rstrip()
        if c.endswith(')'):
            c = c[:-1]
    feats = FEAT_RE.findall(c)
    if not feats:
        return None
    # 条件里若还有别的东西（如 feature="x" 之外的），不自动处理
    residue = FEAT_RE.sub('', c)
    if 'feature' in residue:
        return None
    return (feats, negated, any_)


def find_stmt_end(lines, start):
    """从 start 行开始，返回**语句**结束行号（含）。括号/字符串感知。"""
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
            if c == '(':
                depth += 1
            elif c == ')':
                depth -= 1
            elif c == ';' and depth == 0:
                return i
            j += 1
    return -1


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


def merge_triples(path, lines):
    """把**连续的三元组**（mysql / postgres / sqlite 各一条）合并成运行期 match。

    改造前三个方言各写一份语句，编译期三选一；现在三个驱动同时在位，
    必须按运行期方言挑一份执行 —— 否则三条都会执行，且 `let x` 的作用域
    只在各自的块里，后续代码拿不到。

    返回 (新行列表, 合并处数, 人工清单)。
    """
    out = []
    i = 0
    merged = 0
    manual = []
    while i < len(lines):
        m = CFG_RE.match(lines[i])
        if not m:
            out.append(lines[i])
            i += 1
            continue
        feats = FEAT_RE.findall(m.group('cond'))
        if len(feats) != 1 or feats[0] not in ('mysql', 'postgres', 'sqlite'):
            out.append(lines[i])
            i += 1
            continue
        # 收集本方言的语句范围
        j = i + 1
        while j < len(lines) and lines[j].strip() == '':
            j += 1
        if j >= len(lines):
            out.append(lines[i])
            i += 1
            continue
        if lines[j].strip() == '{':
            end = find_block_end(lines, j)
            if end == -1:
                out.append(lines[i])
                i += 1
                continue
            stmt_end = end
            body_start = j
        else:
            stmt_end = find_stmt_end(lines, j)
            if stmt_end == -1:
                out.append(lines[i])
                i += 1
                continue
            body_start = j
        # 找后面两个方言（中间只能有空白）
        groups = {}
        groups[feats[0]] = (body_start, stmt_end)
        k = stmt_end + 1
        ok = True
        for _ in range(2):
            # 三个方言分支之间允许夹**注释行**（方言专属的说明注释很常见，
            # 例如「// SQLite：DATETIME 直接以文本存储」）—— 注释不影响语义。
            while k < len(lines) and (lines[k].strip() == ''
                                      or lines[k].strip().startswith('//')):
                k += 1
            if k >= len(lines):
                ok = False
                break
            m2 = CFG_RE.match(lines[k])
            if not m2:
                ok = False
                break
            f2 = FEAT_RE.findall(m2.group('cond'))
            if len(f2) != 1 or f2[0] in groups:
                ok = False
                break
            j2 = k + 1
            while j2 < len(lines) and lines[j2].strip() == '':
                j2 += 1
            if j2 >= len(lines):
                ok = False
                break
            if lines[j2].strip() == '{':
                e2 = find_block_end(lines, j2)
                if e2 == -1:
                    ok = False
                    break
            else:
                e2 = find_stmt_end(lines, j2)
                if e2 == -1:
                    ok = False
                    break
            groups[f2[0]] = (j2, e2)
            k = e2 + 1
        if not ok or len(groups) != 3:
            out.append(lines[i])
            i += 1
            continue

        indent = m.group(1)
        # 每个方言的 arm：语句形态拆成 `let NAME = <expr>` 的 expr 部分；
        # 块形态整个块就是 arm。
        arms = []
        names = []
        for f in ('mysql', 'postgres', 'sqlite'):
            bs, be = groups[f]
            seg = lines[bs:be + 1]
            if seg and seg[0].strip() == '{':
                # 块形态：**递归**处理块内可能还有一层三元组
                inner_lines = seg[1:-1]
                inner_lines, _, _ = merge_triples(path, inner_lines)
                arms.append('\n'.join([seg[0]] + inner_lines + [seg[-1]]))
                names.append(None)
            else:
                txt = '\n'.join(seg).rstrip()
                if txt.endswith(';'):
                    txt = txt[:-1].rstrip()
                nm = re.match(
                    r'\s*let\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*[^=]+?)?\s*=\s*(.*)$',
                    txt, re.S)
                if nm:
                    arms.append(nm.group(2))
                    names.append(nm.group(1))
                else:
                    arms.append(txt)
                    names.append(None)
        # 用 **if / else if / else 链** 而不是 match：
        # 三个分支各自独立求值，**整体类型只由最后一个分支决定** ——
        # 三个方言的中间类型不一致时（比如 i32 vs i64）match 会直接报
        # "arms have incompatible types"，if-else 链不会。
        same_name = names[0] is not None and names[0] == names[1] == names[2]
        prefix = ('let %s = ' % names[0]) if same_name else ''
        cond = {
            'mysql': 'crate::db::dialect::current().is_mysql()',
            'postgres': 'crate::db::dialect::current().is_postgres()',
            'sqlite': 'crate::db::dialect::current().is_sqlite()',
        }
        out.append('%s%sif %s {' % (indent, prefix, cond['mysql']))
        for ln in arms[0].split('\n'):
            out.append('%s    %s' % (indent, ln) if ln.strip() else ln)
        out.append('%s} else if %s {' % (indent, cond['postgres']))
        for ln in arms[1].split('\n'):
            out.append('%s    %s' % (indent, ln) if ln.strip() else ln)
        out.append('%s} else {' % indent)
        for ln in arms[2].split('\n'):
            out.append('%s    %s' % (indent, ln) if ln.strip() else ln)
        out.append('%s}%s' % (indent, ';' if same_name else ''))
        merged += 1
        i = k
    return out, merged, manual



def pool_ref(lines, fn_head):
    """返回该函数里 pool 的**实际引用表达式**。

    - `pool: &Pool` → "pool"
    - `State(state): State<AppState>` → "state.pool"
    - `pool: Option<crate::db::Pool>` → "pool"（调用点再解包）
    """
    # 扫函数签名（前 12 行）
    head = '\n'.join(lines[fn_head:fn_head + 12])
    m = re.search(r'\b(\w+)\s*:\s*&\s*(?:crate::db::)?Pool\b', head)
    if m:
        return m.group(1)
    m = re.search(r'\b(\w+)\s*:\s*State\s*\(\s*AppState\s*\)', head)
    if m:
        return m.group(1) + '.pool'
    m = re.search(r'\b(\w+)\s*:\s*&\s*AppState\b', head)
    if m:
        return m.group(1) + '.pool'
    return 'pool'

def main():
    apply = '--apply' in sys.argv
    stats = Counter()
    manual = []

    for path in sorted(pathlib.Path('src').rglob('*.rs')):
        lines = path.read_text().split('\n')
        lines, nmerged, _ = merge_triples(path, lines)
        stats['merged'] += nmerged
        out = []
        i = 0
        changed = False
        while i < len(lines):
            m = CFG_RE.match(lines[i])
            if not m:
                out.append(lines[i])
                i += 1
                continue
            raw_cond = m.group('cond').strip()
            # 三个方言 feature 恒定开启 → `all(test, feature = X)` ≡ `test`
            mtest = re.match(r'^all\(\s*test\s*,\s*feature\s*=\s*"(mysql|postgres|sqlite)"\s*\)$', raw_cond)
            if mtest:
                out.append('%s#[cfg(test)]' % m.group(1))
                i += 1
                stats['test_gate'] += 1
                changed = True
                continue
            # `any(feature = "postgres", test)` —— postgres 恒真 → 恒成立
            many = re.match(r'^any\(\s*feature\s*=\s*"(mysql|postgres|sqlite)"\s*,\s*test\s*\)$', raw_cond)
            if many:
                out.append('%s#[cfg(test)]' % m.group(1))
                i += 1
                stats['test_gate'] += 1
                changed = True
                continue

            parsed = parse_cond(m.group('cond'))
            if parsed is None:
                out.append(lines[i])
                i += 1
                continue
            feats, negated, any_ = parsed
            indent = m.group(1)

            # 找下一非空行
            j = i + 1
            while j < len(lines) and lines[j].strip() == '':
                j += 1
            if j >= len(lines):
                out.append(lines[i])
                i += 1
                continue

            nxt = lines[j].strip()
            if nxt == '{':
                # 块形态 → 运行期 if。
                # **必须同时跳过 cfg 行和原块的 `{` 行** —— 只跳 cfg 行会多留
                # 一层花括号（`if ... { { ... } }`），编译报 unclosed delimiter。
                expr = dialect_call(feats, negated, any_)
                out.append('%sif %s {' % (indent, expr))
                i += 2
                stats['block'] += 1
                changed = True
                continue
            if nxt.startswith('//') or nxt == '':
                out.append(lines[i])
                i += 1
                continue

            # 测试函数：`#[cfg(feature = "sqlite")]` 修饰的 `#[test]` /
            # `#[tokio::test]` —— sqlite feature 现在恒定开启 → 转 `#[cfg(test)]`
            k = j
            while k < len(lines) and lines[k].strip().startswith('#[') and 'test' not in lines[k]:
                k += 1
            if k < len(lines) and re.match(r'\s*#\[(tokio::)?test\]', lines[k]):
                out.append('%s#[cfg(test)]' % indent)
                i += 1
                stats['test_fn'] += 1
                changed = True
                continue

            # `use` 语句：三方言都可用 → 直接去掉 cfg
            if nxt.startswith('use '):
                stats['use'] += 1
                changed = True
                i += 1
                continue

            # 项形态（const/fn/struct/impl）→ 人工
            if re.match(r'(pub\s+)?(async\s+)?(const|static|fn|struct|enum|trait|impl|mod|type)\b', nxt):
                manual.append('%s:%d  %s  → %s' % (path, i + 1, lines[i].strip(), nxt[:50]))
                stats['item'] += 1
                out.append(lines[i])
                i += 1
                continue

            # 语句形态：`#[cfg(...)] let r = ...;` → 包进运行期 if
            stmt_end = find_stmt_end(lines, j)
            if stmt_end == -1:
                out.append(lines[i])
                i += 1
                continue
            expr = dialect_call(feats, negated, any_)
            out.append('%sif %s {' % (indent, expr))
            for k in range(j, stmt_end + 1):
                out.append('    ' + lines[k] if lines[k].strip() else lines[k])
            out.append('%s}' % indent)
            i = stmt_end + 1
            stats['stmt'] += 1
            changed = True

        if changed and apply:
            path.write_text('\n'.join(out))

    print("块形态改写: %d   语句形态改写: %d   use 去 cfg: %d   测试门控: %d   测试函数: %d   项形态待人工: %d"
          % (stats['block'], stats['stmt'], stats['use'], stats['test_gate'],
             stats['test_fn'], stats['item']))
    for m in manual:
        print("   ", m)
    return 0


if __name__ == '__main__':
    sys.exit(main())

#!/usr/bin/env python3
"""错误驱动：把「pool 直接作为 sqlx 方法参数」的调用包进 dispatch。

按 `cargo check` 报的位置，精确找出所有
    .fetch_all(pool) / .fetch_one(pool) / .fetch_optional(pool) / .execute(pool)
这类**直接把 pool 交给 sqlx**的调用（方言枚举不能当 Executor 用），
向前扫到本语句的 `sqlx::query*`、向后扫到 `.await`，然后包进 `dispatch!`。

扫描用括号平衡，遇到**顶层分号**即判定跨语句 → 放弃（那属于变量形式，
由 04_wrap_var_queries.py 处理）。
"""
import re
import sys
import pathlib

ERR_RE = re.compile(r'^(src/[^:]+):(\d+):(\d+): error\[E0277\]')
POOL_ARG_RE = re.compile(r'\.\w+\s*\(\s*&?\s*(?:\w+\.)?pool\s*\)')
ENTRY_RE = re.compile(r'sqlx::query(?:_as|_scalar)?\b')
FNSIG_RE = re.compile(r'(?:pub\s+)?(?:async\s+)?fn\s+\w+\s*\([^)]*\)', re.S)


def pool_ref_text(src, pos):
    head = src[:pos]
    matches = list(FNSIG_RE.finditer(head))
    if not matches:
        return 'pool'
    sig = matches[-1].group(0)
    m = re.search(r'\b(\w+)\s*:\s*&\s*(?:crate::db::)?Pool\b', sig)
    if m:
        return m.group(1)
    m = re.search(r'\b(\w+)\s*:\s*State\s*\(\s*AppState\s*\)', sig)
    if m:
        return '&' + m.group(1) + '.pool'
    m = re.search(r'\b(\w+)\s*:\s*&\s*AppState\b', sig)
    if m:
        return '&' + m.group(1) + '.pool'
    return 'pool'


def scan_back(src, pool_pos):
    """从 pool 参数位置向前扫到本语句的 `sqlx::query*`，再向后扫到 `.await`。

    返回 (start, end) 或 None。
    """
    # 向前：最近的 sqlx::query，且中间不能有顶层分号
    idx = src.rfind('sqlx::query', 0, pool_pos)
    if idx == -1:
        return None
    # 检查 idx..pool_pos 之间有没有顶层分号（跨语句了）
    seg = src[idx:pool_pos]
    depth_p = depth_b = 0
    i = 0
    while i < len(seg):
        c = seg[i]
        if c in '"\'':
            q = c
            i += 1
            while i < len(seg):
                if seg[i] == '\\':
                    i += 2
                    continue
                if seg[i] == q:
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
            return None          # 跨语句
        i += 1

    # 向后：从 pool 参数处扫到 `.await`
    j = src.index(')', pool_pos) + 1
    # 后续的链式调用
    while True:
        mm = re.match(r'\s*\.\w+\s*\(', src[j:])
        if not mm:
            break
        k = j + mm.end() - 1
        d = 0
        while k < len(src):
            c = src[k]
            if c in '"\'':
                q = c
                k += 1
                while k < len(src):
                    if src[k] == '\\':
                        k += 2
                        continue
                    if src[k] == q:
                        break
                    k += 1
            elif c == '(':
                d += 1
            elif c == ')':
                d -= 1
                if d == 0:
                    k += 1
                    break
            k += 1
        j = k
    ma = re.match(r'\s*\.await\b', src[j:])
    if not ma:
        return None
    return (idx, j + ma.end())


def main():
    check_file = None
    apply = False
    args = sys.argv[1:]
    for i, a in enumerate(args):
        if a == '--check':
            check_file = args[i + 1]
        if a == '--apply':
            apply = True
    if not check_file:
        print('需要 --check <cargo check 输出>')
        return 1

    files = set()
    for raw in open(check_file, encoding='utf-8', errors='replace'):
        m = ERR_RE.match(raw.strip())
        if m:
            files.add(m.group(1))

    total = 0
    for rel in sorted(files):
        path = pathlib.Path(rel)
        if not path.exists():
            continue
        src = path.read_text()
        edits = []
        seen = set()
        for m in POOL_ARG_RE.finditer(src):
            pos = m.start()
            sc = scan_back(src, pos)
            if not sc:
                continue
            start, end = sc
            if start in seen:
                continue
            # 已在一个 dispatch 里？
            before = src[:start]
            if before.rfind('crate::dispatch') > before.rfind('}'):
                seen.add(start)
                continue
            seen.add(start)
            pref = pool_ref_text(src, start)
            body = src[start:end]
            body = re.sub(r'&(?:\w+\.)?pool\b', 'p', body)
            body = re.sub(r'(?<![.\w])pool\b(?!\s*\.)', 'p', body)
            body = body.rstrip()
            if body.endswith('.await'):
                body = body[:-len('.await')].rstrip()
            is_exec = '.execute(' in body
            macro = 'crate::dispatch_exec!' if is_exec else 'crate::dispatch!'
            edits.append((start, end, '%s(%s, |p| {\n%s\n})' % (macro, pref, body)))
        if not edits:
            continue
        # 去重叠：一条查询链里可能有两个 pool 参数（`sqlx::query(&sql).bind(&pool)`），
        # 两次编辑的范围会重叠 → 替换错位 → 文件内容被重复插入。
        keep = []
        for e in sorted(edits, key=lambda x: (x[1] - x[0])):
            if any(k[0] <= e[0] and e[1] <= k[1] for k in keep):
                continue
            keep = [k for k in keep if not (e[0] <= k[0] and k[1] <= e[1])]
            keep.append(e)
        out = src
        for start, end, new in sorted(keep, key=lambda e: -e[0]):
            out = out[:start] + new + out[end:]
        if apply:
            path.write_text(out)
        total += len(edits)
        print('  %s: %d 处' % (rel, len(edits)))
    print('共修复 %d 处' % total)
    return 0


if __name__ == '__main__':
    sys.exit(main())

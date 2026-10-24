#!/usr/bin/env python3
"""修「dispatch 调用里的 `pool` 不在作用域」。

handlers 层的函数签名是 `State(state): State<AppState>` —— 手里没有 `pool`
变量，pool 是 `state.pool`。转换脚本按 db 层的习惯生成了
`crate::dispatch!(pool, ...)` → 编译报 "cannot find value `pool`"。

本脚本按**编译器的报错位置**，把那个 `pool` 换成该函数里真正的 pool 引用
（`&state.pool` / `pool` / `&app_state.pool` …）。
"""
import re
import sys
import pathlib

ERR_RE = re.compile(r'^(src/[^:]+):(\d+):(\d+): error\[E0425\]')
FNSIG_RE = re.compile(r'(?:pub\s+)?(?:async\s+)?fn\s+\w+\s*\([^)]*\)', re.S)


def fn_of(src, pos):
    """返回包含 pos 的**最内层**函数签名文本。"""
    head = src[:pos]
    # 取最后一个 'fn ' 之前的签名（到配对的 ')'）
    idx = head.rfind('fn ')
    if idx == -1:
        return None
    # 从 idx 往前的函数名
    start = head.rfind('\n', 0, idx)
    line = head[start + 1:idx]
    # 找签名里的参数部分
    p = head.find('(', idx)
    if p == -1:
        return None
    d = 0
    i = p
    while i < len(head):
        if head[i] == '(':
            d += 1
        elif head[i] == ')':
            d -= 1
            if d == 0:
                break
        i += 1
    return head[start + 1:i + 1]


def pool_ref(sig):
    if not sig:
        return None
    m = re.search(r'\b(\w+)\s*:\s*&\s*(?:crate::db::)?Pool\b', sig)
    if m:
        return m.group(1)
    # 模式解构写法：`State(state): State<AppState>`
    m = re.search(r'\bState\s*\(\s*(\w+)\s*\)\s*:\s*State\s*<\s*AppState\s*>', sig)
    if m:
        return '&' + m.group(1) + '.pool'
    m = re.search(r'\b(\w+)\s*:\s*State\s*<\s*AppState\s*>', sig)
    if m:
        return '&' + m.group(1) + '.pool'
    m = re.search(r'\b(\w+)\s*:\s*&\s*AppState\b', sig)
    if m:
        return '&' + m.group(1) + '.pool'
    return None


def main():
    check_file = None
    apply = '--apply' in sys.argv
    args = sys.argv[1:]
    for i, a in enumerate(args):
        if a == '--check':
            check_file = args[i + 1]
    if not check_file:
        print('需要 --check <cargo check 输出>')
        return 1

    sites = {}
    for raw in open(check_file, encoding='utf-8', errors='replace'):
        m = ERR_RE.match(raw.strip())
        if m and 'pool' in raw:
            sites.setdefault(m.group(1), []).append(int(m.group(2)))

    total = 0
    for rel, lns in sorted(sites.items()):
        path = pathlib.Path(rel)
        if not path.exists():
            continue
        src = path.read_text()
        lines = src.split('\n')
        edits = []
        for ln in sorted(set(lns)):
            if ln - 1 >= len(lines):
                continue
            line = lines[ln - 1]
            m = re.search(r'(?<![.\w])pool\b', line)
            if not m:
                continue
            line_off = sum(len(l) + 1 for l in lines[:ln - 1])
            pos = line_off + m.start()
            # `m.end()` 是**行内**偏移，换算成绝对偏移
            end_abs = line_off + m.end()
            sig = fn_of(src, pos)
            ref = pool_ref(sig)
            if not ref or ref == 'pool':
                continue
            edits.append((pos, end_abs, ref))
        if not edits:
            continue
        # 去重叠：同一条 dispatch 调用里可能出现多个 `pool`
        # （`crate::dispatch!(pool, |p| { ... state.pool ... }`），
        # 两次编辑范围重叠 → 替换错位 → 文件内容被重复插入。
        keep = []
        for e in sorted(edits, key=lambda x: (x[1] - x[0])):
            if any(k[0] <= e[0] and e[1] <= k[1] for k in keep):
                continue
            keep = [k for k in keep if not (e[0] <= k[0] and k[1] <= e[1])]
            keep.append(e)
        out = src
        for s, e, ref in sorted(keep, key=lambda x: -x[0]):
            out = out[:s] + ref + out[e:]
        if apply:
            path.write_text(out)
        total += len(edits)
        print('  %s: %d 处' % (rel, len(edits)))
    print('共修正 %d 处' % total)
    return 0


if __name__ == '__main__':
    sys.exit(main())

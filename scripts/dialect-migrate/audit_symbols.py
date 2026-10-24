#!/usr/bin/env python3
"""转换后体检：比对「函数/常量数量」与原文，揪出内容被吞的文件。"""
import subprocess, pathlib, re, sys

# 改造前的基线提交（不能用 HEAD —— HEAD 已经是 WIP，丢了函数也看不出来）
BASE = '43b178b'

FUNC_RE = re.compile(r'^\s*(?:pub(?:\([a-z]+\))?\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)')
STRUCT_RE = re.compile(r'^\s*(?:pub(?:\([a-z]+\))?\s+)?(?:struct|enum|trait)\s+([A-Za-z_][A-Za-z0-9_]*)')
CONST_RE = re.compile(r'^\s*(?:pub(?:\([a-z]+\))?\s+)?(?:const|static)\s+([A-Za-z_][A-Za-z0-9_]*)')

def symbols(text):
    fns, types, consts = set(), set(), set()
    for line in text.split('\n'):
        m = FUNC_RE.match(line)
        if m: fns.add(m.group(1))
        m = STRUCT_RE.match(line)
        if m: types.add(m.group(1))
        m = CONST_RE.match(line)
        if m: consts.add(m.group(1))
    return fns, types, consts

bad = []
total = 0
for p in sorted(pathlib.Path('src').rglob('*.rs')):
    rel = str(p)
    try:
        orig = subprocess.run(['git', 'show', BASE + ':' + rel], capture_output=True, text=True, check=True).stdout
    except subprocess.CalledProcessError:
        continue
    total += 1
    ofn, otp, ocs = symbols(orig)
    cfn, ctp, ccs = symbols(p.read_text())
    lost_fn = ofn - cfn
    lost_ty = otp - ctp
    lost_cs = ocs - ccs
    if lost_fn or lost_ty or lost_cs:
        bad.append((rel, sorted(lost_fn), sorted(lost_ty), sorted(lost_cs)))

print("=== 符号丢失的文件（%d / %d）===" % (len(bad), total))
for rel, lf, lt, lc in bad:
    print("  %s" % rel)
    if lf: print("     fn 丢失: %s" % ', '.join(lf[:12]))
    if lt: print("     类型丢失: %s" % ', '.join(lt[:12]))
    if lc: print("     常量丢失: %s" % ', '.join(lc[:12]))
if not bad:
    print("  （无）")

#!/usr/bin/env bash
# 内容膨胀守卫：转换脚本若把文件内容**重复插入**，行数会异常增长。
# 每次跑转换后都必须过这一关 —— 膨胀是"静默改坏代码"最典型的症状
# （历史上就栽过：文件内容被重复插入 11 倍）。
#
# 两个阈值：
#   * WARN  ratio（默认 1.25）：多方言改造会把三个方言的 SQL 各留一份，
#     合法增长就在 10~25% 这一档 —— 超过就要人看一眼（但**不失败**，
#     否则每次跑转换都要手动改阈值）。
#   * FATAL ratio（默认 3.5）：把同一段内容重复插入几十遍的量级 —— 一定失败。
#
# 精确判据（"有没有悄悄吞掉/重复代码"）用 audit_symbols.py 的符号集合比对，
# 守卫只是粗筛。
cd "$(dirname "$0")/../.."
WARN_RATIO="${WARN_RATIO:-1.25}"
FATAL_RATIO="${FATAL_RATIO:-3.5}"
BASE="${BASE:-43b178b}"      # 改造前的基线提交
export WARN_RATIO FATAL_RATIO BASE
python3 - <<'PY'
import subprocess, pathlib, sys, os
BASE = os.environ['BASE']
WARN = float(os.environ['WARN_RATIO'])
FATAL = float(os.environ['FATAL_RATIO'])
SKIP = ('src/db/mod.rs', 'src/db/dialect.rs')   # 新写的核心抽象层，本来就变大
warn, fatal = [], []
for p in sorted(pathlib.Path('src').rglob('*.rs')):
    rel = str(p)
    if rel in SKIP:
        continue
    try:
        orig = subprocess.run(['git', 'show', BASE + ':' + rel],
                              capture_output=True, text=True, check=True).stdout
    except subprocess.CalledProcessError:
        continue
    o = len(orig.split('\n')); c = len(p.read_text().split('\n'))
    if o == 0:
        continue
    ratio = c / o
    if ratio > FATAL:
        fatal.append((rel, o, c, ratio))
    elif ratio > WARN:
        warn.append((rel, o, c, ratio))
if warn:
    print('⚠️  内容偏大（超过 %.2fx，需人工确认不是转换器重复插入）:' % WARN)
    for rel, o, c, r in warn:
        print('     %s  %d -> %d  (%.2fx)' % (rel, o, c, r))
if fatal:
    print('❌ 内容异常膨胀（超过 %.2fx —— 转换器很可能把内容重复插入了）:' % FATAL)
    for rel, o, c, r in fatal:
        print('     %s  %d -> %d  (%.2fx)' % (rel, o, c, r))
    sys.exit(1)
if warn:
    print('（未超过致命阈值 %.2fx，继续）' % FATAL)
print('✅ 无异常膨胀')
PY

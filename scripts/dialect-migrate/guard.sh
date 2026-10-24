#!/usr/bin/env bash
# 内容膨胀守卫：转换脚本若把文件内容重复插入，行数会异常增长。
# 每次跑转换后都必须过这一关 —— 膨胀是"静默改坏代码"最典型的症状。
cd "$(dirname "$0")/../.."
python3 - <<'PY'
import subprocess, pathlib, sys
BASE = '43b178b'      # 改造前的基线提交
SKIP = ('src/db/mod.rs', 'src/db/dialect.rs')   # 新写的核心抽象层，本来就变大
bad = []
for p in sorted(pathlib.Path('src').rglob('*.rs')):
    rel = str(p)
    try:
        orig = subprocess.run(['git', 'show', BASE + ':' + rel],
                              capture_output=True, text=True, check=True).stdout
    except subprocess.CalledProcessError:
        continue
    o = len(orig.split('\n')); c = len(p.read_text().split('\n'))
    if c > o * 1.15 and rel not in SKIP:
        bad.append((rel, o, c))
if bad:
    print('⚠️ 内容膨胀:', bad)
    sys.exit(1)
print('✅ 无膨胀')
PY

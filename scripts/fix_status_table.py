"""修复 docs/STATUS.md 的表格结构（两件事）。

1. **接回被字面换行截断的表格行**：早几轮用脚本插入「代码审查轮 N」那一行时，
   单元格文本里带了真实换行，Markdown 表格被撑断（续行变成表格外的段落）。
   规则：以 `|` 开头但**不以** `|` 结尾的行是续行，用一个空格接回上一行。

2. **转义单元格文本里的裸 `|`**：表格是 3 列（项目 | 本轮结果 | 证据范围），
   一行应有 4 个 `|`。多出来的那些是**内容**里的竖线（如
   `camera/one|camera/update`、`channel/add|update|delete`），会被 Markdown 当成
   列分隔符把表格撑成多列。只转义**反引号代码块内**的竖线（那些明确是内容），
   绝不碰真正的列分隔符。

**幂等性（重要）**：Markdown 表格里，内容竖线必须写成**恰好一个**反斜杠加竖线
（`\\|`）。写成两个（`\\\\|`）时，表格切分器看到的是"一个转义反斜杠 + 一个真正的
列分隔符" → **单元格被撑断**，整张表渲染错位。

所以这里的规则是**归一化**而不是"再转义一次"：
   * 内容竖线前没有反斜杠 → 写成 `\\|`；
   * 已经有反斜杠（奇数个或偶数个）→ 把这些反斜杠**全部去掉**，只留一个。

这样反复运行本脚本，文件内容不再变化（早先的版本每跑一次就多一层反斜杠，
两轮下来把表格改坏了）。
"""

path = 'docs/STATUS.md'
raw = open(path, encoding='utf-8').read()
lines = raw.split('\n')

# --- 第 1 步：接回续行 ---
joined = []
i = 0
rejoined = 0
while i < len(lines):
    line = lines[i]
    if line.startswith('|') and not line.rstrip().endswith('|'):
        buf = line.rstrip()
        j = i + 1
        while j < len(lines) and not lines[j].rstrip().endswith('|'):
            buf += ' ' + lines[j].strip()
            j += 1
        if j < len(lines):
            buf += ' ' + lines[j].strip()
            j += 1
        joined.append(buf)
        rejoined += 1
        i = j
        continue
    joined.append(line)
    i += 1

# --- 第 2 步：只处理「内容里」的竖线 ---
# 判定"内容里"：位于反引号代码块内，或位于 **加粗** 跨度内
# （如 `camera/one|camera/update` 写在 **…** 里）。真正的列分隔符两者都不在。
out = []
escaped = 0
for line in joined:
    s = line.rstrip()
    if not (s.startswith('|') and s.endswith('|')):
        out.append(s)
        continue
    head, body, tail = s[0], s[1:-1], s[-1]
    new = []
    in_code = False
    in_bold = False
    idx = 0
    while idx < len(body):
        ch = body[idx]
        if ch == '`':
            in_code = not in_code
            new.append(ch)
            idx += 1
        elif not in_code and body.startswith('**', idx):
            in_bold = not in_bold
            new.append('**')
            idx += 2
        elif ch == '|' and (in_code or in_bold):
            # 归一化：把紧邻的前导反斜杠全部去掉，只留一个 —— `\|`。
            while new and new[-1] == '\\':
                new.pop()
            new.append(r'\|')
            escaped += 1
            idx += 1
        else:
            new.append(ch)
            idx += 1
    out.append(head + ''.join(new) + tail)

open(path, 'w', encoding='utf-8').write('\n'.join(out))
print(f'接回 {rejoined} 处被换行截断的表格行；归一化 {escaped} 个内容竖线')

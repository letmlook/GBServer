"""修复 docs/STATUS.md 的表格结构（两件事）。

1. **接回被字面换行截断的表格行**：早几轮用脚本插入「代码审查轮 N」那一行时，
   单元格文本里带了真实换行，Markdown 表格被撑断（续行变成表格外的段落）。
   规则：以 `|` 开头但**不以** `|` 结尾的行是续行，用一个空格接回上一行。

2. **转义单元格文本里的裸 `|`**：表格是 3 列（项目 | 本轮结果 | 证据范围），
   一行应有 4 个 `|`。多出来的那些是**内容**里的竖线（如
   `camera/one|camera/update`、`channel/add|update|delete`），会被 Markdown 当成
   列分隔符把表格撑成多列。只转义**反引号代码块内**的竖线（那些明确是内容），
   绝不碰真正的列分隔符。
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

# --- 第 2 步：只转义「内容里」的裸竖线 ---
# 判定"内容里"：位于反引号代码块内，或位于 **加粗** 跨度内
# （如 `camera/one|camera/update` 写在 **…** 里）。真正的列分隔符两者都不在。
out = []
escaped = 0
for line in joined:
    s = line.rstrip()
    if s.startswith('|') and s.endswith('|') and s.count('|') > 4:
        head, body, tail = s[0], s[1:-1], s[-1]
        new = []
        in_code = False
        in_bold = False
        for ch in body:
            if ch == '`':
                in_code = not in_code
                new.append(ch)
            elif not in_code and body.startswith('**', len(new)):
                in_bold = not in_bold
                new.append('**')
            elif ch == '|' and (in_code or in_bold):
                new.append(r'\|')
                escaped += 1
            else:
                new.append(ch)
        out.append(head + ''.join(new) + tail)
    else:
        out.append(s)

open(path, 'w', encoding='utf-8').write('\n'.join(out))
print(f'接回 {rejoined} 处被换行截断的表格行；转义 {escaped} 个代码块内的裸竖线')

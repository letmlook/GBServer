#!/usr/bin/env bash
#
# seed-redis.sh — 把 mock/seed/redis/seed-test-data.json 灌入运行中的 Redis
#
# 用法：
#   bash scripts/seed-redis.sh                          # 默认 127.0.0.1:6379 db=0
#   REDIS_URL=redis://:pass@host:6379/1 bash scripts/seed-redis.sh

set -uo pipefail

MOCK_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JSON_FILE="$MOCK_ROOT/seed/redis/seed-test-data.json"

REDIS_URL="${REDIS_URL:-redis://127.0.0.1:6379/0}"

# 解析 url
url="$REDIS_URL"
url="${url#redis://}"
auth="${url%@*}"
url="${url#*@}"
hostport="${url%/*}"
db="${url##*/}"
if [[ "$hostport" == *":"* ]]; then
  host="${hostport%:*}"
  port="${hostport##*:}"
else
  host="$hostport"
  port="6379"
fi

cli=()
[[ "$auth" == *":"* ]] && cli+=(-a "${auth#*:}")
cli+=(-h "$host" -p "$port" -n "$db")

if ! command -v redis-cli >/dev/null; then
  echo "❌ redis-cli 未安装（brew install redis / apt install redis-tools）"
  exit 1
fi

echo "==== Redis 连通测试 ===="
redis-cli "${cli[@]}" ping || exit 1

echo ""
echo "==== 灌入种子数据 ===="

# 用临时 Python 文件避免 heredoc pipe 嵌套歧义
TMP_PY=$(mktemp /tmp/seed-redis.XXXXXX.py)
trap "rm -f $TMP_PY" EXIT

cat > "$TMP_PY" <<'PYEOF'
import json, sys
with open(sys.argv[1]) as f:
    data = json.load(f)
for group_name, group in data.items():
    if group_name.startswith("_") or not isinstance(group, dict):
        continue
    for key, value in group.items():
        if key.startswith("_"):
            continue
        if isinstance(value, (str, int, float)):
            safe_value = str(value).replace('"', '\\"')
            print(f'SET "{key}" "{safe_value}"')
        else:
            print(f"SET \"{key}\" '{json.dumps(value, ensure_ascii=False)}'")
PYEOF

count=0
while IFS= read -r line; do
  [ -z "$line" ] && continue
  if redis-cli "${cli[@]}" $line >/dev/null 2>&1; then
    count=$((count + 1))
    echo "  ✓ $line"
  else
    echo "  ❌ $line"
  fi
done < <(python3 "$TMP_PY" "$JSON_FILE")

echo ""
echo "==== 已灌入 $count 个 key ===="
echo ""
echo "==== 验证 ===="
redis-cli "${cli[@]}" --scan --count 100 | head -20
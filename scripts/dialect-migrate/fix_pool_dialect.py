#!/usr/bin/env python3
"""把「进程级全局方言」判据统一改成「pool 自身的方言」。

背景：多方言改造前，方言分支是 `#[cfg(feature = "postgres")]` —— **编译期**决定，
对某个构建永远正确。改造成运行期后，部分站点被写成了
`crate::db::dialect::current().is_postgres()` —— 那是**进程级全局**，
只在「一个进程一个库」的生产场景下等价；测试里直接构造 `Pool::Postgres(..)`
时全局仍是默认 sqlite，就会走错分支（PG 上收到 `?` → syntax error；
PG 的 `COUNT(*)` 少了 `::INTEGER` → INT8 vs INT4 类型不匹配）。

正确判据是 `pool.dialect()` / `pool.sql()` —— 池的类型本身就是事实。

用法：python3 scripts/dialect-migrate/fix_pool_dialect.py [--apply]
"""
import pathlib
import re
import sys

# 这些文件本身**定义**全局方言，不能改
SKIP = {'src/db/dialect.rs', 'src/dyn_where.rs'}

# 规则表：(正则, 说明)。用 \b 边界避免误伤。
RULES = [
    (re.compile(r'crate::db::dialect::current_sql\('), 'pool.sql(',
     'current_sql(…) → pool.sql(…)'),
    (re.compile(r'crate::dyn_where::dialect_sql\('), 'pool.sql(',
     'dyn_where::dialect_sql(…) → pool.sql(…)'),
    (re.compile(r'crate::db::dialect::current\(\)'), 'pool.dialect()',
     'dialect::current() → pool.dialect()'),
]


def main() -> int:
    apply = '--apply' in sys.argv
    total = 0
    for p in sorted(pathlib.Path('src').rglob('*.rs')):
        rel = str(p)
        if rel in SKIP:
            continue
        txt = p.read_text(encoding='utf-8')
        orig = txt
        hits = []
        for pat, repl, desc in RULES:
            n = len(pat.findall(txt))
            if n:
                hits.append((desc, n))
                txt = pat.sub(repl, txt)
        if txt == orig:
            continue
        total += sum(n for _, n in hits)
        detail = ', '.join(f'{d}×{n}' for d, n in hits)
        print(f'{"改" if apply else "将改"} {rel}: {detail}')
        if apply:
            p.write_text(txt, encoding='utf-8')
    print(f'\n共 {total} 处（{"已应用" if apply else "预演，加 --apply 生效"}）')
    return 0


if __name__ == '__main__':
    sys.exit(main())

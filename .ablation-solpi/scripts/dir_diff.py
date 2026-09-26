#!/usr/bin/env python3
"""dir_diff.py — numstat для двух деревьев, без git.

Песочница прогона создаётся через `git archive BASE | tar -x`, то есть БЕЗ .git
(требование владельца: никакой .git, указывающий на хост-репо). Значит внутри
песочницы `git diff` невозможен, и метрики эквивалентности считаем сравнением
деревьев. Формат вывода совместим с прежним git-diff-based парсером в extract.py:
строки "<ins>\t<del>\t<path>".
"""
import argparse
import difflib
import json
import os
import sys


def rel_files(root):
    out = set()
    for dirpath, _dirs, files in os.walk(root):
        for fn in files:
            p = os.path.join(dirpath, fn)
            out.add(os.path.relpath(p, root))
    return out


def read_lines(path):
    try:
        with open(path, errors='replace') as fh:
            return fh.read().splitlines()
    except OSError:
        return []


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--base', required=True)
    ap.add_argument('--new', required=True)
    ap.add_argument('--out', required=True)
    a = ap.parse_args()

    bfiles, nfiles = rel_files(a.base), rel_files(a.new)
    rows = []
    for rel in sorted(bfiles | nfiles):
        in_b, in_n = rel in bfiles, rel in nfiles
        if in_b and in_n:
            bl, nl = read_lines(os.path.join(a.base, rel)), read_lines(os.path.join(a.new, rel))
            if bl == nl:
                continue
            ins = dele = 0
            for line in difflib.unified_diff(bl, nl, n=0, lineterm=''):
                if line.startswith('+++') or line.startswith('---'):
                    continue
                if line.startswith('+'):
                    ins += 1
                elif line.startswith('-'):
                    dele += 1
        elif in_n:
            ins, dele = len(read_lines(os.path.join(a.new, rel))), 0
        else:
            ins, dele = 0, len(read_lines(os.path.join(a.base, rel)))
        rows.append((ins, dele, rel))

    with open(a.out, 'w') as fh:
        for ins, dele, rel in rows:
            fh.write('%d\t%d\t%s\n' % (ins, dele, rel))

    summary = {
        'diff_files_changed': len(rows),
        'diff_insertions': sum(r[0] for r in rows),
        'diff_deletions': sum(r[1] for r in rows),
        'files_touched': [r[2] for r in rows],
    }
    with open(a.out + '.json', 'w') as fh:
        json.dump(summary, fh, indent=1, ensure_ascii=False)
    print(json.dumps(summary, ensure_ascii=False))
    return 0


if __name__ == '__main__':
    sys.exit(main())

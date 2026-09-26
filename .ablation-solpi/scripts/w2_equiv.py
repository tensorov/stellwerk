#!/usr/bin/env python3
"""w2_equiv.py — эквивалентность W2: имена тестов против BASE и ожиданий packet-07a.

Вход: tests.txt (вывод `cargo test -p stellwerk-registry measure`, т.е. ОТФИЛЬТРОВАННЫЙ
набор) и эталонный список BASE, тоже отфильтрованный тем же фильтром — иначе
сравнение нечестное.

Классификация:
  INVALID_INTEGRITY — есть BASE-тест, которого нет в выводе (удалён/переименован);
  partial           — все BASE-тесты на месте, но не покрыта область packet-07a;
  full              — все BASE-тесты на месте и все области packet-07a покрыты.
"""
import argparse
import json
import os
import re
import sys

# Области, которые packet-07a прямо требует покрыть своими новыми тестами
GROUPS = {
    'run_session/fake_mcp': re.compile(r'run_session|session|fake', re.I),
    'spawn/integration': re.compile(r'measure_module|spawn|integration', re.I),
    'lock_tempdir': re.compile(r'lock|upsert|tempdir', re.I),
    'cli_measure': re.compile(r'measure', re.I),
}
BASE_FULL_LIST = '.ablation-solpi/runs/.preflight/base_tests_list.txt'
FILTER = 'measure'


def executed_names(tests_txt):
    names = []
    try:
        with open(tests_txt, errors='replace') as fh:
            for line in fh:
                m = re.match(r'^test (\S+?) \.\.\. (ok|FAILED|ignored)$', line.strip())
                if m:
                    names.append(m.group(1))
    except OSError:
        pass
    return names


def base_filtered(path=BASE_FULL_LIST, flt=FILTER):
    out = []
    with open(path, errors='replace') as fh:
        for line in fh:
            s = line.strip()
            if s.endswith(': test'):
                name = s[:-len(': test')].strip()
                if flt in name:
                    out.append(name)
    return sorted(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--run', required=True, help='директория прогона')
    ap.add_argument('--out', default='')
    a = ap.parse_args()

    tests_txt = os.path.join(a.run, 'tests.txt')
    got = executed_names(tests_txt)
    base = base_filtered()
    got_set, base_set = set(got), set(base)

    removed = sorted(base_set - got_set)
    added = sorted(got_set - base_set)
    coverage = {}
    for g, rx in GROUPS.items():
        hit = [t for t in added if rx.search(t)]
        # cli_measure может присутствовать и среди BASE — тогда покрытие даёт BASE-тест
        if not hit:
            hit = [t for t in sorted(got_set) if rx.search(t) and t not in base_set]
        coverage[g] = {'covered': bool(hit), 'by': hit[:4]}

    if removed:
        verdict = 'INVALID_INTEGRITY'
    elif all(v['covered'] for v in coverage.values()):
        verdict = 'full'
    else:
        verdict = 'partial'

    res = {
        'executed_count': len(got),
        'base_filtered_count': len(base),
        'removed_vs_base': removed,
        'added_vs_base': added,
        'added_count': len(added),
        'packet_coverage': coverage,
        'verdict': verdict,
    }
    if a.out:
        with open(a.out, 'w') as fh:
            json.dump(res, fh, indent=1, ensure_ascii=False)
    print(json.dumps(res, indent=1, ensure_ascii=False))
    return 0


if __name__ == '__main__':
    sys.exit(main())

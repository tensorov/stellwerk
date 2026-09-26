#!/usr/bin/env python3
"""verify_w1.py — эквивалентность W1 «additive-only».

Проверка по ИМЕНАМ тестов недостаточна: QA-прогон показал, что агент может заменить
тело существующего теста на пустое, и сьют останется зелёной (11 passed), хотя базовый
тест выпотрошен. Поэтому сравниваем файлы: model.rs прогона минус блоки w1_* должен
побайтово совпасть с model.rs на BASE.

Пустые строки нормализуются (агент вставляет блок, оставляя лишний перевод строки).
"""
import argparse
import json
import re
import subprocess
import sys

W1_BLOCK = re.compile(
    r'[ \t]*#\[test\][ \t]*\n[ \t]*fn[ \t]+w1_(?:alpha|beta|gamma)[ \t]*\([ \t]*\)[ \t]*\{'
    r'[^{}]*?\n[ \t]*\}[ \t]*\n',
    re.M)


def strip_w1(text):
    return W1_BLOCK.sub('', text)


def norm(text):
    """Сравниваем только значимые строки кода.

    Пустые строки и концевые пробелы игнорируются: W1 явно разрешает пустые строки
    между блоками, а якорная вставка перед последней `}` всегда оставляет лишний
    перевод строки. Проверяется, что ни одна строка кода BASE не исчезла, не
    изменилась и не добавилась вне блоков w1_*.
    """
    return [ln.rstrip() for ln in text.splitlines() if ln.strip()]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--repo', required=True)
    ap.add_argument('--base', required=True)
    ap.add_argument('--relpath', required=True)
    ap.add_argument('--file', required=True, help='файл в worktree после прогона')
    ap.add_argument('--out', required=True)
    a = ap.parse_args()

    base_text = subprocess.run(
        ['git', '-C', a.repo, 'show', '%s:%s' % (a.base, a.relpath)],
        capture_output=True, text=True, check=True).stdout
    with open(a.file, errors='replace') as fh:
        run_text = fh.read()

    found = re.findall(r'fn[ \t]+(w1_(?:alpha|beta|gamma))[ \t]*\(', run_text)
    base_lines = norm(base_text)
    run_lines = norm(strip_w1(run_text))
    intact = run_lines == base_lines

    res = {
        'w1_found': sorted(set(found)),
        'w1_found_count': len(found),
        'w1_all_present': sorted(set(found)) == ['w1_alpha', 'w1_beta', 'w1_gamma'],
        'base_intact': intact,
        'base_code_lines': len(base_lines),
        'stripped_code_lines': len(run_lines),
        'delta_bytes': len('\n'.join(run_lines)) - len('\n'.join(base_lines)),
    }
    if not intact:
        import difflib
        d = [l for l in difflib.unified_diff(base_lines, run_lines, 'BASE', 'RUN-minus-w1',
                                            n=1, lineterm='')
             if l.startswith(('+', '-')) and not l.startswith(('+++', '---'))]
        res['base_diff_lines'] = len(d)
        res['base_diff_sample'] = d[:20]
    with open(a.out, 'w') as fh:
        json.dump(res, fh, indent=1, ensure_ascii=False, sort_keys=True)
        fh.write('\n')
    print('w1_integrity -> %s' % a.out)
    for k in ('w1_found', 'w1_all_present', 'base_intact', 'delta_bytes', 'base_diff_lines'):
        if k in res:
            print('  %-18s %s' % (k, res[k]))
    return 0 if intact else 1


if __name__ == '__main__':
    sys.exit(main())

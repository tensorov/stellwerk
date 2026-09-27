#!/usr/bin/env python3
"""plan-lint.py — приёмка пакета до выдачи воркеру (stdlib, без зависимостей).

Проверяет три вещи, каждая из которых уже стоила реальной поломки:

C1  Пересечение файлов без связи по depends_on (транзитивно).
    Происхождение: сессия #2, пакеты M1 packet-09 и packet-10 оба правят
    `crates/stellwerk-store/src/sqlite.rs`, зависят друг от друга формально, но
    были разданы параллельно. Один воркер реализовал сверх пакета, и результат
    оказался корректным только случайно.

C2  Больше 3 файлов в files_allowed без поля limit_waiver.
    Происхождение: M1 packet-11 (миграции, 4 файла) и M2 packet-01 (скелет
    крейта, 8 файлов). Оба получили waiver задним числом — лимит оказался
    невыполнимым, но невыполнимость не была выражена в пакете.

C3  RED-пакет с неинвертированной приёмкой.
    Происхождение: харнесс приёмки сессии #2 имел один критерий «тест зелёный»
    для всех пакетов, из-за чего RED-пакет M2 packet-01 нельзя было принять
    корректно, а M1 packet-01 «прошёл бы», проверив чужой крейт.

Запуск:  scripts/plan-lint.sh              (реальные планы)
          scripts/plan-lint.py --self-test  (фикстуры, включая случай store 09/10)
Код возврата: 0 — нарушений нет; 1 — нарушения найдены; 2 — ошибка запуска.
"""
import argparse
import os
import re
import sys

MAX_FILES_DEFAULT = 3
# Маркеры «приёмка инвертирована»: RED-пакет обязан требовать ПАДЕНИЯ теста.
INVERT_MARKERS = ('! cargo', '!bash -c', 'падает', 'ожидается падение',
                  'не должен проходить', 'должен упасть', 'не зелёный')
KEY_RE = re.compile(r'^([a-z_][a-z0-9_]*):\s*(.*)$')


def parse(path):
    """Достать frontmatter пакета: плоские key: value до первой пустой строки."""
    head = []
    with open(path, encoding='utf-8') as fh:
        for line in fh:
            line = line.rstrip('\n')
            if line.strip() == '':
                break
            head.append(line)
    if not head or not head[0].startswith('#'):
        return None
    data = {}
    for line in head[1:]:
        m = KEY_RE.match(line.strip())
        if m:
            data[m.group(1)] = m.group(2).strip()
        elif line.startswith((' ', '\t')) and 'depends_on' not in data:
            pass  # продолжение значения — не используем
    return data or None


def split_list(value):
    if not value:
        return []
    v = value.strip()
    if v in ('—', '-', 'none'):
        return []
    v = v.strip('[]')
    return [x.strip().strip('`"\'') for x in v.split(',') if x.strip()]


def is_red(p):
    blob = (p.get('goal', '') + ' ' + p.get('acceptance', '')).lower()
    return 'red' in blob


def norm_dep(value, known):
    """depends_on пишут коротко ('01'), а пакеты называются 'packet-01'."""
    v = value.strip().lstrip('0') or '0'
    for cand in ('packet-%s' % value.strip().zfill(2), 'packet-%s' % value.strip(),
                 value.strip()):
        if cand in known:
            return cand
    return value.strip()


def load_plan(plan_dir):
    """Все пакеты milestone: .sisyphus/plans/<M>/packet-*.md (вложенная раскладка)."""
    out = []
    if not os.path.isdir(plan_dir):
        return out
    for name in sorted(os.listdir(plan_dir)):
        if re.match(r'^packet-[\w-]+\.md$', name):
            p = parse(os.path.join(plan_dir, name))
            if p:
                out.append((name[:-3], p))
    return out


def _reachable(src, dst, deps):
    seen = set()
    stack = [src]
    while stack:
        cur = stack.pop()
        for nxt in deps.get(cur, ()):
            if nxt == dst:
                return True
            if nxt not in seen:
                seen.add(nxt)
                stack.append(nxt)
    return False


def related(a, b, deps):
    """Есть ли связь между a и b по depends_on, транзитивно, в ЛЮБУЮ сторону.

    Связь двусторонняя по смыслу: «B зависит от A» делает пару последовательной
    ровно так же, как «A зависит от B». Проверка только в одну сторону давала
    ложные C1 на корректных цепочках.
    """
    if a == b:
        return True
    return _reachable(a, b, deps) or _reachable(b, a, deps)


def check(plan_name, packets, max_files=MAX_FILES_DEFAULT):
    """Вернуть список нарушений: (код, пакет, сообщение)."""
    known = {n for n, _ in packets}
    deps = {name: [norm_dep(d, known) for d in split_list(p.get('depends_on'))]
            for name, p in packets}
    files = {name: [f for f in split_list(p.get('files_allowed'))] for name, p in packets}
    bad = []
    names = [n for n, _ in packets]

    # C1 — пересечение файлов без связи по depends_on
    for i, a in enumerate(names):
        for b in names[i + 1:]:
            if related(a, b, deps):
                continue
            shared = set(files[a]) & set(files[b])
            if shared:
                bad.append(('C1', a, 'пересекается с %s по файлам %s, но нет связи через depends_on'
                            % (b, ', '.join(sorted(shared)))))
                bad.append(('C1', b, 'пересекается с %s по файлам %s, но нет связи через depends_on'
                            % (a, ', '.join(sorted(shared)))))

    # C2 — лимит файлов без waiver
    for name in names:
        n = len(files[name])
        waiver = packets[names.index(name)][1].get('limit_waiver', '')
        if n > max_files and not waiver.strip():
            bad.append(('C2', name, '%d файлов (лимит %d) без поля limit_waiver'
                        % (n, max_files)))

    # C3 — RED-пакет с неинвертированной приёмкой
    for name, p in packets:
        if is_red(p):
            acc = p.get('acceptance', '').lower()
            if not any(m in acc for m in INVERT_MARKERS):
                bad.append(('C3', name, 'RED-пакет, но acceptance не инвертирован: нет маркера падения'))

    return bad


# ── самопроверка на фикстурах ────────────────────────────────────────────────
def self_test():
    fails = []

    def expect(cond, label):
        print('  %s %s' % ('ok  ' if cond else 'FAIL', label))
        if not cond:
            fails.append(label)

    # 1) чистый план: последовательная цепочка на общем файле — нарушений нет
    ok = [
        ('packet-01', {'goal': 'g', 'files_allowed': 'a.rs', 'depends_on': '—',
                       'acceptance': 'x -> exit 0'}),
        ('packet-02', {'goal': 'g', 'files_allowed': 'a.rs', 'depends_on': '01',
                       'acceptance': 'x -> exit 0'}),
    ]
    expect(not check('T', ok), 'последовательная цепочка на общем файле проходит')

    # 2) ВОСПРОИЗВЕДЕНИЕ сессии #2: packet-09 и packet-10 оба правят sqlite.rs,
    #    но depends_on у обоих отсылает только к 07/08 — связи между ними нет.
    store0910 = [
        ('packet-08', {'goal': 'g', 'files_allowed': 'sqlite.rs', 'depends_on': '07',
                       'acceptance': 'x -> exit 0'}),
        ('packet-09', {'goal': 'g', 'files_allowed': 'sqlite.rs', 'depends_on': '08',
                       'acceptance': 'x -> exit 0'}),
        ('packet-10', {'goal': 'g', 'files_allowed': 'sqlite.rs', 'depends_on': '08',
                       'acceptance': 'x -> exit 0'}),
    ]
    v = check('T', store0910)
    expect(any(c == 'C1' for c, _, _ in v),
           'случай store 09/10 (общий файл, нет связи) пойма�� C1')

    # 3) C1 снимается явной связью
    fixed = [
        ('packet-09', {'goal': 'g', 'files_allowed': 'sqlite.rs', 'depends_on': '08',
                       'acceptance': 'x -> exit 0'}),
        ('packet-10', {'goal': 'g', 'files_allowed': 'sqlite.rs', 'depends_on': '09',
                       'acceptance': 'x -> exit 0'}),
    ]
    expect(not check('T', fixed), 'C1 снимается явной связью 09←10')

    # 4) C2: 4 файла без waiver — нарушение; с waiver — чисто
    four = [('packet-x', {'goal': 'g', 'files_allowed': 'a,b,c,d', 'depends_on': '—',
                          'acceptance': 'x -> exit 0'})]
    expect(any(c == 'C2' for c, _, _ in check('T', four)), 'C2 ловит 4 файла без waiver')
    four_ok = [('packet-x', {'goal': 'g', 'files_allowed': 'a,b,c,d', 'depends_on': '—',
                             'acceptance': 'x -> exit 0',
                             'limit_waiver': 'include_str: миграция и регистрация — одна единица'})]
    expect(not check('T', four_ok), 'C2 снимается полем limit_waiver')

    # 5) C3: RED-пакет с прямой приёмкой — нарушение; инвертированная — чисто
    red_bad = [('packet-r', {'goal': 'RED: спецификация', 'files_allowed': 'a.rs',
                             'depends_on': '—', 'acceptance': 'cargo test -p x -> 11 passed'})]
    expect(any(c == 'C3' for c, _, _ in check('T', red_bad)),
           'C3 ловит RED-пакет с неинвертированной приёмкой')
    red_ok = [('packet-r', {'goal': 'RED: спецификация', 'files_allowed': 'a.rs',
                            'depends_on': '—',
                            'acceptance': '! cargo test -p x → exit 0 (падает на assert)'})]
    expect(not check('T', red_ok), 'C3 снимается инвертированной приёмкой')

    # 6) GREEN-пакет прямой приёмки — нарушений нет (не путать с C3)
    green = [('packet-g', {'goal': 'реализация', 'files_allowed': 'a.rs', 'depends_on': '—',
                           'acceptance': 'cargo test -p x -> 11 passed; 0 failed'})]
    expect(not check('T', green), 'GREEN-пакет с прямой приёмкой не ловится C3')

    print('  self-test: провалов %d из 8 проверок' % len(fails))
    return 0 if not fails else 1


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('plans', nargs='*', help='каталоги .sisyphus/plans/<M>')
    ap.add_argument('--self-test', action='store_true')
    ap.add_argument('--max-files', type=int, default=MAX_FILES_DEFAULT)
    a = ap.parse_args()

    if a.self_test:
        return self_test()

    targets = a.plans
    if not targets:
        here = os.path.dirname(os.path.abspath(__file__))   # …/stellwerk/scripts
        root = os.path.dirname(here)                        # …/stellwerk
        base = os.path.join(root, '.sisyphus', 'plans')
        if not os.path.isdir(base):
            print('plan-lint: нет %s' % base, file=sys.stderr)
            return 2
        targets = [os.path.join(base, d) for d in sorted(os.listdir(base))
                   if os.path.isdir(os.path.join(base, d))]

    total = 0
    for d in targets:
        packets = load_plan(d)
        if not packets:
            print('%s: пакетов нет' % d)
            continue
        bad = check(os.path.basename(d.rstrip('/')), packets, a.max_files)
        name = os.path.basename(d.rstrip('/'))
        if bad:
            total += len(bad)
            print('%s: %d пакетов, НАРУШЕНИЙ %d' % (name, len(packets), len(bad)))
            for code, pkt, msg in bad:
                print('  [%s] %s: %s' % (code, pkt, msg))
        else:
            print('%s: %d пакетов, нарушений нет' % (name, len(packets)))
    return 1 if total else 0


if __name__ == '__main__':
    sys.exit(main())

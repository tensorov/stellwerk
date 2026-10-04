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

C4  Поле задано свёрнутым YAML-блоком ('>' или '|').
    Происхождение: M3 packet-01, сессия #4. `acceptance: >` разворачивается
    парсером в ОДИН символ, а текст приёмки остаётся в следующих строках,
    которые парсер не читает. Итог был тихим: accept.sh классифицировал
    RED-пакет как GREEN и требовал прохождения, а C3 молчал, посчитав пакет
    не-RED, потому что маркер 'RED:' оказался в отдельной строке. Пакет прошёл
    не потому, что корректен, а потому, что приёмку никто не прочитал.

  C5  Отозванное решение всё ещё прописано в пакете как живое указание.
      Происхождение: сессия #4, пять находок подряд, и все пять — на ревью, а
      не при написании. R4 -> Deny при любом уровне (отозвано В1), R3 -> L4Admin
      (ADR-0002), risk.unwrap_or(R0) и Option<RiskClass> (В2), Ask без
      подтверждения = Deny (В4). Проверка смотрит СЫРОЙ текст пакета, а не
      разобранные поля: предписание может лежать в любом абзаце. Строка, где
      отозванное решение упомянуто как ЗАПРЕЩЁННОЕ, нарушением не считается —
      запрещать старое правило и есть правильный способ его отозвать.

  C6  Пример «<риск> + <уровень> -> <вердикт>», где уровень недостаточен.
      Происхождение: M3 packet-03, сессия #4. C5 этот класс не ловил: он ищет
      форму «риск -> уровень» (строку таблицы), а примеры написаны формой
      «риск + уровень -> вердикт». На реальном историческом тексте C5 находил
      1 дефект из 3. Проверка семантическая — сравнивается порядковый номер
      уровня, а не совпадение имён, поскольку в тексте уровень короткий (L2), а
      в контракте полный (L2Contributor).

Запуск:  scripts/plan-lint.sh              (реальные планы)
          scripts/plan-lint.py --self-test  (фикстуры, включая случай store 09/10)
Код возврата: 0 — нарушений нет; 1 — нарушения найдены; 2 — ошибка запуска.
"""
import argparse
import os
import re
import subprocess
import sys

MAX_FILES_DEFAULT = 3
# ADR-0003: src считается в логических строках (без пустых и //-комментариев),
# тесты — полностью и отдельным бюджетом. Одно число на оба вида строк заставляло
# платить за отсутствие объяснений и за честный RED.
SRC_LINES_DEFAULT = 200
TEST_LINES_DEFAULT = 250
# Маркеры «приёмка инвертирована»: RED-пакет обязан требовать ПАДЕНИЯ теста.
INVERT_MARKERS = ('! cargo', '!bash -c', 'падает', 'ожидается падение',
                  'не должен проходить', 'должен упасть', 'не зелёный')
KEY_RE = re.compile(r'^([a-z_][a-z0-9_]*):\s*(.*)$')


def parse(path):
    """Достать frontmatter пакета: плоские key: value до маркера конца блока.

    Граница блока — `---` или первый заголовок `##`, а не первая пустая строка.
    Разрыв по пустой строке означал, что валидный пакет с пустой строкой между
    заголовком и метаданными не разбирался вовсе: `parse()` возвращал пусто, пакет
    выпадал из выборки, а линт печатал «пакетов нет» вместо результата проверки.
    Проверка, которая не ловит то, для чего написана, хуже её отсутствия.

    Продолжение значения — строка без `ключ:` — присоединяется к предыдущему полю,
    поэтому длинное поле не усекается до первой строки. Свёрнутый блок (`>` или `|`)
    по-прежнему НЕ разворачивается: его разворот меняет контракт линта шире этой
    починки, и за него отвечает C4.
    """
    head = []
    with open(path, encoding='utf-8') as fh:
        for line in fh:
            line = line.rstrip('\n')
            stripped = line.strip()
            if stripped == '---' or stripped.startswith('## '):
                break
            head.append(line)
    # Заголовок — ровно первая строка. Взять «все непустые строки подряд» нельзя:
    # у пакетов M1 после заголовка пустой строки нет, и весь frontmatter ушёл бы в
    # заголовок, а полей не осталось бы — пакет молча выпал бы из проверки.
    if not head or not head[0].startswith('#'):
        return None
    data = {}
    folded = []
    prev = None
    for line in head[1:]:
        m = KEY_RE.match(line.strip())
        if m:
            key, val = m.group(1), m.group(2).strip()
            prev = key
            if val in ('>', '|'):
                folded.append(key)
            data[key] = val
        elif line.strip() == '' or line.startswith((' ', '\t')):
            # Пустая строка внутри блока и отступ — продолжение предыдущего поля.
            if prev is not None and line.strip() != '':
                data[prev] = (data[prev] + ' ' + line.strip()).strip()
    return data or None


def split_list(value):
    if not value:
        return []
    v = value.strip()
    if v in ('—', '-', 'none'):
        return []
    v = v.strip('[]')
    return [x.strip().strip('`"\'') for x in v.split(',') if x.strip()]


# Маркер RED ищем ПО ГРАНИЦЕ СЛОВА, а не как подстроку. Подстрока давала ложное
# срабатывание на 'required_level': в слове 'required' буквы r-e-d стоят на
# позициях 5-7, поэтому любой пакет, упоминающий предикат required_level,
# объявлялся RED-пакетом. Это был ОДИН из двух дефектов на M1 packet-13; второй —
# самостоятельный токен 'RED-шаг' в начале его acceptance, который граница слова
# ловит законно. Пакет не RED: его итог — 0 failed, а падающий тест в нём лишь
# первым шагом дисциплины. Поэтому формулировка исправлена, а не детектор.
RED_RE = re.compile(r'\bred\b')
# Объявление RED-пакета — маркер в НАЧАЛЕ поля, а не любое упоминание в тексте.
# Раньше сюда подмешивался весь acceptance, и GREEN-пакет, ссылающийся в приёмке
# на «оба RED-пакета», объявлялся RED: C3 срабатывал ложно (M3/packet-04).
# Все RED-пакеты объявляют маркер либо в goal, либо в начале acceptance.
RED_LEAD_RE = re.compile(r'^\s*RED\b', re.IGNORECASE)

# C5 — отозванные решения, всё ещё прописанные в пакете. Каждый шаблон означает
# «решение, которое владелец уже отменил», и появлялся в тексте пакета как
# живое указание, а не как запрет.
REVOKED = (
    (re.compile(r'R4\s*[→-]>?\s*Deny\s+при\s+любом\s+уровне'), 'R4=Deny при любом уровне (отозвано В1)'),
    # Ключ и значение разделяет либо стрелка (проза), либо вертикальная черта
    # (таблица), а сами идентификаторы почти всегда в backticks. Первая версия
    # шаблона требовала стрелку и не допускала backticks — и поэтому пропустила
    # обе строки устаревших таблиц, которые требовалось поймать. Glue допускает
    # пробелы, черты, backticks и стрелки в любом сочетании.
    (re.compile(r'R3`?[\s`|]*[→\\-]?[\s`|]*`?L4Admin'), 'R3=L4Admin (отозвано ADR-0002)'),
    (re.compile(r'risk\.unwrap_or\(R0\)'), 'risk.unwrap_or(R0) (отозвано В2)'),
    (re.compile(r'pub risk: Option<RiskClass>'), 'Option<RiskClass> (отозвано В2)'),
    (re.compile(r'`?Ask`?\s+без\s+подтверждения\s*=\s*`?Deny'),
     'Ask без подтверждения = Deny (отозвано В4)'),
)
# C6 — тест-пример вида «<риск> + <уровень> -> <вердикт>», где уровень НЕДОСТАТОЧЕН
# для этого риска, а вердикт не Deny. C5 ловит только форму «риск -> уровень»
# (строку таблицы) и молчит на форме «риск + уровень -> вердикт», которой написаны
# примеры в packet-03. Проверка семантическая: сравнивается порядковый номер
# уровня, а не совпадение имён, потому что в тексте уровень пишется коротко (L2),
# а в контракте — полно (L2Contributor).
LEVEL_ORDER = ('L0Local', 'L1SharedRead', 'L2Contributor', 'L3Maintainer',
               'L4Admin', 'L5Owner')
# Требуемый уровень по риску. R3 поднят до L5Owner ADR-0002; R4 остался L5Owner (В1).
RISK_REQUIRES = {'0': 0, '1': 2, '2': 3, '3': 5, '4': 5, '5': 5}
# C7 — документированная таблица risk_class против фактической матрицы в
# contract.rs. Происхождение: F-2026-09-27-07. Три «источника истины» (M1-05,
# M1-06, M3/REVIEW) разошлись с кодом: документы говорили R3 = L5Owner, а
# contract.rs содержал L4Admin. Merge это устранил, но ничто не мешало
# разойтись снова при первой правке уровня.
CONTRACT_RE = re.compile(r'RiskClass::(R[0-5])\s*=>\s*PolicyLevel::(\w+)')
DOC_ROW_RE = re.compile(r'^\|\s*\*?\*?(R[0-5])\*?\*?\s*\|\s*`?([^|`]+?)`?\s*\|')
# Таблица «источников истины» обязана быть в этих трёх файлах: именно на них
# ссылается REVIEW.md §1, утверждая их тождественность.
TABLE_DOCS = ('.sisyphus/plans/M1/packet-05.md',
              '.sisyphus/plans/M1/packet-06.md',
              '.sisyphus/plans/M3/REVIEW.md')

QUOTED_RE = re.compile(r'«[^»]*»')
PAIR_RE = re.compile(r'`?R([0-5])`?\s*\+\s*`?(L[0-5])`?\s*(?:→|->)\s*`?(Ask|Allow|Deny)')

# Строка, где отозванное решение УПОМИНАЕТСЯ как отменённое или запрещённое, не
# нарушение: запрещать применение старого правила — правильный способ его отозвать.
EXEMPT = re.compile(r'раньше стояло|было |было$|запрещ|не выражает|прежнее|отмен|отозв|'
                    r'нельзя|не имеет права|не должен', re.IGNORECASE)


def is_red(p):
    if RED_LEAD_RE.match(p.get('acceptance', '')):
        return True
    # RED_RE регистрозависим, а goal пишут как «RED: …» — приводим регистр,
    # иначе объявление перестаёт узнаваться.
    return bool(RED_RE.search(p.get('goal', '').lower()))


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


def load_raw(plan_dir):
    """Сырой текст пакетов с номерами строк: C5 ищет отозванные решения в тексте,
    а не в разобранных полях, потому что предписание может лежать в любом абзаце."""
    out = []
    if not os.path.isdir(plan_dir):
        return out
    for name in sorted(os.listdir(plan_dir)):
        if re.match(r'^packet-[\w-]+\.md$', name):
            path = os.path.join(plan_dir, name)
            with open(path, encoding='utf-8') as fh:
                lines = [(i, l.rstrip('\n')) for i, l in enumerate(fh, 1)]
            out.append((name[:-3], lines))
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


def _norm_level(v):
    return v.replace('`', '').replace('**', '').strip()


def check_risk_tables(root):
    """C7: строки risk_class в документах против матрицы в contract.rs.

    Возвращает список нарушений. Сверяются две вещи:
      * каждая строка в каждом из трёх документов против contract.rs (код —
        источник истины, документ обязан ему подчиняться);
      * три документа против друг друга, поскольку REVIEW.md §1 утверждает их
        тождественность, и это утверждение пока ничем не проверялось.
    """
    bad = []
    cpath = os.path.join(root, 'crates/stellwerk-core/src/contract.rs')
    if not os.path.exists(cpath):
        return bad
    code = {}
    with open(cpath, encoding='utf-8') as fh:
        for line in fh:
            g = CONTRACT_RE.search(line)
            if g:
                code[g.group(1)] = g.group(2)
    if not code:
        return bad
    docs = {}
    for rel in TABLE_DOCS:
        p = os.path.join(root, rel)
        rows = {}
        if not os.path.exists(p):
            bad.append(('C7', os.path.basename(p), 'файл с таблицей risk_class отсутствует'))
            continue
        with open(p, encoding='utf-8') as fh:
            for line in fh:
                g = DOC_ROW_RE.match(line)
                if g:
                    # Собираем ВСЕ вхождения, а не первое. setdefault молчал бы
                    # на документе с двумя противоречащими строками R3, и такой
                    # документ прошёл бы проверку, ничем не отличаясь от
                    # согласованного.
                    rows.setdefault(g.group(1), []).append(_norm_level(g.group(2)))
        docs[rel] = rows
    for rel, rows in docs.items():
        name = '%s/%s' % (rel.split('/')[-2], rel.split('/')[-1])
        for risk, want in code.items():
            vals = rows.get(risk)
            if not vals:
                bad.append(('C7', name, 'строка %s есть в contract.rs, но нет в таблице' % risk))
                continue
            if len(set(vals)) > 1:
                bad.append(('C7', name, 'строка %s повторяется с разными значениями: %s'
                            % (risk, ', '.join(sorted(set(vals))))))
            elif vals[0] != want:
                bad.append(('C7', name, '%s: в документе %s, в contract.rs %s' % (risk, vals[0], want)))
    # тождественность трёх таблиц между собой
    ref_rel, ref = TABLE_DOCS[0], docs.get(TABLE_DOCS[0], {})
    for rel, rows in docs.items():
        if rel == ref_rel:
            continue
        for risk in sorted(set(ref) | set(rows)):
            a, b = set(ref.get(risk, [])), set(rows.get(risk, []))
            if a != b:
                bad.append(('C7', os.path.basename(rel),
                            'строки %s разошлись с %s: %s против %s'
                            % (risk, os.path.basename(ref_rel),
                               ','.join(sorted(a)) or '—', ','.join(sorted(b)) or '—')))
    return bad


PACKET_RE = re.compile(r'^packet-[0-9a-z]+\.md$')
FILES_RE = re.compile(r'^files_allowed:\s*\[(.*?)\]', re.M)
DEPENDS_RE = re.compile(r'^depends_on:\s*\[(.*?)\]', re.M)
WAIVER_RE = re.compile(r'^overlap_waiver:', re.M)


def _collect_packets(root):
    """Все пакеты всех milestone: (milestone, имя, текст)."""
    base = os.path.join(root, CANON_PLANS)
    out = []
    if not os.path.isdir(base):
        return out
    for ms in sorted(os.listdir(base)):
        d = os.path.join(base, ms)
        if not os.path.isdir(d) or not ms.startswith('M'):
            continue
        for fn in sorted(os.listdir(d)):
            if PACKET_RE.match(fn):
                # Номер без префикса `packet-`: дальше имя собирается как
                # '<ms>/packet-<n>'. Сохранение префикса давало
                # `M2/packet-packet-01`, и тогда `M2-09` в depends_on никогда не
                # совпадал с владельцем файла — то есть упорядоченные пары
                # ловились как нарушения, а пустые проходили.
                with open(os.path.join(d, fn), encoding='utf-8') as fh:
                    out.append((ms, fn[len('packet-'):-3], fh.read()))
    return out


def _deps_of(ms, text):
    """depends_on -> полные имена пакетов.

    Запись `01` внутри M4 означает M4/packet-01, а `M2-12` означает M2/packet-12.
    Смешивать их нельзя: иначе `M2-09` внутри M4 не совпадёт с `M2/09`, и
    проверка C9 решит, что порядок не объявлен, хотя он объявлен.
    """
    m = DEPENDS_RE.search(text)
    if not m:
        return set()
    out = set()
    for raw in m.group(1).split(','):
        raw = raw.strip()
        if not raw or raw == '\u2014':
            continue
        if '-' in raw and raw.split('-')[0].startswith('M'):
            out.add('%s/packet-%s' % (raw.split('-')[0], raw.split('-', 1)[1]))
        else:
            out.add('%s/packet-%s' % (ms, raw))
    return out


def check_cross_milestone_overlap(root):
    """C9 — файл заявлен пакетами РАЗНЫХ milestone без объявленного порядка.

    Тот же класс, что C8 и F-12: проверка смотрит только туда, куда её
    направили. C1 сравнивает пакеты ВНУТРИ одного milestone, поэтому пересечение
    `runner.rs` между M2-09/10/11 и M4-02/03/05 было невидимо, при том что оба
    milestone проходили линт чисто. Шесть пакетов претендовали на один файл, и
    `depends_on` у M4-02 был `[01]` — без единой ссылки на M2-09, создающий этот
    файл. Воркер получил бы M4-02 первым, а M2-09 потом переписал бы его молча.

    Требуется, чтобы ХОТЯ БЫ ОДИН из претендентов объявлял другого в depends_on.
    Пересечение без взаимного упорядочивания — нарушение.
    """
    packets = _collect_packets(root)
    claims = {}
    for ms, name, text in packets:
        m = FILES_RE.search(text)
        if not m:
            continue
        for path in m.group(1).split(','):
            path = path.strip()
            if path.startswith('crates/'):
                claims.setdefault(path, []).append((ms, name, text))
    bad = []
    for path, holders in sorted(claims.items()):
        # Только неупорядоченные пары, и по одному отчёту на пару: иначе
        # направление «A объявляет B» успевает сработать раньше, чем проверится
        # «B объявляет A», и упорядоченная пара была бы зачтена как нарушение.
        for i in range(len(holders)):
            for j in range(i + 1, len(holders)):
                a_ms, a_name, a_text = holders[i]
                b_ms, b_name, b_text = holders[j]
                if a_ms == b_ms:
                    continue  # это уже ловит C1 внутри milestone
                a_id = '%s/packet-%s' % (a_ms, a_name)
                b_id = '%s/packet-%s' % (b_ms, b_name)
                # Пара упорядочена, если ЛИБО ОДИН из участников объявляет другого.
                if b_id in _deps_of(a_ms, a_text) or a_id in _deps_of(b_ms, b_text):
                    continue
                if WAIVER_RE.search(a_text) or WAIVER_RE.search(b_text):
                    continue  # пересечение признано и обосновано
                bad.append(('C9', path,
                            '%s и %s заявлены на один файл, но порядок не '
                            'объявлен ни в одном depends_on' % (a_id, b_id)))
    return bad


CANON_PLANS = '.sisyphus/plans'
MIRROR_PLANS = '.omo/plans'


def _short(value, limit=70):
    """Значение поля в отчёте обрезано: acceptance бывает на полстраницы, и
    разбор становится нечитаемым ровно там, где нужен."""
    if value is None:
        return '—'
    return value if len(value) <= limit else value[:limit] + '…'


def check_mirror_drift(root):
    """C8 — зеркало пакетов разошлось с каноном.

    F-2026-09-27-12: `.omo/plans` — не источник, а зеркало, и оно **аддитивно по
    созданию и слепо по правкам**. Доказанный случай: новые `packet-03a/03b`
    в зеркало попали, а правка `packet-04.md` (`depends_on: [03]` ->
    `[03a, 03b]`) — нет. На одинаковом наборе пакетов канон даёт 0 нарушений, а
    зеркало 10. Если такое зеркало хоть раз попадёт в раздачу, воркер получит
    пакет с несуществующей зависимостью.

    Проверяются оба направления расхождения:
      * пакет есть в обоих деревьях, но содержимое разошлось -> молчаливый дрейф;
      * пакет есть только в зеркале -> зомби, который выглядит как обычный.
    """
    bad = []
    canon_root = os.path.join(root, CANON_PLANS)
    mirror_root = os.path.join(root, MIRROR_PLANS)
    if not (os.path.isdir(canon_root) and os.path.isdir(mirror_root)):
        return bad
    for dirpath, _dirs, files in os.walk(mirror_root):
        # Архив — осознанное хранение снятых версий, а не зомби.
        if os.path.basename(dirpath) == '_archive':
            continue
        rel = os.path.relpath(dirpath, mirror_root)
        for name in sorted(files):
            if not name.startswith('packet-') or not name.endswith('.md'):
                continue
            mirror_file = os.path.join(dirpath, name)
            canon_file = os.path.join(canon_root, rel, name)
            if not os.path.exists(canon_file):
                bad.append(('C8', '%s/%s' % (rel, name),
                            'есть в зеркале, но нет в каноне: зомби-пакет в раздачу не годится'))
                continue
            with open(mirror_file, encoding='utf-8') as fh:
                mirrored = fh.read()
            with open(canon_file, encoding='utf-8') as fh:
                canonical = fh.read()
            if mirrored != canonical:
                for field in ('depends_on', 'files_allowed', 'goal', 'acceptance'):
                    a = next((l for l in canonical.split('\n') if l.startswith(field + ':')), None)
                    b = next((l for l in mirrored.split('\n') if l.startswith(field + ':')), None)
                    if a != b:
                        bad.append(('C8', '%s/%s' % (rel, name),
                                    'зеркало разошлось с каноном: %s — канон `%s` против зеркала `%s`'
                                    % (field, _short(a), _short(b))))


    return bad


def is_test_path(path):
    """Тестовый бюджет — по расположению, а не по имени: `src/x_test.rs` это src."""
    return 'tests' in path.replace('\\', '/').split('/')


def is_plan_path(path):
    """Файл пакета вне бюджета: это задание воркеру, а не изменение под ревью."""
    return '.sisyphus/plans' in path.replace('\\', '/')


def added_counts(diff_text):
    """(логические строки src, полные строки tests) из unified diff.

    Считается ДОБАВЛЕННОЕ коммитом, а не нынешний размер файла: иначе пакет,
    добавивший в общий файл десять строк, штрафуется за чужой код.
    """
    src = tests = 0
    cur = ''
    for line in diff_text.splitlines():
        if line.startswith('+++ b/'):
            cur = line[6:]
        elif line.startswith('+') and not line.startswith('+++') and cur not in ('', '/dev/null'):
            if is_plan_path(cur):
                continue
            body = line[1:]
            if is_test_path(cur):
                tests += 1
            elif body.strip() and not body.strip().startswith('//'):
                src += 1
    return src, tests


def budget_violations(name, src, tests, src_limit=SRC_LINES_DEFAULT,
                      test_limit=TEST_LINES_DEFAULT):
    bad = []
    if src > src_limit:
        bad.append(('C2', name, 'src %d логических строк (лимит %d)' % (src, src_limit)))
    if tests > test_limit:
        bad.append(('C2', name, 'tests %d строк (лимит %d)' % (tests, test_limit)))
    return bad


def measure_commit(root, ref, src_limit=SRC_LINES_DEFAULT, test_limit=TEST_LINES_DEFAULT):
    """C2 по диффу коммита. Неизмеренный дифф — нарушение, а не «чисто»."""
    proc = subprocess.run(['git', '-C', root, 'diff', '%s^1' % ref, ref, '--unified=0'],
                          capture_output=True, text=True)
    if proc.returncode != 0:
        return [('C2', ref, 'дифф не измерен: %s'
                 % (proc.stderr.strip().split('\n')[0][:70]))]
    return budget_violations(ref, *added_counts(proc.stdout), src_limit, test_limit)


def check(plan_name, packets, raw_packets=(), max_files=MAX_FILES_DEFAULT):
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

    # C6 — пример «риск + уровень -> вердикт» с недостаточным уровнем
    for name, raw in raw_packets:
        for lineno, line in raw:
            # Ссылка на исправленный пример заключена в ёлочки, и её не следует
            # проверять: «раньше был записан как «R2 + L2 -> Ask»» — это цитата
            # ошибочного варианта, а не предписание. Структурный признак надёжнее
            # списка слов: словесный («раньше стояло») не совпадал с «раньше был
            # записан», и проверка продолжала ругаться на собственное исправление.
            bare = QUOTED_RE.sub(' ', line)
            if EXEMPT.search(bare):
                continue
            for risk, lvl, verdict in PAIR_RE.findall(bare):
                need, have = RISK_REQUIRES[risk], int(lvl[1])
                if verdict in ('Ask', 'Allow') and have < need:
                    bad.append(('C6', name,
                                'строка %d: %s требует %s, указан %s, а вердикт %s — '
                                'при недостаточном уровне должен быть Deny'
                                % (lineno, 'R' + risk, LEVEL_ORDER[need],
                                   LEVEL_ORDER[have], verdict)))

    # C5 — отозванное решение всё ещё прописано как живое указание
    for name, raw in raw_packets:
        for lineno, line in raw:
            for rx, why in REVOKED:
                if rx.search(line) and not EXEMPT.search(line):
                    bad.append(('C5', name, 'строка %d: %s' % (lineno, why)))

    # C4 — поле задано свёрнутым блоком: значение молча усекается до маркера
    for name, p in packets:
        for key in p.get('_folded', ()):
            bad.append(('C4', name,
                        'поле %s задано свёрнутым блоком: значение усекается до '
                        'одного символа, приёмка и маркер RED теряются молча' % key))

    return bad


# ── самопроверка на фикстурах ────────────────────────────────────────────────
def self_test():
    fails = []

    ran = []

    def expect(cond, label):
        ran.append(label)
        print('  %s %s' % ('ok  ' if cond else 'FAIL', label))
        if not cond:
            fails.append(label)

    # 0) RED-фикстура пакета: пустая строка между заголовком и метаданными.
    #    До починки parse() обрывался на первой пустой строке, и пакет выпадал
    #    из проверки целиком, а линт печатал «пакетов нет» вместо результата.
    here = os.path.dirname(os.path.abspath(__file__))
    fixture = os.path.join(here, "tests", "plan-lint-parse-fixture.md")
    parsed = parse(fixture) if os.path.exists(fixture) else None
    expect(
        parsed is not None and parsed.get("plan_id") == "TEST",
        "пустая строка в метаданных не обрывает разбор пакета",
    )
    expect(
        bool(parsed) and str(parsed.get("files_allowed", "")) == "[a.rs]",
        "значение поля читается целиком, а не обрезается до первой строки",
    )
    # Регрессия по настоящим пакетам. Фикстура проверяет частный случай, а живой
    # прогон — единственный, кто поймал регрессию, внесённую этой же починкой:
    # self-test был зелёным, а M1/M2/M4/M5 перестали разбираться целиком.
    plans_root = os.path.join(os.path.dirname(here), ".sisyphus", "plans")
    physical = parsed_count = 0
    unreadable = []
    for milestone in sorted(os.listdir(plans_root)) if os.path.isdir(plans_root) else []:
        mdir = os.path.join(plans_root, milestone)
        if not os.path.isdir(mdir):
            continue
        for name in sorted(os.listdir(mdir)):
            # Тот же отбор, что и в load_plan: packet-NN.result.md — артефакт
            # результата, а не пакет, и считать его пакетом значит требовать
            # метаданных от файла, который пакетом не является.
            if not re.match(r"^packet-[\w-]+\.md$", name):
                continue
            physical += 1
            if parse(os.path.join(mdir, name)) is not None:
                parsed_count += 1
            else:
                unreadable.append("%s/%s" % (milestone, name))
    # Известные нечитаемые пакеты: у них блок-цитата стоит ПЕРЕД заголовком, и
    # они не разбирались никогда — ни старым, ни новым парсером. Это отдельный дефект
    # (зафиксирован как FINDING), а не следствие починки. Перечисление явное, чтобы
    # новое непроверяемое правило всё равно роняло тест.
    known_unparsed = {"M2/packet-12.md", "M3/packet-12.md"}
    unexpected = sorted(set(unreadable) - known_unparsed)
    expect(
        physical > 0 and parsed_count + len(known_unparsed) == physical and not unexpected,
        "все пакеты кроме известных нечитаемых разобраны: %d из %d%s"
        % (parsed_count, physical, ("; неожиданно не прочитаны: " + ", ".join(unexpected)) if unexpected else ""),
    )

    junk = os.path.join(here, "tests", "plan-lint-no-title.tmp")
    with open(junk, "w", encoding="utf-8") as fh:
        fh.write("plan_id: X\ngoal: g\n")
    expect(parse(junk) is None, "файл без заголовка не становится пакетом")
    os.remove(junk)

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

    # 4a) C2, часть про строки (ADR-0003): бюджет — это ДОБАВЛЕННОЕ пакетом, а не
    #     нынешний размер файла. Иначе все пакеты, трогающие один файл, получают
    #     его полный размер и «нарушают» лимит задним числом.
    def _add(path, lines, comments=0):
        body = ['// инвариант'] * comments + ['строка'] * lines
        return '+++ b/%s\n@@ -0,0 +1,%d @@\n%s' % (
            path, len(body), '\n'.join('+' + l for l in body))

    def _src(diff_text):
        return added_counts(diff_text)[0]

    def _tst(diff_text):
        return added_counts(diff_text)[1]

    over = _add('crates/x/src/lib.rs', 201, comments=150)
    expect(_src(over) == 201, '201 логическая строка src считается за 201, не за 351')
    expect(bool(budget_violations('p', *added_counts(over))),
           'C2 ловит превышение src при 150 комментариях')
    at200 = _add('crates/y/src/lib.rs', 200, comments=150)
    expect(not budget_violations('p', *added_counts(at200)),
           'C2 чист на 200 логических строках src с комментариями')
    t251, t250 = _add('crates/x/tests/big.rs', 251), _add('crates/y/tests/big.rs', 250)
    expect(_tst(t251) == 251 and _tst(t250) == 250,
           'тесты считаются полностью, включая пустые строки')
    expect(bool(budget_violations('p', *added_counts(t251)))
           and not budget_violations('p', *added_counts(t250)),
           'C2 ловит 251 строку теста и чист на 250')
    fake = _add('crates/z/src/session_test.rs', 201)
    expect(any('src' in m for _, _, m in budget_violations('p', *added_counts(fake))),
           'файл с тестовым именем вне tests/ считается src')
    import subprocess as _spr
    _repo = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    expect(bool(measure_commit(_repo, 'no-such-ref-0000')),
           'C2 ловит неизмеренный дифф: отсутствие замера не проходит как чистота')
    plan = _add('.sisyphus/plans/M9/packet-99.md', 300)
    expect(added_counts(plan) == (0, 0) and not budget_violations('p', *added_counts(plan)),
           'файл пакета вне бюджета: план это задание, а не изменение под ревью')
    doc = _add('docs/adr/0004-x.md', 201)
    expect(bool(budget_violations('p', *added_counts(doc))),
           'ADR идёт в src-бюджет логическими строками, отдельной категории нет')

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
    # 6) C4: свёрнутое значение усекается молча — ловим
    folded_bad = [('packet-f', {'goal': 'GREEN: спецификация', 'files_allowed': 'a.rs',
                                'acceptance': '>', '_folded': ['acceptance']})]
    expect(any(c == 'C4' for c, _, _ in check('T', folded_bad)),
           'C4 ловит поле, заданное свёрнутым блоком')
    expect(not check('T', [('packet-ok', {'goal': 'GREEN: x', 'files_allowed': 'a.rs',
                                          'acceptance': 'exit 0'})]),
           'C4 не срабатывает на обычном однострочном поле')
    # 7) is_red: 'required_level' содержит буквы r-e-d, но это НЕ маркер RED
    required_level_pkt = [
        ('packet-req', {'goal': 'GREEN: правка предиката',
                        'files_allowed': 'a.rs',
                        'acceptance': 'добиться risk_level_policy -> 0 failed; '
                                      'r4_r5_require_owner и required_level_monotonic '
                                      'проходят без правок'})]
    expect(not any(c == 'C3' for c, _, _ in check('T', required_level_pkt)),
           'is_red не срабатывает на required_level (подстрока requi-RED)')
    expect(is_red({'goal': 'RED: спецификация', 'acceptance': 'падает'}),
           'is_red по-прежнему ловит настоящий маркер RED')
    # 7) C3 не ловит GREEN-пакет, ссылающийся на RED-пакеты в приёмке
    green_backref = [('packet-g', {'goal': 'Реализовать decide',
                                   'files_allowed': 'a.rs',
                                   'depends_on': '03a, 03b',
                                   'acceptance': 'оба теста зелёные, и каждый сценарий '
                                                 'из обоих RED-пакетов утверждён'})]
    expect(not any(c == 'C3' for c, _, _ in check('T', green_backref, [])),
           'C3 не считает RED-пакетом GREEN со ссылкой на RED-пакеты в приёмке')
    # 8) C3 по-прежнему ловит пакет, объявляющий RED в начале приёмки (packet-01)
    red_lead = [('packet-r1', {'goal': 'Создать крейт и падающий тест',
                              'files_allowed': 'a.rs',
                              'depends_on': '—',
                              'acceptance': 'RED: `! cargo test` -> exit 0'})]
    expect(not any(c == 'C3' for c, _, _ in check('T', red_lead, [])),
           'RED в начале приёмки распознаётся и снимается маркером падения')
    expect(is_red({'goal': 'Создать крейт и падающий тест',
                   'acceptance': 'RED: собирается, падает'}),
           'RED-пакет без слова RED в goal распознаётся по началу приёмки')
    # 8) пустой каталог — не «всё чисто», а «проверять было нечего»
    import sys as _sys
    _saved = _sys.argv
    try:
        _sys.argv = ['plan-lint.py', '/nonexistent/milestone']
        expect(main() == 2, 'пустой/несуществующий каталог даёт exit 2, а не 0')
        # Проверять main('--self-test') отсюда нельзя: main зовёт self_test, а
        # self_test звал бы main — рекурсия до переполнения.
    finally:
        _sys.argv = _saved
    # 8) C8: расхождение зеркала с каноном — и никакой «проверки вхолостую»
    import tempfile as _tf
    _r = _tf.mkdtemp()
    for _tree in (CANON_PLANS, MIRROR_PLANS):
        os.makedirs(os.path.join(_r, _tree, 'M3'))
    with open(os.path.join(_r, CANON_PLANS, 'M3/packet-04.md'), 'w') as f:
        f.write('depends_on: [03a, 03b]\n')
    with open(os.path.join(_r, MIRROR_PLANS, 'M3/packet-04.md'), 'w') as f:
        f.write('depends_on: [03]\n')
    expect(any(c == 'C8' for c, _, _ in check_mirror_drift(_r)),
           'C8 ловит зеркало с устаревшим depends_on')
    with open(os.path.join(_r, MIRROR_PLANS, 'M3/packet-04.md'), 'w') as f:
        f.write('depends_on: [03a, 03b]\n')
    expect(not check_mirror_drift(_r), 'C8 молчит, когда зеркало совпадает с каноном')
    with open(os.path.join(_r, MIRROR_PLANS, 'M3/packet-99.md'), 'w') as f:
        f.write('goal: зомби\n')
    expect(any('зомби' in m for _, _, m in check_mirror_drift(_r)),
           'C8 ловит пакет, которого нет в каноне')
    # 8) C9: пересечение файла между milestone без объявленного порядка.
    # Положительный случай обязателен: проверка, для которой нет теста «вот тут
    # она срабатывает», неотличима от проверки, которая не работает.
    _r9 = _tf.mkdtemp()
    for _ms in ('M2', 'M4'):
        os.makedirs(os.path.join(_r9, CANON_PLANS, _ms))
    _fa = 'files_allowed: [crates/x/src/runner.rs]'
    with open(os.path.join(_r9, CANON_PLANS, 'M2/packet-09.md'), 'w') as f:
        f.write('%s\ndepends_on: [08]\n' % _fa)
    with open(os.path.join(_r9, CANON_PLANS, 'M4/packet-02.md'), 'w') as f:
        f.write('%s\ndepends_on: [01]\n' % _fa)
    expect(any(c == 'C9' for c, _, _ in check_cross_milestone_overlap(_r9)),
           'C9 ловит пересечение между milestone без объявленного порядка')
    with open(os.path.join(_r9, CANON_PLANS, 'M4/packet-02.md'), 'w') as f:
        f.write('%s\ndepends_on: [01, M2-09]\n' % _fa)
    expect(not check_cross_milestone_overlap(_r9),
           'C9 молчит, когда порядок объявлен через M2-09')
    # Пересечение ВНУТРИ одного milestone — не дело C9 (это C1). Файл берётся
    # отдельный: если заявить его ещё и M2-пакету, не объявив его в M4-02, то
    # получится настоящая неупорядоченная ПАРА МЕЖДУ milestone, и проверка
    # была бы права, а тест — нет.
    _fb = 'files_allowed: [crates/x/src/lifecycle.rs]'
    for _n, _d in (('07', '[06]'), ('08', '[07]')):
        with open(os.path.join(_r9, CANON_PLANS, 'M2/packet-%s.md' % _n), 'w') as f:
            f.write('%s\ndepends_on: %s\n' % (_fb, _d))
    expect(not any(c == 'C9' for c, _, _ in check_cross_milestone_overlap(_r9)),
           'C9 не ловит пересечение ВНУТРИ milestone — это дело C1')
    with open(os.path.join(_r9, CANON_PLANS, 'M4/packet-02.md'), 'w') as f:
        f.write('%s\ndepends_on: [01]\noverlap_waiver: осознанное пересечение\n' % _fa)
    expect(not check_cross_milestone_overlap(_r9),
           'overlap_waiver снимает C9')
    # 9) C5: отозванное решение, прописанное как живое указание, — ловим
    revoked_bad = ([('packet-v', {'goal': 'GREEN: x', 'files_allowed': 'a.rs',
                                  'acceptance': 'exit 0'})],
                   [('packet-v', [(9, '3. Риск: risk.unwrap_or(R0) + delta')])])
    expect(any(c == 'C5' for c, _, _ in check('T', *revoked_bad)),
           'C5 ловит отозванное решение как предписание')
    # 9) C5 ловит ТАБЛИЧНУЮ форму: `| R3 | `L4Admin` | ...`
    #    Первая версия шаблона требовала стрелку и пропустила обе строки
    #    устаревших таблиц, которые как раз требовалось поймать.
    table_bad = ([('packet-tb', {'goal': 'GREEN: x', 'files_allowed': 'a.rs',
                                 'acceptance': 'exit 0'})],
                 [('packet-tb', [(4, '| R3 | `L4Admin` | Ask | доступ к секретам |')])])
    expect(any(c == 'C5' for c, _, _ in check('T', *table_bad)),
           'C5 ловит табличную форму R3=L4Admin (с вертикальной чертой и backticks)')
    # 10) C5 ловит форму с backticks вокруг Ask и Deny
    ask_bad = ([('packet-ak', {'goal': 'GREEN: x', 'files_allowed': 'a.rs',
                               'acceptance': 'exit 0'})],
               [('packet-ak', [(31, '3. `Ask` без подтверждения = `Deny` с причиной.')])])
    expect(any(c == 'C5' for c, _, _ in check('T', *ask_bad)),
           'C5 ловит форму `Ask` без подтверждения = `Deny` с backticks')
    # 11) C6: пример «риск + уровень -> вердикт» с недостаточным уровнем
    lvl_bad = ([('packet-lb', {'goal': 'GREEN: x', 'files_allowed': 'a.rs',
                               'acceptance': 'exit 0'})],
               [('packet-lb', [(44, '- R2 + L2 → Ask (спрашиваем);')])])
    expect(any(c == 'C6' for c, _, _ in check('T', *lvl_bad)),
           'C6 ловит R2+L2 → Ask (R2 требует L3Maintainer)')
    # 12) C6 НЕ срабатывает на корректных случаях — иначе проверка бесполезна
    lvl_ok = ([('packet-lo', {'goal': 'GREEN: x', 'files_allowed': 'a.rs',
                              'acceptance': 'exit 0'})],
              [('packet-lo', [(45, '- R3 + L5 → Ask (человек решает доступ к секретам);'),
                              (46, '- R4 + L5 → **Ask на одно действие** (В1), не Deny;'),
                              (47, '- R2 + L2 → **Deny**: R2 требует L3Maintainer;'),
                              (48, '- R0 + L2 + Public + first-party binding → Allow;')])])
    expect(not any(c == 'C6' for c, _, _ in check('T', *lvl_ok)),
           'C6 молчит на корректных сочетаниях (R3+L5, R4+L5, R2+L2→Deny, R0+L2)')
    # 13) C6 не ловит ЦИТАТУ исправленного примера (реальный ложноположительный)
    quoted = ([('packet-q', {'goal': 'GREEN: x', 'files_allowed': 'a.rs',
                             'acceptance': 'exit 0'})],
              [('packet-q', [(46, 'был записан как «R2 + L2 -> Ask», что противоречило правилу')])])
    expect(not any(c == 'C6' for c, _, _ in check('T', *quoted)),
           'C6 не ловит цитату исправленного примера')
    # 14) C5: то же самое, но СПЕЦИАЛЬНО ЗАПРЕЩЁННОЕ — нарушением не является
    revoked_ok = ([('packet-w', {'goal': 'GREEN: x', 'files_allowed': 'a.rs',
                                 'acceptance': 'exit 0'})],
                  [('packet-w', [(9, 'risk.unwrap_or(R0) запрещён: это дефект (В2)')])])
    expect(not any(c == 'C5' for c, _, _ in check('T', *revoked_ok)),
           'C5 не срабатывает на запрет отозванного решения')
    # 10) C5 не должен ловить живую таблицу, обновлённую по ADR-0002
    table_ok = ([('packet-t', {'goal': 'GREEN: x', 'files_allowed': 'a.rs',
                               'acceptance': 'exit 0'})],
                [('packet-t', [(4, '| R3 | `L5Owner` | Ask | необратимая утечка |')])])
    expect(not any(c == 'C5' for c, _, _ in check('T', *table_ok)),
           'C5 не срабатывает на актуальной таблице R3=L5Owner')

    # 15) C7 сверяет документ с contract.rs
    import tempfile
    tdir = tempfile.mkdtemp()
    os.makedirs(os.path.join(tdir, 'crates/stellwerk-core/src'))
    with open(os.path.join(tdir, 'crates/stellwerk-core/src/contract.rs'), 'w') as f:
        f.write('RiskClass::R0 => PolicyLevel::L0Local,\n'
                'RiskClass::R3 => PolicyLevel::L5Owner,\n')
    for _rel in TABLE_DOCS:
        _full = os.path.join(tdir, _rel)
        os.makedirs(os.path.dirname(_full), exist_ok=True)
        with open(_full, 'w') as f:
            f.write('| R0 | `L0Local` | Allow | x |\n| R3 | `L5Owner` | Ask | y |\n')
    expect(not check_risk_tables(tdir), 'C7 молчит, когда документ совпадает с contract.rs')
    with open(os.path.join(tdir, TABLE_DOCS[0]), 'a') as f:
        f.write('| R3 | `L4Admin` | Ask | устарело |\n')
    expect(any(c == 'C7' for c, _, _ in check_risk_tables(tdir)),
           'C7 ловит расхождение документа с contract.rs')
    # 16) C7 ловит расхождение двух документов между собой
    with open(os.path.join(tdir, TABLE_DOCS[1]), 'a') as f:
        f.write('| R3 | `L4Admin` | Ask | расходится с первым |\n')
    expect(any('разошлись' in m for _, _, m in check_risk_tables(tdir)),
           'C7 ловит расхождение двух таблиц risk_class между собой')

    print('  self-test: провалов %d из %d проверок' % (len(fails), len(ran)))
    return 0 if not fails else 1


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('plans', nargs='*', help='каталоги .sisyphus/plans/<M>')
    ap.add_argument('--self-test', action='store_true')
    ap.add_argument('--max-files', type=int, default=MAX_FILES_DEFAULT)
    ap.add_argument('--budget', help='реф (merge-коммит) для замера бюджета строк (C2)')
    ap.add_argument('--gating', action='store_true',
                    help='код возврата определяет только C7 (сверка risk_class '
                         'с contract.rs); C1-C6 остаются информационными')
    a = ap.parse_args()

    if a.self_test:
        return self_test()

    here = os.path.dirname(os.path.abspath(__file__))   # …/stellwerk/scripts
    root = os.path.dirname(here)                        # …/stellwerk
    targets = a.plans
    if not targets:
        base = os.path.join(root, '.sisyphus', 'plans')
        if not os.path.isdir(base):
            print('plan-lint: нет %s' % base, file=sys.stderr)
            return 2
        targets = [os.path.join(base, d) for d in sorted(os.listdir(base))
                   if os.path.isdir(os.path.join(base, d))]

    total = 0
    # Каталог без пакетов — это не «всё чисто», а «проверять было нечего».
    # Раньше такой случай печатался и давал exit 0, то есть гейт, наведённый на
    # неверный путь, отчитывался об успехе, не проверив ни одного пакета.
    # Тот же класс, что вакуумный PASS теста.
    empty_targets = 0
    for d in targets:
        packets = load_plan(d)
        if not packets:
            print('%s: пакетов нет' % d)
            empty_targets += 1
            continue
        bad = check(os.path.basename(d.rstrip('/')), packets, load_raw(d), a.max_files)
        name = os.path.basename(d.rstrip('/'))
        if bad:
            # При --gating в код возврата идёт только C7. Иначе гейт упал бы на
            # 16 исторических нарушениях M0, и проверка сверки таблицы вообще
            # не дошла бы до исполнения — то есть гейт был бы мёртвым.
            if not a.gating:
                total += len(bad)
            print('%s: %d пакетов, НАРУШЕНИЙ %d' % (name, len(packets), len(bad)))
            for code, pkt, msg in bad:
                print('  [%s] %s: %s' % (code, pkt, msg))
        else:
            print('%s: %d пакетов, нарушений нет' % (name, len(packets)))
    if a.budget:
        mb = measure_commit(root, a.budget)
        if mb:
            if not a.gating:
                total += len(mb)
            print('БЮДЖЕТ %s: нарушений %d' % (a.budget, len(mb)))
            for code, pkt, msg in mb:
                print('  [%s] %s: %s' % (code, pkt, msg))
        else:
            print('БЮДЖЕТ %s: в пределах лимита' % a.budget)
      # C7 — сверка документированной таблицы с contract.rs. Печатается всегда,
    # даже когда пакеты переданы явными аргументами: это проверка не пакета, а
    # согласованности источников истины между собой.
    if a.gating:
        c7g = check_risk_tables(root)
        if c7g:
            print('ГЕЙТ: сверка risk_class с contract.rs — НАРУШЕНИЙ %d' % len(c7g))
            for code, where, msg in c7g:
                print('  [%s] %s: %s' % (code, where, msg))
            return 1
        print('ГЕЙТ: сверка risk_class с contract.rs — совпадает')
        return 0
    # C8 — зеркало пакетов. Печатается рядом с C7, потому что обе проверки
    # отвечают на один вопрос: соответствует ли видимая воркеру картина плана
    # канону.
    c8 = check_mirror_drift(root)
    if c8:
        print('ЗЕРКАЛО ПАКЕТОВ: расхождений %d — .omo/plans НЕ является источником' % len(c8))
        for code, where, msg in c8[:12]:
            print('  [%s] %s: %s' % (code, where, msg))
        if len(c8) > 12:
            print('  ... ещё %d' % (len(c8) - 12))
    else:
        print('ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном')

    c9 = check_cross_milestone_overlap(root)
    if c9:
        total += len(c9)
        print('ПЕРЕСЕЧЕНИЯ МЕЖДУ MILESTONE: %d — порядок не объявлен' % len(c9))
        for code, where, msg in c9[:10]:
            print('  [%s] %s: %s' % (code, where, msg))
        if len(c9) > 10:
            print('  ... ещё %d' % (len(c9) - 10))
    else:
        print('ПЕРЕСЕЧЕНИЯ МЕЖДУ MILESTONE: порядок объявлен везде')

    c7 = check_risk_tables(root)
    if c7:
        total += len(c7)
        print('ТАБЛИЦА risk_class: НАРУШЕНИЙ %d' % len(c7))
        for code, where, msg in c7:
            print('  [%s] %s: %s' % (code, where, msg))
    else:
        print('ТАБЛИЦА risk_class: совпадает с contract.rs во всех трёх документах')
    if empty_targets:
        print('ПРОВЕРЕНО ПУСТЫХ КАТАЛОГОВ: %d — их содержимое не проверено' % empty_targets)
        return 2
    return 1 if total else 0


if __name__ == '__main__':
    sys.exit(main())

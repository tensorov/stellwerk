#!/usr/bin/env python3
import os
import sys
import unittest

# Add scripts dir to path
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))

import importlib.util
spec = importlib.util.spec_from_file_location("plan_lint", os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "plan-lint.py"))
plan_lint = importlib.util.module_from_spec(spec)
sys.modules['plan_lint'] = plan_lint
spec.loader.exec_module(plan_lint)


class TestPlanLintN2Formats(unittest.TestCase):
    def test_parse_n2_packet_00j1(self):
        path = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
                            '.sisyphus', 'plans', 'N2', 'packet-00j1.md')
        parsed = plan_lint.parse(path)
        self.assertIsNotNone(parsed)
        self.assertEqual(parsed.get('depends_on'), '[]')
        # allowed_changes should have 2 items
        allowed = plan_lint.split_list(parsed.get('allowed_changes', ''))
        self.assertEqual(len(allowed), 2)
        self.assertIn('crates/stellwerk-cli/src/build_runner.rs', allowed)
        self.assertIn('crates/stellwerk-cli/tests/build_runner.rs', allowed)
        # phase is not RED
        self.assertFalse(plan_lint.is_red(parsed))
        # acceptance containing "16 passed"
        self.assertIn('16 passed', parsed.get('acceptance', ''))


class TestPlanLintN1TuiFormats(unittest.TestCase):
    def _path(self, *parts):
        root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
        return os.path.join(root, '.sisyphus', 'plans', *parts)

    def test_parse_n1_packet_01(self):
        parsed = plan_lint.parse(self._path('N1', 'packet-01.md'))
        self.assertIsNotNone(parsed)
        files = plan_lint.split_list(parsed.get('files_allowed', ''))
        self.assertEqual(len(files), 5)
        self.assertIn('Cargo.toml', files)
        self.assertIn('crates/stellwerk-cli/src/main.rs', files)
        self.assertFalse(plan_lint.is_red(parsed))

    def test_parse_tui_packet_01(self):
        parsed = plan_lint.parse(self._path('TUI', 'packet-01.md'))
        self.assertIsNotNone(parsed)
        files = plan_lint.split_list(parsed.get('files_allowed', ''))
        self.assertEqual(len(files), 4)
        self.assertIn('Cargo.lock', files)
        deps = plan_lint.split_list(parsed.get('depends_on', ''))
        self.assertIn('TUI-00', deps)
        self.assertFalse(plan_lint.is_red(parsed))

    def test_parse_n2_packet_00b(self):
        parsed = plan_lint.parse(self._path('N2', 'packet-00b.md'))
        self.assertIsNotNone(parsed)
        files = plan_lint.split_list(parsed.get('files_allowed', ''))
        self.assertEqual(len(files), 2)
        self.assertIn('crates/stellwerk-task/src/runner.rs', files)

    def test_c9_cross_milestone_n2_h1(self):
        import tempfile
        root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
        r = tempfile.mkdtemp()
        canon = os.path.join(r, '.sisyphus', 'plans')
        os.makedirs(os.path.join(canon, 'N2'))
        os.makedirs(os.path.join(canon, 'H1'))
        with open(os.path.join(canon, 'N2', 'packet-00j7.md'), 'w') as f:
            f.write('---\nid: N2/00j7\nmilestone: N2\ntitle: t\ndepends_on: []\nallowed_changes:\n  - crates/x/src/tool.py\n---\n\n# t\n\n## Зачем\nx\n\n## Приёмка\n```\nexit 0\n```\n')
        with open(os.path.join(canon, 'H1', 'packet-01.md'), 'w') as f:
            f.write('# H1/01\nplan_id: H1/01\ngoal: t\nfiles_allowed: [crates/x/src/tool.py]\ndepends_on: []\nacceptance: exit 0\n')
        bad = plan_lint.check_cross_milestone_overlap(r)
        self.assertTrue(any(c == 'C9' for c, _, _ in bad))
        with open(os.path.join(canon, 'H1', 'packet-01.md'), 'w') as f:
            f.write('# H1/01\nplan_id: H1/01\ngoal: t\nfiles_allowed: [crates/x/src/tool.py]\ndepends_on: [N2/00j7]\nacceptance: exit 0\n')
        bad2 = plan_lint.check_cross_milestone_overlap(r)
        self.assertFalse(bad2)

    def test_is_red_goal_with_red_in_text_not_leading(self):
        p = {'goal': 'accept.sh reads canonical schema and RED/GREEN sections', 'acceptance': 'exit 0'}
        self.assertFalse(plan_lint.is_red(p))
        bad = plan_lint.check('T', [('packet-x', p)], [])
        self.assertFalse(any(c == 'C3' for c, _, _ in bad))

    def test_is_red_goal_leading_red_still_c3(self):
        p = {'goal': 'RED: specification', 'acceptance': 'cargo test -p x -> 11 passed'}
        self.assertTrue(plan_lint.is_red(p))
        bad = plan_lint.check('T', [('packet-x', p)], [])
        self.assertTrue(any(c == 'C3' for c, _, _ in bad))


if __name__ == '__main__':
    unittest.main()

#!/usr/bin/env python3
import os
import sys
import unittest
from pathlib import Path

# Add scripts dir to path
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))

import importlib.util
spec = importlib.util.spec_from_file_location("packet_replay", os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "packet_replay.py"))
packet_replay = importlib.util.module_from_spec(spec)
sys.modules['packet_replay'] = packet_replay
spec.loader.exec_module(packet_replay)


class TestPacketReplayFormats(unittest.TestCase):
    def test_read_n2_frontmatter(self):
        path = Path(os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
                            '.sisyphus', 'plans', 'N2', 'packet-00j1.md'))
        fields, problems = packet_replay.read_fields(path)
        self.assertEqual(fields.get('id'), 'N2/00j1')
        self.assertEqual(fields.get('milestone'), 'N2')
        self.assertIn('allowed_changes', fields)

    def test_read_n1_bold_fields(self):
        path = Path(os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
                            '.sisyphus', 'plans', 'N1', 'packet-01.md'))
        fields, problems = packet_replay.read_fields(path)
        self.assertIn('depends_on', fields)
        self.assertIn('files_allowed', fields)

    def test_read_tui_bold_fields(self):
        path = Path(os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
                            '.sisyphus', 'plans', 'TUI', 'packet-01.md'))
        fields, problems = packet_replay.read_fields(path)
        self.assertIn('depends_on', fields)
        self.assertIn('files_allowed', fields)

    def test_red_fallback_to_priemka(self):
        path = Path(os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
                            '.sisyphus', 'plans', 'N2', 'packet-00j1.md'))
        cmd = packet_replay.extract_command(path)
        self.assertTrue(cmd.startswith('cargo test'))

    def test_test_s_recognition(self):
        path = Path(os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
                            '.sisyphus', 'plans', 'H1', 'packet-08.md'))
        cmd = packet_replay.extract_command(path)
        self.assertTrue(cmd.startswith('test '))

    def test_n2_00i1_remains_no_command(self):
        path = Path(os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
                            '.sisyphus', 'plans', 'N2', 'packet-00i1.md'))
        cmd = packet_replay.extract_command(path)
        self.assertEqual(cmd, '')


if __name__ == '__main__':
    unittest.main()

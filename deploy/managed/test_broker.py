import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from broker import Broker


class BrokerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.broker = Broker(self.root, '192.168.69.21')
        for name in ('runtime.py', 'web.py', 'xorg.conf'):
            (self.root / name).write_text('test fixture')
        self.profiles = patch.object(self.broker, 'environments', return_value={'environments': [{'id': 'gnome'}, {'id': 'xfce'}]})
        self.profiles.start()
        self.addCleanup(self.profiles.stop)

    def test_rejects_unknown_environment(self):
        with self.assertRaises(ValueError):
            self.broker.create('arbitrary-command', 'a'*36)

    def test_rejects_path_traversal(self):
        with self.assertRaises(KeyError):
            self.broker.metadata('../other')

    def test_empty_inventory_is_array(self):
        self.assertEqual(self.broker.list(), {'desktops': [], 'history': []})

    @patch('broker.subprocess.run')
    def test_create_is_idempotent_and_conflicts_fail(self, run):
        run.return_value.returncode = 0
        first = self.broker.create('gnome', 'a'*36)
        self.assertEqual(first, self.broker.create('gnome', 'a'*36))
        self.assertEqual(run.call_count, 1)
        with self.assertRaises(ValueError):
            self.broker.create('xfce', 'a'*36)

    def test_capacity(self):
        with patch.object(self.broker, 'list', return_value={'desktops': [{}]*4}):
            with self.assertRaisesRegex(ValueError, 'limit'):
                self.broker.create('gnome', 'b'*36)

    @patch('broker.subprocess.run')
    def test_private_roots_and_ports(self, run):
        run.return_value.returncode = 0
        first = self.broker.create('gnome', 'a'*36)
        with patch.object(self.broker, 'list', return_value={'desktops': [first]}):
            second = self.broker.create('gnome', 'b'*36)
        for key in ('id', 'display', 'httpPort', 'icePort'):
            self.assertNotEqual(first[key], second[key])
        self.assertTrue((self.root/'sessions'/second['id']/'runtime.py').is_file())


if __name__ == '__main__':
    unittest.main()

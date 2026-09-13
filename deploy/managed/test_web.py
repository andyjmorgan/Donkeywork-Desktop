import importlib.util
from pathlib import Path
import socket
import threading
import unittest

spec = importlib.util.spec_from_file_location("managed_web", Path(__file__).with_name("web.py"))
web = importlib.util.module_from_spec(spec)
spec.loader.exec_module(web)


class FakeCore:
    instances = []
    def __init__(self, root):
        self.calls = []
        self.closed = False
        self.instances.append(self)
    def call(self, kind, payload=None):
        self.calls.append((kind, dict(payload or {})))
        if kind == "screenshot.request":
            return dict(snapshotId="snapshot", width=1920, height=1080, topologyRevision=1)
        return {"leaseId": "lease"}
    def close(self):
        self.closed = True


class AdapterTests(unittest.TestCase):
    def setUp(self):
        self.original = web.Core
        web.Core = FakeCore
        self.client, server = socket.socketpair()
        self.client.settimeout(3)
        self.worker = threading.Thread(target=web.input_loop, args=(server, Path("/unused"),
            dict(displayId="78", width=1920, height=1080, topologyRevision=1)))
        self.worker.start()
        web.write_json(self.client, {"type": "acquire", "requestId": "test"})
        self.ready = web.read_json(self.client)
        self.core = FakeCore.instances[-1]
    def tearDown(self):
        self.client.close()
        self.worker.join(4)
        web.Core = self.original
        self.assertFalse(self.worker.is_alive())
        self.assertTrue(self.core.closed)
    def event(self, seq, event, generation=None):
        web.write_json(self.client, dict(generation=generation or self.ready["generation"], sequence=seq, event=event))
        return web.read_json(self.client)
    def test_wheel_maps_direction_and_contiguous_core_sequences(self):
        self.assertEqual(self.event(1, dict(type="renew"))["type"], "ack")
        self.assertEqual(self.event(2, dict(type="wheel", x=10, y=20, vertical=2, horizontal=-1))["type"], "ack")
        calls = [p for k, p in self.core.calls if k == "input.pointer"]
        self.assertEqual([p["button"] for p in calls], ["wheel_down", "wheel_down", "wheel_left"])
        self.assertEqual([p["inputSequence"] for p in calls], [1, 2, 3])
        self.assertEqual(self.event(3, dict(type="release"))["type"], "ack")
        self.assertTrue(self.core.closed)
    def test_stale_generation_does_not_inject(self):
        self.assertEqual(self.event(1, dict(type="key", hid=4, down=True), "stale")["type"], "unavailable")
        self.assertFalse(any(k == "input.key" for k, _ in self.core.calls))
    def test_timeout_closes_core_and_releases_held_input(self):
        self.assertEqual(self.event(1, dict(type="key", hid=225, down=True))["type"], "ack")
        self.assertEqual(web.read_json(self.client)["type"], "unavailable")
        self.assertTrue(self.core.closed)


if __name__ == "__main__":
    unittest.main()

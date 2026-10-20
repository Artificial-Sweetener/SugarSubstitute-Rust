# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Verify fixture custody against fake native boundaries without creating windows."""
import importlib.util
import sys
import types
import unittest
from pathlib import Path
from unittest.mock import patch


def owner_type():
    spec = importlib.util.spec_from_file_location("offscreen_native_fixture", Path(__file__).with_name("offscreen_native.py"))
    module = importlib.util.module_from_spec(spec)
    with patch.dict(sys.modules, {name: types.ModuleType(name) for name in ("win32con", "win32gui", "win32process")}):
        spec.loader.exec_module(module)
    return module.PrivateDesktop


class CustodyTests(unittest.TestCase):
    def test_discovery_and_one_stop_failure_cannot_skip_independent_cleanup(self):
        native_owner = owner_type()
        owner = native_owner.__new__(native_owner)
        owner.handles = {1: "root", 2: "child"}
        calls = []
        def discover():
            raise RuntimeError("Snapshot unavailable")
        def stop(handle):
            calls.append(("stop", handle))
            if handle == "child":
                raise RuntimeError("Child stop failed")
        owner.discover = discover
        owner.processes = types.SimpleNamespace(stop=stop, close=lambda h: calls.append(("close", h)))
        owner.desktop = "private"
        owner.close_desktop = lambda desktop: calls.append(("desktop", desktop)) or True
        with self.assertRaisesRegex(RuntimeError, "2 failures"):
            owner.close()
        self.assertEqual(calls, [("stop", "child"), ("stop", "root"), ("close", "root"), ("close", "child"), ("desktop", "private")])
        self.assertEqual(owner.handles, {})

    def test_old_orphan_with_reused_parent_number_is_never_adopted(self):
        native_owner = owner_type()
        owner = native_owner.__new__(native_owner)
        owner.handles, owner.creations = {10: 10}, {10: 20}
        closed = []
        owner.processes = types.SimpleNamespace(
            children=lambda parent: [(21, "old"), (22, "actual")] if parent == 10 else [],
            open=lambda pid: pid,
            lifetime_identity=lambda handle: (handle, 1 if handle == 21 else 21),
            close=closed.append,
        )
        owner.discover()
        self.assertEqual(owner.handles, {10: 10, 22: 22})
        self.assertEqual(closed, [21])


if __name__ == "__main__":
    unittest.main()

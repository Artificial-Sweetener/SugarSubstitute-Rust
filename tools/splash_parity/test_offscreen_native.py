# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Verify fixture custody against fake native boundaries without creating windows."""
import importlib.util
import sys
import types
import unittest
from pathlib import Path
from unittest.mock import Mock, patch


def owner_type():
    spec = importlib.util.spec_from_file_location("offscreen_native_fixture", Path(__file__).with_name("offscreen_native.py"))
    module = importlib.util.module_from_spec(spec)
    with patch.dict(sys.modules, {name: types.ModuleType(name) for name in ("win32con", "win32gui", "win32process")}):
        spec.loader.exec_module(module)
    return module.PrivateDesktop


class CustodyTests(unittest.TestCase):
    def test_root_retention_requires_the_callers_original_native_lifetime(self):
        native_owner = owner_type()
        owner = native_owner.__new__(native_owner)
        owner.handles, owner.creations = {}, {}
        owner.processes = Mock()
        owner.processes.open.return_value = 70
        owner.processes.lifetime_identity.return_value = (7, 100)
        self.assertEqual(owner.retain_root(60), 7)
        self.assertEqual(owner.handles, {7: 70})
        self.assertEqual(owner.creations, {7: 100})
        with self.assertRaisesRegex(RuntimeError, 'already retained'):
            owner.retain_root(60)

    def test_changed_root_lifetime_closes_the_unadmitted_handle(self):
        native_owner = owner_type()
        owner = native_owner.__new__(native_owner)
        owner.handles, owner.creations = {}, {}
        owner.processes = Mock()
        owner.processes.open.return_value = 70
        owner.processes.lifetime_identity.side_effect = [(7, 100), (7, 101)]
        with self.assertRaisesRegex(RuntimeError, 'lifetime changed'):
            owner.retain_root(60)
        owner.processes.close.assert_called_once_with(70)
        self.assertEqual(owner.handles, {})

    def test_root_query_failure_closes_the_unadmitted_handle(self):
        native_owner = owner_type()
        owner = native_owner.__new__(native_owner)
        owner.handles, owner.creations = {}, {}
        owner.processes = Mock()
        owner.processes.open.return_value = 70
        owner.processes.lifetime_identity.side_effect = [(7, 100), OSError('Query failed')]
        with self.assertRaisesRegex(OSError, 'Query failed'):
            owner.retain_root(60)
        owner.processes.close.assert_called_once_with(70)
        self.assertEqual(owner.handles, {})

    def test_suspend_requires_existing_fixture_custody(self):
        native_owner = owner_type()
        owner = native_owner.__new__(native_owner)
        owner.handles = {10: 70}
        owner.processes = Mock()
        with self.assertRaises(KeyError):
            owner.suspend(11)
        owner.processes.suspend.assert_not_called()
        owner.suspend(10)
        owner.processes.suspend.assert_called_once_with(70)

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
        owner.parents = {}
        closed = []
        owner.processes = types.SimpleNamespace(
            ancestry=lambda: {10: [(21, "old"), (22, "actual")]},
            open=lambda pid: pid,
            lifetime_identity=lambda handle: (handle, 1 if handle == 21 else 21),
            close=closed.append,
        )
        owner.discover()
        self.assertEqual(owner.handles, {10: 10, 22: 22})
        self.assertEqual(closed, [21])
        self.assertEqual(owner.descendants(10), {22})

    def test_descendant_query_failure_releases_only_the_unadmitted_child(self):
        native_owner = owner_type()
        owner = native_owner.__new__(native_owner)
        owner.handles, owner.creations, owner.parents = {10: 10}, {10: 20}, {}
        owner.processes = Mock()
        owner.processes.ancestry.return_value = {10: [(22, 'child')]}
        owner.processes.open.return_value = 22
        owner.processes.lifetime_identity.side_effect = OSError('Query failed')
        with self.assertRaisesRegex(OSError, 'Query failed'):
            owner.discover()
        owner.processes.close.assert_called_once_with(22)
        self.assertEqual(owner.handles, {10: 10})
        self.assertEqual(owner.parents, {})

    def test_one_snapshot_admits_all_descendants_and_excludes_unrelated_processes(self):
        native_owner = owner_type()
        owner = native_owner.__new__(native_owner)
        owner.handles, owner.creations = {10: 10}, {10: 10}
        owner.parents = {}
        snapshots = []
        def ancestry():
            snapshots.append(True)
            return {10: [(20, "child")], 20: [(30, "grandchild")],
                    999: [(40, "unrelated")]}
        owner.processes = types.SimpleNamespace(
            ancestry=ancestry, open=lambda pid: pid,
            lifetime_identity=lambda handle: (handle, handle),
            close=lambda handle: self.fail(f"Unexpected close {handle}"),
        )
        owner.discover()
        self.assertEqual(owner.handles, {10: 10, 20: 20, 30: 30})
        self.assertEqual(len(snapshots), 1)
        self.assertEqual(owner.descendants(10), {20, 30})
        self.assertEqual(owner.descendants(20), {30})
        self.assertEqual(owner.descendants(999), set())


class CreatorTransferTests(unittest.TestCase):
    def setUp(self):
        native_owner = owner_type()
        self.owner = native_owner.__new__(native_owner)
        self.owner.handles, self.owner.creations, self.owner.parents = {10: 100}, {10: 20}, {}
        self.owner.processes = Mock()
        self.owner.processes.lifetime_identity.return_value = (22, 30)
        self.owner.duplicate_handle = Mock(return_value=220)
        self.report = {'creator_pid': 10, 'entry_pid': 22, 'alive': [22],
            'handles': {'22': 200}, 'process_creation_filetimes': {'22': 30}, 'parents': {}}

    def test_transfer_uses_creator_capability_and_validates_original_lifetime(self):
        self.owner.import_family(self.report, 10, 22)
        self.owner.duplicate_handle.assert_called_once_with(100, 200)
        self.owner.processes.open.assert_not_called()
        self.assertEqual(self.owner.handles, {10: 100, 22: 220})
        self.assertEqual(self.owner.parents, {22: 10})
        self.owner.import_family(self.report, 10, 22)
        self.owner.duplicate_handle.assert_called_once()

    def test_foreign_creator_and_entry_never_transfer_handles(self):
        for key in ('creator_pid', 'entry_pid'):
            with self.subTest(key=key):
                report = dict(self.report, **{key: 999})
                with self.assertRaisesRegex(RuntimeError, 'retained creator'):
                    self.owner.import_family(report, 10, 22)
        self.owner.duplicate_handle.assert_not_called()

    def test_changed_native_lifetime_closes_unadmitted_transfer(self):
        self.owner.processes.lifetime_identity.return_value = (22, 31)
        with self.assertRaisesRegex(RuntimeError, 'differs from its receipt'):
            self.owner.import_family(self.report, 10, 22)
        self.owner.processes.close.assert_called_once_with(220)
        self.assertEqual(self.owner.handles, {10: 100})

    def test_stale_capability_does_not_admit_a_process_or_prove_completion(self):
        for code in (6, 87):
            cause = OSError('Creator retired its handle')
            cause.winerror = code
            self.owner.duplicate_handle.side_effect = cause
            self.owner.import_family(self.report, 10, 22)
            self.assertEqual(self.owner.handles, {10: 100})
            self.assertNotIn('completed', self.report)

    def test_access_failure_is_not_mistaken_for_retirement(self):
        cause = OSError('Access denied')
        cause.winerror = 5
        self.owner.duplicate_handle.side_effect = cause
        with self.assertRaises(OSError):
            self.owner.import_family(self.report, 10, 22)

    def test_old_lifetime_and_pseudo_handles_are_rejected(self):
        self.report['process_creation_filetimes']['22'] = 19
        with self.assertRaisesRegex(RuntimeError, 'predates'):
            self.owner.import_family(self.report, 10, 22)
        self.report['process_creation_filetimes']['22'] = 30
        for raw in (0, -1, True, '200'):
            self.report['handles']['22'] = raw
            with self.assertRaisesRegex(RuntimeError, 'capability is invalid'):
                self.owner.import_family(self.report, 10, 22)
        self.owner.duplicate_handle.assert_not_called()


class ImageObservationTests(unittest.TestCase):
    """Exercise image-query retirement without launching processes or native windows."""

    def setUp(self):
        native_owner = owner_type()
        self.owner = native_owner.__new__(native_owner)
        self.owner.handles, self.owner.creations = {7: 70}, {7: 100}
        self.owner.processes = Mock()
        self.owner.processes.identity.return_value = (7, Path("app.exe"), 100)
        self.owner.processes.lifetime_identity.return_value = (7, 100)
        self.owner.processes.wait_exited.return_value = True
        self.owner.exit_codes = Mock(return_value={7: 0})
        self.report = {}

    def query_failure(self, code):
        cause = OSError("Injected native query failure")
        cause.winerror = code
        self.owner.processes.identity.side_effect = cause

    def test_exited_image_metadata_retirement_keeps_exact_lifetime_and_exit(self):
        self.query_failure(31)
        self.owner.observe_images(self.report)
        self.assertEqual(self.report["process_images"], {})
        receipt = self.report["unavailable_process_images"][7]
        self.assertEqual(receipt["creation_100ns"], 100)
        self.assertEqual(receipt["exit_code"], 0)
        self.assertTrue(receipt["exited"])
        self.owner.observe_images(self.report)
        self.owner.processes.identity.assert_called_once_with(70)

    def test_live_process_image_failure_is_not_hidden(self):
        self.owner.processes.wait_exited.return_value = False
        for code in (5, 31):
            self.query_failure(code)
            with self.subTest(code=code), self.assertRaises(OSError):
                self.owner.observe_images(self.report)
            self.assertEqual(self.report["unavailable_process_images"], {})

    def test_access_denied_during_teardown_requires_original_process_exit(self):
        self.query_failure(5)
        self.owner.observe_images(self.report)
        self.owner.processes.wait_exited.assert_called_once_with(70, 100)
        receipt = self.report["unavailable_process_images"][7]
        self.assertEqual((receipt["winerror"], receipt["creation_100ns"], receipt["exit_code"]), (5, 100, 0))
        self.assertEqual(self.report["process_images"], {})

    def test_other_native_failure_is_not_hidden_after_exit(self):
        self.query_failure(6)
        with self.assertRaises(OSError):
            self.owner.observe_images(self.report)
        self.owner.processes.lifetime_identity.assert_not_called()
        self.owner.processes.wait_exited.assert_not_called()

    def test_exited_reused_identity_is_rejected(self):
        self.query_failure(31)
        self.owner.processes.lifetime_identity.return_value = (7, 101)
        with self.assertRaises(RuntimeError):
            self.owner.observe_images(self.report)
        self.assertEqual(self.report["unavailable_process_images"], {})

    def test_success_requires_original_retained_lifetime(self):
        self.owner.processes.identity.return_value = (8, Path("app.exe"), 100)
        with self.assertRaises(RuntimeError):
            self.owner.observe_images(self.report)
        self.assertEqual(self.report["process_images"], {})

    def test_success_records_path_without_fabricating_missing_images(self):
        self.owner.observe_images(self.report)
        self.assertEqual(self.report["process_images"], {7: "app.exe"})
        self.assertEqual(self.report["unavailable_process_images"], {})
        self.owner.exit_codes.assert_not_called()


if __name__ == "__main__":
    unittest.main()

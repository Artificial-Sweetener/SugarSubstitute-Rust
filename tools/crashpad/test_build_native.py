# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Actual build orchestration rejects drift with every external build boundary faked."""
import json
from pathlib import Path, PureWindowsPath
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import build_native


class Source:
    def resolve(self):
        return PureWindowsPath(r"E:\reviewed-source")

    def __str__(self):
        return r"E:\reviewed-source"


class BuildTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.tools = self.root / "tools"
        self.tools.mkdir()
        for name in ("gn.exe", "ninja.exe"):
            (self.tools / name).write_bytes(name.encode())
        self.output = self.root / "out-msvc-x64"
        self.output.mkdir()
        for name in ("crashpad_handler.exe", "crashpad_handler.pdb",
                     "sugarsubstitute_crashpad_client.dll", "sugarsubstitute_crashpad_client.pdb"):
            (self.output / name).write_bytes(b"fake-compiler-output")
        instances = [{"installationPath": str(build_native.VS),
                      "catalog": {"productDisplayVersion": "17.13.6"}}]
        for target, value in (("GN_SHA", build_native.digest(self.tools / "gn.exe")),
                              ("NINJA_SHA", build_native.digest(self.tools / "ninja.exe"))):
            self.enterContext(patch.object(build_native, target, value))
        self.enterContext(patch.object(build_native.subprocess, "check_output", return_value=json.dumps(instances)))
        self.runner = self.enterContext(patch.object(build_native.subprocess, "run", return_value=subprocess.CompletedProcess([], 0)))
        self.enterContext(patch.object(build_native, "verify_environment"))
        self.enterContext(patch.object(build_native, "repair_output"))

    def inventory(self, changed):
        calls = []

        def verify(source, authored, downloads, destination):
            calls.append(destination)
            destination.write_bytes(b"after" if changed and len(calls) == 2 else b"before")

        self.enterContext(patch.object(build_native, "verify", side_effect=verify))
        return calls

    def test_source_drift_rejects_final_association_and_preserves_both_inventories(self):
        calls = self.inventory(True)
        with self.assertRaisesRegex(ValueError, "inputs changed"):
            build_native.run(Source(), self.root, self.tools, 1)
        self.assertEqual([path.read_bytes() for path in calls], [b"before", b"after"])
        self.assertEqual(self.runner.call_count, 2)
        self.assertFalse((self.root / "build-receipt.json").exists())

    def test_recipe_drift_rejects_even_when_native_sources_are_unchanged(self):
        self.inventory(False)
        original = build_native.digest
        observed = 0

        def digest(path):
            nonlocal observed
            if path.name == "build_native.py":
                observed += 1
                return "first" if observed == 1 else "changed"
            return original(path)

        with patch.object(build_native, "digest", side_effect=digest):
            with self.assertRaisesRegex(ValueError, "recipe changed"):
                build_native.run(Source(), self.root, self.tools, 1)
        self.assertFalse((self.root / "build-receipt.json").exists())

    def test_unchanged_inputs_bind_both_inventory_hashes_without_executing_runtime(self):
        self.inventory(False)
        build_native.run(Source(), self.root, self.tools, 1)
        receipt = json.loads((self.root / "build-receipt.json").read_text())
        self.assertEqual(receipt["source_inventory_before_sha256"], receipt["source_inventory_after_sha256"])
        self.assertFalse(receipt["runtime_executed"])
        self.assertEqual(len(receipt["files"]), 4)
        self.assertEqual(self.runner.call_count, 2)


if __name__ == "__main__":
    unittest.main()

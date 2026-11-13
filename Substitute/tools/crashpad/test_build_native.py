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

VERIFY_ENVIRONMENT = build_native.verify_environment


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

    def test_selected_parser_targets_require_all_actual_outputs(self):
        self.inventory(False)
        with self.assertRaisesRegex(ValueError, "parser test outputs"):
            build_native.run(Source(), self.root, self.tools, 1, parser_tests=True)
        command = self.runner.call_args_list[1].args[0]
        self.assertIn("finished_parser_tests", command)
        self.assertFalse((self.root / "build-receipt.json").exists())

    def test_parser_symbols_bind_receipt_without_running_test_or_runtime(self):
        self.inventory(False)
        for name in ("bounded_metadata_tests.exe", "bounded_metadata_tests.pdb",
                     "incident_bridge_tests.exe", "incident_bridge_tests.pdb"):
            (self.output / name).write_bytes(b"fake-parser-output")
        build_native.run(Source(), self.root, self.tools, 1, parser_tests=True)
        receipt = json.loads((self.root / "build-receipt.json").read_text())
        self.assertTrue(receipt["parser_tests_selected"])
        self.assertFalse(receipt["parser_tests_executed"])
        self.assertFalse(receipt["runtime_executed"])
        self.assertEqual(len(receipt["parser_test_files"]), 4)
        self.assertEqual(len(receipt["files"]), 4)
        self.assertEqual(self.runner.call_count, 2)

    def test_arbitrary_d_source_is_rejected_before_native_build(self):
        with self.assertRaisesRegex(ValueError, "authenticated bundle"):
            build_native.admitted_source(Path(r"D:\foreign\crashpad"), self.root)
        self.assertEqual(self.runner.call_count, 0)

    def test_finished_owner_outputs_are_required_and_not_executed(self):
        self.inventory(False)
        with self.assertRaisesRegex(ValueError, "finished owner outputs"):
            build_native.run(Source(), self.root, self.tools, 1, finished_tests=True)
        self.assertIn("finished_owner_tests", self.runner.call_args_list[1].args[0])
        self.assertFalse((self.root / "build-receipt.json").exists())

    def test_selected_toolchain_drives_instance_admission_environment_and_receipt(self):
        self.inventory(False)
        enterprise = Path(r"C:\Program Files\Microsoft Visual Studio\2022\Enterprise")
        toolchain = build_native.Toolchain(enterprise, "17.14.2", "14.44.35207", "10.0.26100.0")
        instances = [{"installationPath": str(enterprise), "catalog": {"productDisplayVersion": "17.14.2"}}]
        self.enterContext(patch.object(build_native.subprocess, "check_output", return_value=json.dumps(instances)))
        compiler = enterprise / "VC/Tools/MSVC/14.44.35207/bin/Hostx64/x64/cl.exe"
        original = Path.is_file
        self.enterContext(patch.object(Path, "is_file", lambda path: path == compiler or original(path)))
        build_native.run(Source(), self.root, self.tools, 1, toolchain=toolchain)
        environment = self.runner.call_args_list[0].kwargs["env"]
        self.assertEqual(environment["SUGAR_CRASHPAD_VS"], str(enterprise))
        self.assertEqual(environment["SUGAR_CRASHPAD_SDK"], "10.0.26100.0")
        self.assertEqual(environment["SUGAR_CRASHPAD_VCVARS"], "14.44")
        self.assertIn('win_toolchain_path="' + enterprise.as_posix() + '"', self.runner.call_args_list[0].args[0][-1])
        receipt = json.loads((self.root / "build-receipt.json").read_text())
        self.assertEqual((receipt["visual_studio"], receipt["msvc"], receipt["sdk"]), ("17.14.2", "14.44.35207", "10.0.26100.0"))
        self.assertEqual(receipt["visual_studio_path"], str(enterprise))

    def test_selected_instance_must_have_the_named_version(self):
        self.inventory(False)
        toolchain = build_native.REVIEWED._replace(version="17.13.7")
        with self.assertRaisesRegex(ValueError, "instance/version mismatch"):
            build_native.run(Source(), self.root, self.tools, 1, toolchain=toolchain)
        self.assertEqual(self.runner.call_count, 0)

    def test_explicit_bundle_root_admits_only_its_own_prepared_source(self):
        bundle = Path(r"D:\a\_temp\crashpad")
        with patch.object(build_native, "regular"):
            build_native.admitted_source(bundle / "prepared/crashpad", bundle / "native-build", bundle)
            with self.assertRaisesRegex(ValueError, "authenticated bundle"):
                build_native.admitted_source(bundle / "prepared/crashpad", bundle / "native-build")
            with self.assertRaisesRegex(ValueError, "authenticated bundle"):
                build_native.admitted_source(Path(r"D:\a\_temp\other\crashpad"), bundle / "native-build", bundle)

    def test_selected_environment_must_carry_the_selected_versions(self):
        toolchain = build_native.Toolchain(Path(r"C:\VS"), "17.14.2", "14.44.35207", "10.0.26100.0")
        temporary = self.root / "temp"
        temporary.mkdir()
        bin_path = r"C:\VS\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64"

        def write(include):
            block = "\0".join([f"INCLUDE={include}", f"LIB={include}", f"TEMP={temporary}", f"TMP={temporary}", f"PATH={bin_path}"])
            (self.output / "environment.amd64").write_text(block + "\0", encoding="utf-8")

        write(r"C:\VS\VC\Tools\MSVC\14.44.35207\include;C:\Kits\10\Include\10.0.26100.0\ucrt")
        VERIFY_ENVIRONMENT(self.output, temporary, toolchain)
        write(r"C:\VS\VC\Tools\MSVC\14.43.34808\include;C:\Kits\10\Include\10.0.26100.0\ucrt")
        with self.assertRaisesRegex(ValueError, "escaped the reviewed versions"):
            VERIFY_ENVIRONMENT(self.output, temporary, toolchain)
        path = self.output / "test.ninja"
        raw = b"build obj: cxx ../../prepared/crashpad/a.cc\n"
        path.write_bytes(raw)
        receipt = self.root / "same-drive.json"
        build_native.verify_same_drive_output(self.output, receipt)
        self.assertEqual(path.read_bytes(), raw)
        self.assertEqual(json.loads(receipt.read_text())["changes"], [])
        path.write_bytes(b"build obj: cxx ../../D:/escaped/a.cc\n")
        with self.assertRaisesRegex(ValueError, "relative drive"):
            build_native.verify_same_drive_output(self.output, receipt)


if __name__ == "__main__":
    unittest.main()

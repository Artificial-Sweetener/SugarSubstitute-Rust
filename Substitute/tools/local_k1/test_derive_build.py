# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Verify public build-inventory selection and canonical digest rejection."""
import unittest
import json
import os
from pathlib import Path
import subprocess
import tempfile
from unittest.mock import patch
import derive_build

class IdentityTests(unittest.TestCase):
    def digest(self, records, revision="0" * 40):
        anchor = b"a" * 411
        with patch.object(derive_build, "ANCHOR_ID", derive_build.hashlib.sha256(anchor).hexdigest()):
            return derive_build.identity(anchor, revision, records)

    def test_order_is_canonical_and_source_change_changes_identity(self):
        records = [("Cargo.toml", b"metadata"), ("crates/owner/src/lib.rs", b"code")]
        self.assertEqual(self.digest(records), self.digest(list(reversed(records))))
        self.assertNotEqual(self.digest(records), self.digest([("Cargo.toml", b"changed"), records[1]]))
        self.assertNotEqual(self.digest(records), self.digest(records, "1" * 40))
        self.assertEqual(len(self.digest(records)), 51)

    def test_duplicate_empty_foreign_and_derived_inputs_reject(self):
        for records in [[], [("Cargo.toml", b"a"), ("Cargo.toml", b"b")],
                        [("../outside.rs", b"a")], [(derive_build.DERIVED, b"a")]]:
            with self.subTest(records=records), self.assertRaises(ValueError):
                self.digest(records)

    def test_wrong_anchor_and_noncanonical_revision_reject(self):
        with self.assertRaises(ValueError):
            derive_build.identity(b"b" * 411, "0" * 40, [("Cargo.toml", b"a")])
        with self.assertRaises(ValueError):
            self.digest([("Cargo.toml", b"a")], "F" * 40)

    def test_generated_binary_private_and_third_party_inputs_are_excluded(self):
        for relative in [derive_build.DERIVED, "private/signer.rs", "third_party/vendor/src/lib.rs", "target/release/app.exe"]:
            self.assertFalse(derive_build.source_input(relative))
        for relative in ["Cargo.lock", "rust-toolchain.toml", "native/crashpad/BUILD.gn", "tools/local_k1/derive_build.py"]:
            self.assertTrue(derive_build.source_input(relative))

    def test_embedded_scripts_and_runtime_assets_are_selected(self):
        for relative in [
            "crates/sugarsubstitute-desktop/src/main_actor/panel/bridge.js",
            "crates/sugarsubstitute-desktop/src/main_actor/panel/bridge_protocol.js",
            "crates/sugarsubstitute-desktop/src/main_actor/render_preview/downloads.js",
            "crates/sugarsubstitute-desktop/tests/panel_bridge.mjs",
            "crates/sugarsubstitute-networking/worker/http.ts",
            "assets/windows/application.manifest", "assets/brand/wordmark.svg",
            "crates/sugarsubstitute-ui/assets/fluent/Language.svg",
            "crates/sugarsubstitute-desktop/assets/caption/close.coverage",
            "crates/sugarsubstitute-desktop/assets/cursors/qt-open-hand.cur",
            "crates/sugarsubstitute-splash/assets/caption/close-100.png",
            "crates/sugarsubstitute-splash/assets/poses/1.png",
        ]:
            with self.subTest(relative=relative):
                self.assertTrue(derive_build.source_input(relative))

    def test_unowned_asset_locations_and_nonruntime_asset_types_are_excluded(self):
        for relative in [
            "assets/screenshots/capture.png", "assets/windows/diagnostic.manifest",
            "assets/brand/draft.svg", "crates/owner/assets/capture.png",
            "crates/sugarsubstitute-desktop/assets/cursors/openhandcursor_32.png",
            "crates/sugarsubstitute-ui/assets/fluent/Language.svg.bak",
            "crates/sugarsubstitute-splash/assets/poses/capture.log",
        ]:
            with self.subTest(relative=relative):
                self.assertFalse(derive_build.source_input(relative))


class InventoryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir=os.environ.get("TEMP"))
        self.addCleanup(self.temporary.cleanup)
        self.parent = Path(self.temporary.name)
        self.root = self.parent / "repository"
        self.root.mkdir()
        self.git("init", "--quiet")
        self.git("config", "core.autocrlf", "false")
        self.git("config", "core.excludesFile", str(self.parent / "empty-excludes"))
        (self.parent / "empty-excludes").write_text("")
        self.git("config", "core.hooksPath", str(self.parent / "empty-hooks"))
        (self.parent / "empty-hooks").mkdir()
        for name in ("retained.rs", "deleted.rs"):
            (self.root / name).write_text("// actual tracked source\n")
        self.git("add", "--", "retained.rs", "deleted.rs")
        self.git("-c", "user.name=Build Inventory Fixture", "-c", "user.email=fixture@example.invalid",
                 "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "Actual inventory fixture")
        (self.root / "deleted.rs").unlink()
        self.untracked = self.root / "untracked.rs"
        self.untracked.write_text("// actual untracked source\n")
        self.anchor = self.parent / "anchor.blob"
        self.anchor.write_bytes(b"a" * 411)

    def git(self, *arguments):
        return subprocess.run(["git", "-C", str(self.root), *arguments],
                              check=True, capture_output=True)

    def produce(self, output_name="output"):
        with patch.object(derive_build, "ANCHOR_ID", derive_build.hashlib.sha256(self.anchor.read_bytes()).hexdigest()):
            return derive_build.produce(self.root, self.anchor, self.parent, output_name)

    def write_input(self, name, data):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)

    def receipt(self, output_name):
        output = self.produce(output_name)
        return json.loads((output / "build-receipt.json").read_text())

    def test_embedded_script_and_asset_edits_change_the_actual_git_inventory_identity(self):
        names = [
            "crates/sugarsubstitute-desktop/src/main_actor/panel/bridge.js",
            "crates/sugarsubstitute-desktop/src/main_actor/panel/bridge_protocol.js",
            "crates/sugarsubstitute-desktop/src/main_actor/render_preview/downloads.js",
            "assets/windows/application.manifest", "assets/brand/wordmark.svg",
            "crates/sugarsubstitute-ui/assets/fluent/Language.svg",
            "crates/sugarsubstitute-desktop/assets/caption/close.coverage",
            "crates/sugarsubstitute-desktop/assets/cursors/qt-open-hand.cur",
            "crates/sugarsubstitute-splash/assets/caption/close-100.png",
            "crates/sugarsubstitute-splash/assets/poses/1.png",
        ]
        for name in names:
            self.write_input(name, b"embedded original")
        previous = self.receipt("baseline")
        inventoried = {item["relative"] for item in previous["inputs"]}
        self.assertTrue(set(names).issubset(inventoried))
        for index, name in enumerate(names):
            with self.subTest(relative=name):
                self.write_input(name, b"embedded changed")
                current = self.receipt(f"changed-{index}")
                self.assertNotEqual(previous["build"], current["build"])
                previous = current

    def test_excluded_artifacts_and_local_state_do_not_change_identity(self):
        self.write_input(".gitignore", b"tools/local_k1/generated/\n*.local.json\n")
        baseline = self.receipt("baseline")
        excluded = [
            derive_build.DERIVED, "target/debug/build/input.json", "artifacts/capture.json",
            "private/signer.rs", "third_party/vendor/src/lib.rs", "state/session.json",
            "assets/screenshots/capture.png", "assets/windows/diagnostic.manifest",
            "crates/owner/settings.local.json", "tools/local_k1/generated/receipt.json",
        ]
        for name in excluded:
            self.write_input(name, b"unrelated artifact")
        self.assertEqual(baseline, self.receipt("artifacts-added"))
        for name in excluded:
            self.write_input(name, b"unrelated artifact changed")
        self.assertEqual(baseline, self.receipt("artifacts-changed"))

    def test_actual_git_deleted_path_excluded_retained_and_untracked_inventoried(self):
        output = self.produce()
        receipt = json.loads((output / "build-receipt.json").read_text())
        self.assertEqual([item["relative"] for item in receipt["inputs"]], ["retained.rs", "untracked.rs"])
        for item in receipt["inputs"]:
            data = (self.root / item["relative"]).read_bytes()
            self.assertEqual(item["bytes"], len(data))
            self.assertEqual(item["sha256"], derive_build.hashlib.sha256(data).hexdigest())

    def test_untracked_path_disappearing_after_git_inventory_still_fails_closed(self):
        regular_path = derive_build.regular_path

        def remove_before_validation(path, *, directory=False):
            if path == self.untracked:
                self.untracked.unlink()
            return regular_path(path, directory=directory)

        with patch.object(derive_build, "regular_path", side_effect=remove_before_validation):
            with self.assertRaises(FileNotFoundError):
                self.produce()
        self.assertFalse((self.parent / "output").exists())

if __name__ == "__main__":
    unittest.main()

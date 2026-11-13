# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Lane test entries can only add test selections, and changed paths select their packages."""
from pathlib import PurePosixPath
import unittest
import lane_tests

PACKAGES = {
    PurePosixPath("Substitute/crates/sugarsubstitute"): "sugarsubstitute",
    PurePosixPath("Substitute/crates/sugarsubstitute-contracts"): "sugarsubstitute-contracts",
    PurePosixPath("Substitute/crates/sugarsubstitute-execution"): "sugarsubstitute-execution",
    PurePosixPath("Substitute/tools/xtask"): "sugarsubstitute-xtask",
}
NAMES = set(PACKAGES.values())
COMPONENTS = """
[[components]]
id = "sugarcubes-contracts"
root = "Sugar/integrations/contracts"
consumers = ["crates/sugarsubstitute-contracts/src/panel/wire.rs", "crates/sugarsubstitute/src/lib.rs"]
"""


class EntryTests(unittest.TestCase):
    def test_entries_name_one_package_and_an_optional_plain_filter(self):
        selections = lane_tests.lane_entries(
            "# area tests\n\n-p sugarsubstitute-execution coordinator_entry::tests\n"
            "cargo test --locked -p sugarsubstitute-contracts\n", NAMES)
        self.assertEqual(selections, [
            ["test", "--locked", "-p", "sugarsubstitute-execution", "--all-features", "coordinator_entry::tests"],
            ["test", "--locked", "-p", "sugarsubstitute-contracts", "--all-features"],
        ])

    def test_entries_that_would_do_more_than_select_tests_are_rejected(self):
        for line in ("-p sugarsubstitute -- --ignored", "-p sugarsubstitute --skip slow", "--workspace",
                     "-p unknown-package", "-p sugarsubstitute a b", "-p sugarsubstitute $(whoami)",
                     "cargo test -p sugarsubstitute", "-p sugarsubstitute --exclude sugarsubstitute"):
            with self.assertRaises(lane_tests.SelectionError, msg=line):
                lane_tests.lane_entries(line, NAMES)

    def test_selections_are_bounded_and_deduplicated(self):
        repeated = lane_tests.lane_entries("-p sugarsubstitute\n-p sugarsubstitute\n", NAMES)
        self.assertEqual(len(lane_tests.unique(repeated)), 1)
        many = [["test", str(index)] for index in range(lane_tests.MAX_SELECTIONS + 1)]
        with self.assertRaises(lane_tests.SelectionError):
            lane_tests.unique(many)


class ChangedPathTests(unittest.TestCase):
    def test_changed_sources_select_their_nearest_package(self):
        consumers = lane_tests.component_consumers(COMPONENTS)
        selected = lane_tests.changed_packages(
            ["Substitute/crates/sugarsubstitute-execution/src/lib.rs", "Substitute/tools/xtask/src/main.rs",
             "Substitute/crates/sugarsubstitute/src/x.rs", ".github/workflows/sugar-quality.yml",
             "Substitute/Cargo.lock"], PACKAGES, consumers)
        self.assertEqual(selected, ["sugarsubstitute-execution", "sugarsubstitute-xtask", "sugarsubstitute"])

    def test_a_consumed_component_selects_every_consuming_package(self):
        consumers = lane_tests.component_consumers(COMPONENTS)
        selected = lane_tests.changed_packages(["Sugar/integrations/contracts/canvas.schema.json"], PACKAGES, consumers)
        self.assertEqual(selected, ["sugarsubstitute-contracts", "sugarsubstitute"])


if __name__ == "__main__":
    unittest.main()

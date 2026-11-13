# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Prove parity failures for missing controls, wrong animation and hidden alpha changes."""

from __future__ import annotations

import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path

import numpy as np
from PIL import Image

from tools.splash_parity.acceptance import decide, measure, owner_bounds
from tools.splash_parity.compare import compare


class AcceptanceTests(unittest.TestCase):
    """Exercise fixed reviewed expectations independently of production renderers."""

    def test_exact_pixels_and_sparse_single_level_rounding_pass(self) -> None:
        """A tiny quantization difference passes while broad rounding drift fails."""
        delta = np.zeros((32, 46, 4), dtype=np.int16)
        self.assertTrue(decide("close", measure(delta[:, :, :3], delta))["accepted"])
        delta[0, 0] = 1
        self.assertTrue(decide("close", measure(delta[:, :, :3], delta))["accepted"])
        delta[:, :, :] = 1
        self.assertFalse(decide("close", measure(delta[:, :, :3], delta))["accepted"])

    def test_every_owner_rejects_localized_pixel_regression(self) -> None:
        """The global frame average cannot hide even one corrupted owner pixel."""
        for expanded in (False, True):
            for owner in owner_bounds(expanded):
                with self.subTest(owner=owner, expanded=expanded):
                    delta = np.zeros((32, 46, 4), dtype=np.int16)
                    delta[4, 5, 0] = 2
                    self.assertFalse(decide(owner, measure(delta[:, :, :3], delta))["accepted"])

    def test_empty_region_is_invalid(self) -> None:
        """Empty measurements cannot become a parity pass through NaN arithmetic."""
        with self.assertRaisesRegex(ValueError, "empty"):
            measure(np.zeros((0, 0, 3)), np.zeros((0, 0, 4)))

    def test_expanded_character_region_includes_lower_strip(self) -> None:
        """Transformed character errors above and behind the panel remain visible to the owner."""
        self.assertEqual(owner_bounds(True)["character"], (83, 7, 470, 393))


class ComparisonTests(unittest.TestCase):
    """Compare controlled files and inspect the same receipts used by qualification."""

    def setUp(self) -> None:
        """Use an owned temporary directory; callers choose its storage drive."""
        self.temporary = tempfile.TemporaryDirectory(prefix="splash-parity-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.reference = self.root / "reference"
        self.candidate = self.root / "candidate"
        self.reference.mkdir()
        self.candidate.mkdir()
        self.scenario = {"name": "fixture", "theme": "dark", "expanded": False, "scale": 1.0}
        self.write_scenarios([self.scenario])
        self.image = Image.new("RGBA", (558, 558))
        # A distinct small glyph in each caption rectangle and asymmetric mascot.
        for bounds, color in (((480, 8, 490, 18), (255, 255, 255, 255)),
                              ((530, 8, 540, 18), (255, 255, 255, 255)),
                              ((100, 150, 250, 300), (240, 50, 90, 255))):
            self.image.paste(color, bounds)
        self.image.save(self.reference / "fixture.png")

    def write_scenarios(self, scenarios: list[dict[str, object]]) -> None:
        """Write the production-shaped scenario index."""
        (self.reference / "scenarios.json").write_text(json.dumps(scenarios), encoding="utf-8")

    def run_comparison(self, image: Image.Image) -> dict[str, object]:
        """Capture the decision while keeping noisy diagnostics out of test output."""
        image.save(self.candidate / "fixture.png")
        output = self.root / "comparison"
        with contextlib.redirect_stdout(io.StringIO()):
            accepted = compare(self.reference, self.candidate, output)
        receipt = json.loads((output / "comparison.json").read_text())[0]
        self.assertEqual(accepted, receipt["accepted"])
        return receipt

    def test_matching_renderer_passes(self) -> None:
        """The passing fixture verifies acceptance actually runs all owners."""
        receipt = self.run_comparison(self.image)
        self.assertTrue(receipt["accepted"])
        self.assertEqual(set(receipt["owners"]), set(owner_bounds(False)))

    def test_missing_each_control_fails_its_own_decision(self) -> None:
        """Neither caption icon can hide behind a frame or combined-controls average."""
        for owner in ("console_toggle", "close"):
            image = self.image.copy()
            image.paste((0, 0, 0, 0), owner_bounds(False)[owner])
            receipt = self.run_comparison(image)
            self.assertFalse(receipt["decisions"][owner]["accepted"])
            other = "close" if owner == "console_toggle" else "console_toggle"
            self.assertTrue(receipt["decisions"][other]["accepted"])

    def test_wrong_pose_or_static_animation_fails_character(self) -> None:
        """A mirrored asymmetric pose stands for wrong-pose and frozen-flip regressions."""
        receipt = self.run_comparison(self.image.transpose(Image.Transpose.FLIP_LEFT_RIGHT))
        self.assertFalse(receipt["decisions"]["character"]["accepted"])

    def test_alpha_change_hidden_on_theme_background_fails(self) -> None:
        """Alpha is observable even when both images composite to the same theme pixel."""
        self.image.putpixel((30, 500), (32, 32, 32, 255))
        self.image.save(self.reference / "fixture.png")
        image = self.image.copy()
        image.putpixel((30, 500), (0, 0, 0, 0))
        receipt = self.run_comparison(image)
        self.assertEqual(receipt["owners"]["status"]["maximum_channel_error"], 0)
        self.assertFalse(receipt["decisions"]["status"]["accepted"])
        with Image.open(self.root / "comparison" / "fixture-premultiplied-difference.png") as difference:
            self.assertEqual(difference.getpixel((30, 500)), 255)

    def test_missing_frame_and_wrong_dimensions_fail(self) -> None:
        """Unavailable or mismatched captures cannot silently reduce the scenario inventory."""
        with self.assertRaises(FileNotFoundError):
            with contextlib.redirect_stdout(io.StringIO()):
                compare(self.reference, self.candidate, self.root / "missing")
        with self.assertRaisesRegex(ValueError, "dimensions"):
            self.run_comparison(Image.new("RGBA", (100, 100)))

    def test_fractional_canvas_dimensions_use_native_positive_rounding(self) -> None:
        """976.5 device pixels must select 977 like Qt native geometry and Rust exports."""
        self.scenario["scale"] = 1.75
        self.write_scenarios([self.scenario])
        image = Image.new("RGBA", (977, 977))
        image.save(self.reference / "fixture.png")
        self.assertTrue(self.run_comparison(image)["accepted"])

    def test_duplicate_and_unsafe_scenarios_fail(self) -> None:
        """Reject ambiguous inventories and names escaping an artifact root."""
        self.image.save(self.candidate / "fixture.png")
        for names in (("fixture", "fixture"), ("../fixture",), ("",)):
            self.write_scenarios([dict(self.scenario, name=name) for name in names])
            with self.assertRaisesRegex(ValueError, "filenames"):
                compare(self.reference, self.candidate, self.root / "invalid")


if __name__ == "__main__":
    unittest.main()

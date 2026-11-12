# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Reject missing foreground and source changes while admitting legitimate native material variation."""

import unittest

from PIL import Image

from tools.splash_parity.composition import composite_decision, require_same_source, source_pixels


class WholeFrameCompositionTests(unittest.TestCase):
    """Exercise all alpha classes without masking any UI region."""

    def test_variable_underlay_preserves_full_source_over_bounds(self) -> None:
        """Material-only and partially transparent pixels may vary as the unchanged window moves."""
        source = bytes([0, 0, 0, 0, 70, 20, 30, 128, 217, 40, 80, 255])
        for material in (0, 32, 117, 255):
            image = Image.new("RGB", (3, 1))
            image.putdata([(material,) * 3,
                           tuple(channel + round(material * 127 / 255) for channel in (70, 20, 30)),
                           (217, 40, 80)])
            decision = composite_decision(source, 3, 1, image)
            self.assertTrue(decision["accepted"])
            self.assertEqual(decision["pixels_checked"], 3)
            self.assertEqual(decision["opaque_pixels"], 1)
            self.assertEqual(decision["partial_alpha_pixels"], 1)
            self.assertEqual(decision["native_material_pixels"], 1)

    def test_missing_opaque_control_cannot_pass_on_material_background(self) -> None:
        """The native backdrop cannot explain disappearance of an opaque glyph or mascot pixel."""
        decision = composite_decision(bytes([217, 217, 217, 255]), 1, 1, Image.new("RGB", (1, 1), (35, 31, 30)))
        self.assertFalse(decision["accepted"])
        self.assertEqual(decision["violating_pixels"], 1)
        self.assertEqual(decision["maximum_bound_violation"], 187)

    def test_opaque_pixel_one_level_mutation_is_rejected(self) -> None:
        """Opaque presentation has no material-dependent rounding allowance."""
        self.assertFalse(composite_decision(bytes([70, 20, 30, 255]), 1, 1,
                                            Image.new("RGB", (1, 1), (71, 20, 30)))["accepted"])

    def test_partial_alpha_outside_any_material_composite_is_rejected(self) -> None:
        """A changed partial foreground cannot exceed bounds justified by any native underlay."""
        decision = composite_decision(bytes([70, 20, 30, 128]), 1, 1, Image.new("RGB", (1, 1), (70, 200, 30)))
        self.assertFalse(decision["accepted"])
        self.assertEqual(decision["maximum_bound_violation"], 52)

    def test_complete_source_comparison_rejects_single_byte_change(self) -> None:
        """A restored or omitted UI source cannot satisfy byte-for-byte static admission."""
        baseline = bytes([70, 20, 30, 128])
        with self.assertRaisesRegex(ValueError, "differs"):
            require_same_source(bytes([70, 21, 30, 128]), baseline, 1, 1)
        self.assertEqual(len(require_same_source(baseline, baseline, 1, 1)), 64)

    def test_invalid_alpha_geometry_and_capture_cannot_be_hidden(self) -> None:
        """Zero-alpha nonzero color, truncated source and mismatched capture dimensions fail closed."""
        for raw, width, height in ((bytes([1, 0, 0, 0]), 1, 1), (b"", 1, 1), (b"", 0, 1)):
            with self.assertRaises(ValueError):
                source_pixels(raw, width, height)
        with self.assertRaises(ValueError):
            composite_decision(bytes([0, 0, 0, 0]), 1, 1, Image.new("RGB", (2, 1)))


if __name__ == "__main__":
    unittest.main()

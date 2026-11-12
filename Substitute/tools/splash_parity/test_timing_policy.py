# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Deterministic nominal, tolerance and malformed-input timing boundaries."""
import json
import math
import unittest

try:
    from .timing_policy import TIMING_PROFILES, assess_timing
except ImportError:
    from timing_policy import TIMING_PROFILES, assess_timing


class TimingPolicyTests(unittest.TestCase):
    def test_profile_inventory_and_exact_report_keys(self):
        self.assertEqual(TIMING_PROFILES, ("strict", "busy"))
        result = assess_timing(12, 25)
        self.assertEqual(result, {"profile": "strict", "observed_ms": 12,
                                  "nominal_limit_ms": 25, "margin_ms": 0.0,
                                  "effective_limit_ms": 25.0, "strict_passed": True,
                                  "accepted": True})

    def test_strict_input_callback_and_producer_limits_are_exclusive(self):
        for limit in (50, 4, 25):
            for observed, passed in ((math.nextafter(limit, -math.inf), True),
                                     (limit, False), (math.nextafter(limit, math.inf), False)):
                with self.subTest(limit=limit, observed=observed):
                    result = assess_timing(observed, limit)
                    self.assertEqual(result["strict_passed"], passed)
                    self.assertEqual(result["accepted"], passed)
                    self.assertEqual(result["margin_ms"], 0)

    def test_startup_inclusive_nominal_boundaries(self):
        for limit in (500, 750):
            for observed, passed in ((math.nextafter(limit, -math.inf), True), (limit, True),
                                     (math.nextafter(limit, math.inf), False)):
                with self.subTest(limit=limit, observed=observed):
                    result = assess_timing(observed, limit, inclusive=True)
                    self.assertEqual(result["strict_passed"], passed)
                    self.assertEqual(result["accepted"], passed)

    def test_busy_floor_and_fraction_limits_are_fixed(self):
        for nominal, margin in ((4, 1000 / 60), (25, 1000 / 60), (50, 1000 / 60),
                                (500, 125), (750, 187.5)):
            reports = [assess_timing(observed, nominal, profile="busy")
                       for observed in (0, nominal, nominal + margin, None, 999999)]
            for result in reports:
                with self.subTest(nominal=nominal, observed=result["observed_ms"]):
                    self.assertEqual(result["margin_ms"], margin)
                    self.assertEqual(result["effective_limit_ms"], nominal + margin)

    def test_busy_exclusive_nominal_and_effective_boundaries_remain_distinct(self):
        for nominal in (4, 25, 50, 500):
            effective = assess_timing(0, nominal, profile="busy")["effective_limit_ms"]
            cases = ((math.nextafter(nominal, -math.inf), True, True),
                     (nominal, False, True), (math.nextafter(nominal, math.inf), False, True),
                     (math.nextafter(effective, -math.inf), False, True),
                     (effective, False, False), (math.nextafter(effective, math.inf), False, False))
            for observed, strict, accepted in cases:
                with self.subTest(nominal=nominal, observed=observed):
                    result = assess_timing(observed, nominal, profile="busy")
                    self.assertEqual((result["strict_passed"], result["accepted"]),
                                     (strict, accepted))

    def test_busy_startup_effective_boundary_is_inclusive(self):
        for nominal in (500, 750):
            effective = nominal * 1.25
            for observed, accepted in ((math.nextafter(effective, -math.inf), True),
                                       (effective, True),
                                       (math.nextafter(effective, math.inf), False)):
                result = assess_timing(observed, nominal, profile="busy", inclusive=True)
                self.assertTrue(assess_timing(nominal, nominal, profile="busy",
                                             inclusive=True)["strict_passed"])
                self.assertFalse(result["strict_passed"])
                self.assertEqual(result["accepted"], accepted)

    def test_invalid_observations_are_nonaccepting_and_json_safe(self):
        for observed in (None, -1, -math.inf, math.inf, math.nan, True, False, "12", [], {},
                         10 ** 400):
            for profile in TIMING_PROFILES:
                for inclusive in (False, True):
                    with self.subTest(observed=observed, profile=profile, inclusive=inclusive):
                        result = assess_timing(observed, 50, profile=profile, inclusive=inclusive)
                        self.assertIsNone(result["observed_ms"])
                        self.assertFalse(result["strict_passed"])
                        self.assertFalse(result["accepted"])
                        json.dumps(result, allow_nan=False)

    def test_zero_and_numeric_primitives_preserve_valid_observations(self):
        for observed in (0, 0.0, 1, 1.5):
            result = assess_timing(observed, 25)
            self.assertEqual(result["observed_ms"], observed)
            self.assertTrue(result["accepted"])
            json.dumps(result, allow_nan=False)

    def test_invalid_configuration_is_rejected(self):
        for nominal in (None, 0, -1, math.inf, -math.inf, math.nan, True, False, "25", 10 ** 400):
            with self.subTest(nominal=nominal), self.assertRaises(ValueError):
                assess_timing(1, nominal)
        for profile in (None, True, "BUSY", " busy", "unknown", [], {}):
            with self.subTest(profile=profile), self.assertRaises(ValueError):
                assess_timing(1, 25, profile=profile)
        for inclusive in (None, 0, 1, "true"):
            with self.subTest(inclusive=inclusive), self.assertRaises(ValueError):
                assess_timing(1, 25, inclusive=inclusive)
        with self.assertRaises(ValueError):
            assess_timing(1, 1.7e308, profile="busy")


if __name__ == "__main__":
    unittest.main()

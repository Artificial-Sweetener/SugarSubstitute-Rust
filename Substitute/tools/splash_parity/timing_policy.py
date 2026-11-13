# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Fixed timing tolerances that preserve measured costs and nominal results.

The busy profile is a provisional tolerance, not a statistical confidence
interval. Its margin is declared independently of observations and sample counts.
"""
import math

TIMING_PROFILES = ("strict", "busy")


def _finite_number(value):
    """Accept only JSON numeric primitives with a finite floating-point value."""
    if type(value) not in (int, float):
        return False
    try:
        return math.isfinite(value)
    except OverflowError:
        return False


def assess_timing(observed_ms, nominal_ms, *, profile="strict", inclusive=False) -> dict:
    """Report nominal and effective acceptance for one millisecond observation.

    Strict has zero margin. Busy adds the larger of 25% of the nominal limit
    and one 60 Hz interval (1000/60 ms). Both results use ``<`` unless inclusive
    is true, which uses ``<=``. Observations never change the effective limit.

    Missing or invalid observations become None with both flags false, keeping
    the report JSON safe; consumers retain original raw fields separately.
    Valid observations are finite nonnegative exact int/float values, excluding
    bool. Nominal limits must be finite positive exact int/float values.
    Unsupported profiles, non-bool inclusivity and nonfinite effective limits
    raise ValueError. This function does not alter samples or measured costs.
    """
    if type(profile) is not str or profile not in TIMING_PROFILES:
        raise ValueError("Unsupported timing profile")
    if type(inclusive) is not bool:
        raise ValueError("Timing inclusivity must be bool")
    if not _finite_number(nominal_ms) or nominal_ms <= 0:
        raise ValueError("Nominal timing limit must be finite and positive")
    margin = 0.0 if profile == "strict" else max(nominal_ms * .25, 1000 / 60)
    effective = nominal_ms + margin
    if not math.isfinite(effective):
        raise ValueError("Effective timing limit must be finite")
    valid = _finite_number(observed_ms) and observed_ms >= 0
    strict_passed = valid and (observed_ms <= nominal_ms if inclusive else observed_ms < nominal_ms)
    accepted = valid and (observed_ms <= effective if inclusive else observed_ms < effective)
    return {"profile": profile, "observed_ms": observed_ms if valid else None,
            "nominal_limit_ms": nominal_ms, "margin_ms": margin,
            "effective_limit_ms": effective, "strict_passed": strict_passed,
            "accepted": accepted}

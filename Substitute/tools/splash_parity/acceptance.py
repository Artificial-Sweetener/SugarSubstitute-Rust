# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Decide full-frame parity without masking controls or moving animation pixels."""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np


@dataclass(frozen=True)
class RegionExpectation:
    """Allow only single-level premultiplied raster rounding in a named owner."""

    maximum_error: int = 1
    mean_error: float = 0.01
    maximum_changed_fraction: float = 0.01


# These limits express pixel parity, including alpha, rather than renderer identity.
# Each control gets its own decision so a missing glyph cannot dilute into chrome.
EXPECTATIONS = {
    name: RegionExpectation()
    for name in ("frame", "character", "console_toggle", "close", "progress", "console", "status")
}


def owner_bounds(expanded: bool) -> dict[str, tuple[int, int, int, int]]:
    """Cover the whole frame and every visual owner, including the expanded mascot overlap."""
    bounds = {
        "frame": (0, 0, 558, 558),
        "character": (83, 7, 470, 393) if expanded else (85, 86, 472, 472),
        "console_toggle": (466, 0, 512, 32),
        "close": (512, 0, 558, 32),
        "progress": (24, 356, 534, 362) if expanded else (24, 530, 534, 534),
    }
    bounds["console" if expanded else "status"] = (
        (24, 362, 534, 534) if expanded else (24, 486, 534, 530)
    )
    return bounds


def measure(composited: np.ndarray, premultiplied: np.ndarray) -> dict[str, float | int]:
    """Keep composited and alpha-aware evidence; acceptance uses all four premultiplied channels."""
    if composited.size == 0 or premultiplied.size == 0:
        raise ValueError("Parity owner region is empty")
    pixels = int(composited.shape[0] * composited.shape[1])
    return {
        "mean_channel_error": float(composited.mean()),
        "maximum_channel_error": int(composited.max()),
        "different_pixels": int(np.any(composited != 0, axis=2).sum()),
        "pixels": pixels,
        "mean_premultiplied_error": float(premultiplied.mean()),
        "maximum_premultiplied_error": int(premultiplied.max()),
        "different_premultiplied_pixels": int(np.any(premultiplied != 0, axis=2).sum()),
    }


def decide(owner: str, metrics: dict[str, float | int]) -> dict[str, object]:
    """Fail on amplitude, aggregate or coverage regressions; never derive limits from a candidate."""
    expectation = EXPECTATIONS[owner]
    reasons = []
    if metrics["maximum_premultiplied_error"] > expectation.maximum_error:
        reasons.append("premultiplied channel error exceeds single-level raster rounding")
    if metrics["mean_premultiplied_error"] > expectation.mean_error:
        reasons.append("mean premultiplied error exceeds owner expectation")
    fraction = metrics["different_premultiplied_pixels"] / metrics["pixels"]
    if fraction > expectation.maximum_changed_fraction:
        reasons.append("changed pixel coverage exceeds owner expectation")
    return {
        "accepted": not reasons,
        "reasons": reasons,
        "limits": {
            "maximum_premultiplied_error": expectation.maximum_error,
            "mean_premultiplied_error": expectation.mean_error,
            "maximum_changed_fraction": expectation.maximum_changed_fraction,
        },
    }

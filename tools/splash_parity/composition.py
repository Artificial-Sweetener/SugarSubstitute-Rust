# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Check every captured pixel against its retained premultiplied foreground and variable native material."""

import hashlib

import numpy as np
from PIL import Image


def source_pixels(raw: bytes, width: int, height: int) -> np.ndarray:
    """Require a bounded, complete RGBA foreground whose color channels never exceed alpha."""
    if not 0 < width <= 4096 or not 0 < height <= 4096 or len(raw) != width * height * 4:
        raise ValueError("Retained source requires complete bounded premultiplied RGBA geometry")
    pixels = np.frombuffer(raw, dtype=np.uint8).reshape(height, width, 4).astype(np.int16)
    if np.any(pixels[:, :, :3] > pixels[:, :, 3:4]):
        raise ValueError("Retained source has color channels exceeding premultiplied alpha")
    return pixels


def require_same_source(actual: bytes, expected: bytes, width: int, height: int) -> str:
    """Reject any full-source mismatch, including material areas, before admitting a static proof."""
    source_pixels(actual, width, height)
    source_pixels(expected, width, height)
    if actual != expected:
        raise ValueError("Presented full source differs from the admitted static frame")
    return hashlib.sha256(actual).hexdigest()


def composite_decision(raw: bytes, width: int, height: int, image: Image.Image) -> dict[str, object]:
    """Test all channels against source-over bounds for an arbitrary native underlay.

    Opaque source channels must match exactly. Fractional alpha admits one level of
    native compositing rounding. Fully transparent valid source admits every
    underlay color while remaining part of the immutable full-source comparison.
    """
    source = source_pixels(raw, width, height)
    if image.mode != "RGB" or image.size != (width, height):
        raise ValueError("Captured RGB dimensions must match the complete retained source")
    colors, alpha = source[:, :, :3], source[:, :, 3:4]
    partial = (alpha > 0) & (alpha < 255)
    lower = np.maximum(0, colors - partial)
    upper = np.minimum(255, colors + (255 - alpha) + partial)
    captured = np.asarray(image, dtype=np.int16)
    violation = np.maximum(np.maximum(lower - captured, captured - upper), 0)
    pixels = violation.max(axis=2)
    worst_y, worst_x = np.unravel_index(int(pixels.argmax()), pixels.shape)
    maximum = int(pixels.max())
    return {"accepted": maximum == 0, "pixels_checked": width * height,
            "opaque_pixels": int(np.count_nonzero(alpha == 255)),
            "partial_alpha_pixels": int(np.count_nonzero(partial)),
            "native_material_pixels": int(np.count_nonzero(alpha == 0)),
            "violating_pixels": int(np.count_nonzero(pixels)), "maximum_bound_violation": maximum,
            "worst_pixel": {"device_point": [int(worst_x), int(worst_y)],
                            "source_rgba": source[worst_y, worst_x].tolist(),
                            "captured_rgb": captured[worst_y, worst_x].tolist(),
                            "lower_rgb": lower[worst_y, worst_x].tolist(),
                            "upper_rgb": upper[worst_y, worst_x].tolist()} if maximum else None,
            "source_sha256": hashlib.sha256(raw).hexdigest(),
            "capture_sha256": hashlib.sha256(image.tobytes()).hexdigest()}

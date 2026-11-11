#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
#
#    This program is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU Affero General Public License for more details.
#
#    You should have received a copy of the GNU Affero General Public License
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.
"""Select the same visually dense banner regions as SugarSubstitute."""

from __future__ import annotations

import colorsys
from collections.abc import Callable
from dataclasses import dataclass

RgbSampler = Callable[[int, int], tuple[int, int, int]]


@dataclass(frozen=True, slots=True)
class CropRect:
    """Represent a crop independently of a Qt or Pillow image implementation."""

    left: int
    top: int
    width: int
    height: int


def choose_banner_crop(
    width: int, height: int, sample: RgbSampler, *, aspect: float = 4.8
) -> CropRect:
    """Score bounded candidate regions with Substitute's contrast and position policy."""
    if width <= 0 or height <= 0 or aspect <= 0:
        raise ValueError("Banner source and aspect must be positive.")
    crop_width = min(width, max(1, round(height * aspect)))
    crop_height = min(height, max(1, round(width / aspect)))
    candidates = tuple(
        CropRect(x, y, crop_width, crop_height)
        for y in _positions(height, crop_height)
        for x in _positions(width, crop_width)
    )
    return max(candidates, key=lambda rect: _score(width, height, sample, rect))


def _positions(size: int, crop_size: int) -> tuple[int, ...]:
    """Cover the available crop range with deterministic bounded candidates."""
    maximum = size - crop_size
    if maximum == 0:
        return (0,)
    steps = max(2, min(16, maximum // max(1, crop_size // 6) + 1))
    return tuple(
        sorted(
            {round(maximum * index / (steps - 1)) for index in range(steps)}
            | {maximum // 2}
        )
    )


def _brightness(rgb: tuple[int, int, int]) -> float:
    """Use the same perceived-brightness coefficients as Substitute."""
    red, green, blue = rgb
    return (red * 0.2126 + green * 0.7152 + blue * 0.0722) / 255


def _score(width: int, height: int, sample: RgbSampler, rect: CropRect) -> float:
    """Prefer detailed regions while gently favoring upper-middle image content."""
    columns = max(6, min(18, rect.width // 8))
    rows = max(6, min(18, rect.height // 8))
    values: list[float] = []
    saturation_total = 0.0
    edge_total = 0.0
    for row in range(rows):
        y = min(height - 1, rect.top + round((row + 0.5) * rect.height / rows))
        for column in range(columns):
            x = min(width - 1, rect.left + round((column + 0.5) * rect.width / columns))
            rgb = sample(x, y)
            brightness = _brightness(rgb)
            values.append(brightness)
            saturation_total += colorsys.rgb_to_hls(
                *(channel / 255 for channel in rgb)
            )[2]
            edge_total += (
                abs(brightness - _brightness(sample(min(width - 1, x + 1), y)))
                + abs(brightness - _brightness(sample(x, min(height - 1, y + 1))))
            ) / 2
    count = len(values)
    average = sum(values) / count
    contrast = sum(abs(value - average) for value in values) / count
    edge = edge_total / count
    center_x = (rect.left + (rect.width - 1) // 2) / max(1, width - 1)
    center_y = (rect.top + (rect.height - 1) // 2) / max(1, height - 1)
    center_prior = max(0.0, 1.0 - abs(center_x - 0.5) - abs(center_y - 0.5))
    upper_prior = max(0.0, 1.0 - abs(center_y - 0.42) * 2)
    flat_penalty = 0.35 if contrast < 0.025 and edge < 0.025 else 0.0
    return (
        edge * 3.2
        + contrast * 1.9
        + saturation_total / count * 0.65
        + center_prior * 0.32
        + upper_prior * 0.22
        - flat_penalty
    )

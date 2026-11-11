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
"""Interpret spatial measurements without mistaking fixed-size or anisotropic transforms for a scale."""

from __future__ import annotations

from .spatial_probe_values import SpatialProbeMeasurement


def infer_spatial_scale(
    inputs: tuple[tuple[int, int], tuple[int, int]],
    outputs: tuple[tuple[int, int], tuple[int, int]],
) -> SpatialProbeMeasurement:
    """Fit one multiplicative transform while allowing a pixel of rounding in each measured edge."""
    observations = [
        (source, target)
        for frame, result in zip(inputs, outputs, strict=True)
        for source, target in zip(frame, result, strict=True)
    ]
    if any(
        source <= 0 or target <= 0 or target > 16384 for source, target in observations
    ):
        raise ValueError("The measured output exceeds the supported spatial frame.")
    low = max((target - 1) / source for source, target in observations)
    high = min((target + 1) / source for source, target in observations)
    if low > high or high <= 0:
        raise ValueError(
            "This output does not have a uniform relative scale; enter a scale factor explicitly."
        )
    for axis in (0, 1):
        delta = inputs[1][axis] - inputs[0][axis]
        if delta <= 0 or outputs[1][axis] <= outputs[0][axis]:
            raise ValueError("The probe frames cannot distinguish relative scaling.")
        slope = (outputs[1][axis] - outputs[0][axis]) / delta
        intercept = outputs[0][axis] - slope * inputs[0][axis]
        if abs(intercept) > 3:
            raise ValueError(
                "This output adds or removes a fixed pixel region; enter a scale factor explicitly."
            )
    factor = sum(source * target for source, target in observations) / sum(
        source * source for source, _ in observations
    )
    if factor <= 0 or factor > 1000:
        raise ValueError("The measured scale is outside the supported range.")
    return SpatialProbeMeasurement(factor, inputs, outputs)

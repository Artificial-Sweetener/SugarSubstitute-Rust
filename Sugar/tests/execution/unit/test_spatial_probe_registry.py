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
"""Prove schema adaptation and shape-preserving bypasses use installed native identities."""

from __future__ import annotations

from types import SimpleNamespace

import pytest

from sugarcubes.execution.spatial_probe_registry import SpatialProbeRegistry
from sugarcubes.execution.spatial_measurement_node import SpatialProbeMeasurementNode


def test_native_and_v3_schemas_preserve_geometry_constraints() -> None:
    """Use native step values and only bypass verified native samplers, never a same-named override."""
    schemas: dict[str, object] = {
        "Sampler": SimpleNamespace(
            __module__="nodes",
            INPUT_TYPES=lambda: {"required": {"latent_image": ("LATENT",)}},
            RETURN_TYPES=("LATENT",),
        ),
        "V3": SimpleNamespace(
            GET_NODE_INFO_V1=lambda: {
                "input": {
                    "required": {"width": ("INT", {"min": 16, "max": 8192, "step": 8})}
                },
                "output": ["IMAGE"],
                "output_node": False,
            }
        ),
    }
    schemas["KSampler"] = schemas["Sampler"]
    registry = SpatialProbeRegistry(lambda: schemas)
    assert registry.contract("KSampler").identity_input == "latent_image"
    assert registry.contract("V3").dimension_limits == {"width": (16, 8192, 8)}
    assert registry.definition("V3")["input"] == {
        "required": {"width": ("INT", {"min": 16, "max": 8192, "step": 8})}
    }
    override = SpatialProbeRegistry(
        lambda: {
            "KSampler": SimpleNamespace(
                __module__="custom_nodes",
                INPUT_TYPES=lambda: {"required": {"latent_image": ("LATENT",)}},
                RETURN_TYPES=("LATENT",),
            )
        }
    )
    assert override.contract("KSampler").identity_input is None
    with pytest.raises(ValueError, match="not registered"):
        registry.contract("Missing")


def test_measurement_sink_emits_only_frame_metadata() -> None:
    """Keep native tensor shape inspection independent of torch and normal media output."""
    node = SpatialProbeMeasurementNode()
    assert node.measure(
        image_a=SimpleNamespace(shape=(1, 64, 128, 3)),
        image_b=SimpleNamespace(shape=(1, 128, 192, 3)),
    ) == {"ui": {"spatial_dimensions": [[128, 64], [192, 128]]}, "result": ()}
    with pytest.raises(ValueError, match="matched"):
        node.measure(image_a=SimpleNamespace(shape=(1, 64, 128, 3)))
    with pytest.raises(ValueError, match="budget"):
        node.measure(
            image_a=SimpleNamespace(shape=(1, 20000, 20000, 3)),
            image_b=SimpleNamespace(shape=(1, 128, 192, 3)),
        )

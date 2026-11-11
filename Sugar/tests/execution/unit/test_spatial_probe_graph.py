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
"""Prove isolated spatial probes retain actual graph transforms and never execute saving sinks."""

from __future__ import annotations

from copy import deepcopy
import pytest

from sugarcubes.execution.models import ApiPrompt
from sugarcubes.execution.spatial_probe_graph import (
    MEASUREMENT_CLASS,
    SpatialProbeGraph,
)
from sugarcubes.execution.spatial_probe_values import (
    SpatialFieldTarget,
    SpatialNodeContract,
)
from sugarcubes.execution.spatial_probe_measurement import infer_spatial_scale


def contracts(name: str) -> SpatialNodeContract:
    """Declare only native boundaries used by the isolated execution fixtures."""
    values = {
        "EmptyImage": SpatialNodeContract({}, ("IMAGE",)),
        "EmptyLatentImage": SpatialNodeContract({}, ("LATENT",)),
        "ImageScaleBy": SpatialNodeContract({"image": "IMAGE"}, ("IMAGE",)),
        "ImageScale": SpatialNodeContract(
            {"image": "IMAGE"}, ("IMAGE",), fixed_image_frame=True
        ),
        "PreviewImage": SpatialNodeContract({"images": "IMAGE"}, (), output_node=True),
        "SaveImage": SpatialNodeContract({"images": "IMAGE"}, (), output_node=True),
        "LoadImage": SpatialNodeContract({}, ("IMAGE", "MASK")),
        "VAEDecode": SpatialNodeContract(
            {"samples": "LATENT", "vae": "VAE"}, ("IMAGE",)
        ),
        "VAELoader": SpatialNodeContract({}, ("VAE",)),
        "KSampler": SpatialNodeContract(
            {"latent_image": "LATENT", "model": "MODEL"},
            ("LATENT",),
            identity_input="latent_image",
        ),
        "ModelLoader": SpatialNodeContract({}, ("MODEL",)),
    }
    if name not in values:
        raise ValueError("Unregistered fixture node")
    return values[name]


def fixture() -> tuple[ApiPrompt, SpatialProbeGraph]:
    """Build an ordinary image workflow with one input pair and two real output paths."""
    return {
        "1": {
            "class_type": "EmptyImage",
            "inputs": {"width": 1024, "height": 768, "batch_size": 4, "color": 0},
        },
        "2": {
            "class_type": "ImageScaleBy",
            "inputs": {"image": ["1", 0], "scale_by": 1.5},
        },
        "3": {
            "class_type": "SaveImage",
            "inputs": {"images": ["2", 0], "filename_prefix": "Authored"},
            "_meta": {"title": "Final <image>"},
        },
        "4": {"class_type": "PreviewImage", "inputs": {"images": ["1", 0]}},
    }, SpatialProbeGraph(contracts)


def test_probe_captures_real_paths_without_mutating_the_authored_graph() -> None:
    """Keep output ambiguity explicit and remove every normal image saving/preview sink."""
    prompt, graph = fixture()
    before = deepcopy(prompt)
    plan = graph.prepare(
        prompt,
        (SpatialFieldTarget("1", "width"),),
        (SpatialFieldTarget("1", "height"),),
    )
    assert [item.id for item in plan.outputs] == ["3/images", "4/images"]
    execution = graph.build(plan, "3/images")
    assert execution.input_sizes == ((128, 64), (192, 128))
    assert len(execution.prompt) == 5
    assert prompt == before
    assert plan.prompt == before
    classes = [node["class_type"] for node in execution.prompt.values()]
    assert "SaveImage" not in classes and "PreviewImage" not in classes
    assert classes.count(MEASUREMENT_CLASS) == 1
    sources = [
        node for node in execution.prompt.values() if node["class_type"] == "EmptyImage"
    ]
    assert [node["inputs"] for node in sources] == [
        {"width": 128, "height": 64, "batch_size": 1, "color": 0},
        {"width": 192, "height": 128, "batch_size": 1, "color": 0},
    ]


def test_probe_bypasses_shape_preserving_sampling_and_prunes_its_unused_model() -> None:
    """Measure actual VAE and downstream transforms without spending diffusion steps."""
    prompt: ApiPrompt = {
        "1": {
            "class_type": "EmptyLatentImage",
            "inputs": {"width": 1024, "height": 1024},
        },
        "2": {"class_type": "ModelLoader", "inputs": {}},
        "3": {
            "class_type": "KSampler",
            "inputs": {"latent_image": ["1", 0], "model": ["2", 0], "steps": 30},
        },
        "4": {"class_type": "VAELoader", "inputs": {}},
        "5": {
            "class_type": "VAEDecode",
            "inputs": {"samples": ["3", 0], "vae": ["4", 0]},
        },
        "6": {"class_type": "PreviewImage", "inputs": {"images": ["5", 0]}},
    }
    graph = SpatialProbeGraph(contracts)
    plan = graph.prepare(
        prompt,
        (SpatialFieldTarget("1", "width"),),
        (SpatialFieldTarget("1", "height"),),
    )
    execution = graph.build(plan, "6/images")
    assert not any(
        node["class_type"] in ("KSampler", "ModelLoader", "PreviewImage")
        for node in execution.prompt.values()
    )
    assert (
        sum(node["class_type"] == "VAEDecode" for node in execution.prompt.values())
        == 2
    )


def test_known_fixed_frame_source_does_not_load_its_full_size_image() -> None:
    """Use the documented fixed frame boundary rather than executing the original loader."""
    prompt, graph = fixture()
    prompt["0"] = {"class_type": "LoadImage", "inputs": {"image": "original.png"}}
    prompt["1"] = {
        "class_type": "ImageScale",
        "inputs": {"image": ["0", 0], "width": 1024, "height": 768},
    }
    plan = graph.prepare(
        prompt,
        (SpatialFieldTarget("1", "width"),),
        (SpatialFieldTarget("1", "height"),),
    )
    execution = graph.build(plan, "3/images")
    assert not any(
        node["class_type"] == "LoadImage" for node in execution.prompt.values()
    )


@pytest.mark.parametrize(
    "outputs",
    [
        ((256, 256), (256, 256)),
        ((1, 1), (1, 1)),
        ((64, 128), (128, 192)),
        ((160, 96), (224, 160)),
    ],
)
def test_measurement_rejects_fixed_resize_rotation_and_padding(
    outputs: tuple[tuple[int, int], tuple[int, int]],
) -> None:
    """Refuse transforms that cannot be represented by one relative scale."""
    with pytest.raises(ValueError):
        infer_spatial_scale(((128, 64), (192, 128)), outputs)


@pytest.mark.parametrize("factor", [0.5, 1, 1.5, 2, 4])
def test_measurement_reports_observed_uniform_scale(factor: float) -> None:
    """Derive the factor from actual frame sizes rather than fields named scale factor."""
    frames = ((128, 64), (192, 128))
    first = (round(frames[0][0] * factor), round(frames[0][1] * factor))
    second = (round(frames[1][0] * factor), round(frames[1][1] * factor))
    assert infer_spatial_scale(frames, (first, second)).scale == factor

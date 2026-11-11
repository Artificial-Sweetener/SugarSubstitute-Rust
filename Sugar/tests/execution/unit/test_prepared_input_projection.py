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
"""Prove prepared pixels replace only their captured sockets through one normal and Cube graph policy."""

from __future__ import annotations

from copy import deepcopy

import pytest

from sugarcubes.execution.models import (
    ApiPrompt,
    BoundaryBinding,
    ExecutionNodeOwner,
    ExecutionOutput,
    LoweringResult,
    NodeDefinitions,
)
from sugarcubes.execution.prepared_input_projection import PreparedInputProjector
from sugarcubes.execution.prepared_input_values import (
    PreparedInputAsset,
    parse_prepared_input_assets,
)

PREPARED_FILE = "sugarcubes/sources/source-00000000-0000-0000-0000-000000000001.png"
DEFINITIONS: NodeDefinitions = {
    "CustomMask": {"output": ["STRING", "MASK"]},
    "LoadImageMask": {"output": ["MASK"]},
    "LoadImage": {"output": ["IMAGE", "MASK"]},
}


def asset() -> PreparedInputAsset:
    """Capture one custom mask output after its own inversion and decoding settings."""
    return PreparedInputAsset(
        "mask", "CustomMask", "file", "original.png", 1, "mask", PREPARED_FILE
    )


def prompt() -> ApiPrompt:
    """Include spatial consumers, an auxiliary output and a native wrapped literal."""
    return {
        "mask": {
            "class_type": "CustomMask",
            "inputs": {"file": "original.png", "invert": True},
        },
        "image": {"class_type": "LoadImage", "inputs": {"image": "image.png"}},
        "inpaint": {
            "class_type": "Inpaint",
            "inputs": {"mask": ["mask", 1], "pixels": ["image", 0]},
        },
        "aux": {
            "class_type": "StringConsumer",
            "inputs": {"text": ["mask", 0], "literal": {"__value__": ["mask", 1]}},
        },
    }


def test_prepared_mask_is_not_interpreted_twice_and_auxiliary_outputs_remain_native() -> (
    None
):
    """Keep the original custom loader intact while consumers receive the effective processed coverage."""
    source = prompt()
    original = deepcopy(source)
    projected = PreparedInputProjector().project(source, (asset(),), DEFINITIONS)
    replacement = projected.replacements[ExecutionOutput("mask", 1)]
    assert projected.prompt[replacement.node_id] == {
        "class_type": "LoadImageMask",
        "inputs": {"image": PREPARED_FILE, "channel": "alpha"},
    }
    assert projected.prompt["inpaint"]["inputs"] == {
        "mask": [replacement.node_id, 0],
        "pixels": ["image", 0],
    }
    assert projected.prompt["mask"] == original["mask"]
    assert projected.prompt["aux"] == original["aux"]
    assert source == original
    assert (
        PreparedInputProjector()
        .project(projected.prompt, (asset(),), DEFINITIONS)
        .prompt
        == projected.prompt
    )


def test_compiled_aliases_require_unambiguous_frozen_file_evidence() -> None:
    """Resolve compiler paths without confusing two loaders of the same class and file."""
    source = prompt()
    source["compiled-mask"] = source.pop("mask")
    owner = PreparedInputProjector()
    result = owner.project(source, (asset(),), DEFINITIONS)
    assert result.origins and set(result.origins.values()) == {"compiled-mask"}
    source["other-mask"] = deepcopy(source["compiled-mask"])
    with pytest.raises(ValueError, match="one captured loader"):
        owner.project(source, (asset(),), DEFINITIONS)


def test_lowering_preserves_cube_ownership_and_retargets_exposed_mask_outputs() -> None:
    """Make direct Cube outputs use the prepared socket and protect their identities from interning."""
    source = prompt()
    owners = {node: ExecutionNodeOwner("cube") for node in source}
    binding = BoundaryBinding(
        "cube", "mask", "output", "mask", output_index=1, value_type="MASK"
    )
    lowered = LoweringResult(source, owners, (binding,), DEFINITIONS)
    prepared = PreparedInputProjector().lower(lowered, (asset(),))
    target = prepared.lowered.boundary_bindings[0]
    assert target.node_id != binding.node_id and target.output_index == 0
    assert prepared.lowered.node_owners[target.node_id] == owners["mask"]
    assert {target.node_id, "mask"} <= prepared.protected_nodes
    assert lowered.boundary_bindings == (binding,)
    assert PreparedInputProjector().lower(lowered, ()).lowered is lowered


def test_wrong_socket_or_unavailable_decoder_fails_before_queueing() -> None:
    """Reject stale native definitions instead of silently replacing a nonmask output."""
    with pytest.raises(ValueError, match="socket"):
        PreparedInputProjector().project(
            prompt(), (asset(),), {"CustomMask": {"output": ["MASK", "STRING"]}}
        )
    with pytest.raises(ValueError, match="definition"):
        PreparedInputProjector().project(prompt(), (asset(),))


def test_http_input_values_restrict_assets_to_owned_immutable_uploads() -> None:
    """Validate public transport without trusting dynamic names, kinds, slots or file paths."""
    value: dict[str, object] = {
        "nodePath": "mask",
        "classType": "CustomMask",
        "field": "file",
        "file": "original.png",
        "slot": 1,
        "kind": "mask",
        "preparedFile": PREPARED_FILE,
    }
    assert parse_prepared_input_assets([value]) == (asset(),)
    assert parse_prepared_input_assets(None) == ()
    for invalid in (
        {"preparedFile": "../secret.png"},
        {"preparedFile": "C:\\secret.png"},
        {"slot": True},
        {"kind": "other"},
        {"classType": "bad\nclass"},
    ):
        with pytest.raises(ValueError):
            parse_prepared_input_assets([{**value, **invalid}])
    with pytest.raises(ValueError, match="competing"):
        parse_prepared_input_assets([value, value])

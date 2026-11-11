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
"""Prove source classification reuses graph order and never promotes arbitrary custom savers."""

from copy import deepcopy

from sugarcubes.execution.coordinator import CubeExecutionCoordinator
from sugarcubes.execution.models import (
    ApiPrompt,
    CubeExecutionRequest,
    ExecutionNodeOwner,
)
from sugarcubes.output_saving.domain.run_plan import SourceKind
from sugarcubes.output_saving.domain.saver_contract import ImageSaverContract
from sugarcubes.output_saving.infrastructure.source_inventory import (
    ComfySavingSourceInventory,
)
from tests.execution.support.execution_fixtures import cube_workflow, provider_document


def test_source_inventory_uses_shared_stack_projection_not_prompt_or_node_array_order() -> (
    None
):
    """Retain muted policy and disabled-cube ordinal positions without mutating saved graphs."""
    workflow = cube_workflow(
        {
            "a": provider_document("A"),
            "b": provider_document("B"),
            "c": provider_document("C"),
        },
        modes={"b": 2},
    )
    nodes = workflow["nodes"]
    assert isinstance(nodes, list)
    for index, node in enumerate(nodes):
        assert isinstance(node, dict)
        node["pos"] = [index * 500, 0]
    last = nodes[-1]
    assert isinstance(last, dict)
    properties = last["properties"]
    assert isinstance(properties, dict)
    properties["sugarcubes_save_outputs"] = False
    workflow["nodes"] = list(reversed(nodes))
    original = deepcopy(workflow)
    prepared = CubeExecutionCoordinator().prepare(
        CubeExecutionRequest(workflow=workflow)
    )
    inventory = ComfySavingSourceInventory(
        qualified_class=lambda _: None, image_output_class=lambda _: False
    ).collect(prepared.prompt, workflow=workflow, owners=prepared.node_owners)
    assert inventory.active_cube_ids == ("a", "c")
    cubes = {
        source.cube_id: source
        for source in inventory.sources
        if source.kind is SourceKind.CUBE
    }
    assert cubes["a"].cube_number == 1
    assert cubes["c"].cube_number == 3
    assert not cubes["c"].enabled
    assert workflow == original


def test_custom_image_output_remains_unmanaged_and_ordinary_labels_are_literal() -> (
    None
):
    """Qualify exact built-ins and retain per-workflow ordinary-source choices separately."""
    prompt: ApiPrompt = {
        "sampler": {"class_type": "Sampler", "inputs": {"seed": 18319146648460766796}},
        "save": {"class_type": "SaveImage", "inputs": {"images": ["sampler", 0]}},
        "custom": {"class_type": "CustomSaver", "inputs": {"images": ["sampler", 0]}},
    }
    workflow = {
        "nodes": [
            {
                "id": "save",
                "title": "Original",
                "properties": {
                    "sugarcubes_save_outputs": False,
                    "sugarcubes_output_label": "<literal output>",
                },
            }
        ]
    }
    inventory = ComfySavingSourceInventory(
        qualified_class=lambda name: (
            ImageSaverContract(name, "images", SourceKind.SAVE)
            if name == "SaveImage"
            else None
        ),
        image_output_class=lambda name: name == "CustomSaver",
    ).collect(prompt, workflow=workflow)
    assert inventory.sources[0].label == "<literal output>"
    assert not inventory.sources[0].enabled
    assert inventory.sources[0].seed == "18319146648460766796"
    assert inventory.unmanaged_node_ids == ("custom",)


def test_standalone_legacy_output_marker_retains_native_preview_and_saving_behavior() -> (
    None
):
    """Keep the public marker API usable outside a qualified native Cube instance."""
    inventory = ComfySavingSourceInventory(
        qualified_class=lambda _: None, image_output_class=lambda _: False
    ).collect(
        {
            "legacy": {
                "class_type": "SugarCubes.CubeOutput",
                "inputs": {
                    "value": ["image", 0],
                    "cube_id": "legacy",
                    "default_alias": "Legacy",
                    "save_outputs": True,
                },
            }
        },
        workflow={"nodes": []},
    )
    assert inventory.sources == ()
    assert inventory.unmanaged_node_ids == ("legacy",)


def test_nested_source_choices_are_instance_scoped_and_unknown_savers_remain_native() -> (
    None
):
    """Apply portable source choices without editing shared graph definitions or qualifying custom writers."""
    prompt: ApiPrompt = {
        "a:save": {"class_type": "SaveImage", "inputs": {"images": ["image", 0]}},
        "b:save": {"class_type": "SaveImage", "inputs": {"images": ["image", 0]}},
        "custom": {"class_type": "CustomSaver", "inputs": {"images": ["image", 0]}},
    }
    workflow = {
        "nodes": [],
        "extra": {
            "sugarcubes_output_saving": {
                "source_preferences": {
                    "schema_version": 1,
                    "sources": {
                        "a:save": {"enabled": False, "label": "<first instance>"},
                        "b:save": {"enabled": True, "label": "Second instance"},
                        "custom": {"enabled": False},
                    },
                }
            }
        },
    }
    original = deepcopy(workflow)
    inventory = ComfySavingSourceInventory(
        qualified_class=lambda kind: (
            ImageSaverContract(kind, "images", SourceKind.SAVE)
            if kind == "SaveImage"
            else None
        ),
        image_output_class=lambda kind: kind == "CustomSaver",
    ).collect(prompt, workflow=workflow)
    assert [
        (source.node_id, source.enabled, source.label) for source in inventory.sources
    ] == [("a:save", False, "<first instance>"), ("b:save", True, "Second instance")]
    assert inventory.unmanaged_node_ids == ("custom",)
    assert workflow == original


def test_authored_internal_output_marker_delegates_durable_writes_to_public_cube_outputs() -> (
    None
):
    """Treat internal legacy marker image writes as transient without changing their resource passthrough."""
    workflow = cube_workflow({"a": provider_document("A")})
    prepared = CubeExecutionCoordinator().prepare(
        CubeExecutionRequest(workflow=workflow)
    )
    prompt = deepcopy(prepared.prompt)
    prompt["internal-marker"] = {
        "class_type": "SugarCubes.CubeOutput",
        "inputs": {
            "value": ["image", 0],
            "cube_id": "legacy",
            "default_alias": "Legacy",
            "save_outputs": True,
        },
    }
    owners = {
        **prepared.node_owners,
        "internal-marker": ExecutionNodeOwner("a"),
    }
    inventory = ComfySavingSourceInventory(
        qualified_class=lambda _: None, image_output_class=lambda _: False
    ).collect(prompt, workflow=workflow, owners=owners)
    source = next(
        source for source in inventory.sources if source.node_id == "internal-marker"
    )
    assert source.kind is SourceKind.INTERNAL
    assert inventory.contracts[source.node_id].image_input == "value"

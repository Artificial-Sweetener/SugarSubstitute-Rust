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

"""Prove a workflow whose Cubes are carried as documents is rebuilt as native Cubes."""

from __future__ import annotations

from collections.abc import Mapping
from copy import deepcopy
from dataclasses import dataclass, field
import json
from typing import Any

import pytest

from sugarcubes.authoring import (
    ComposedWorkflowAuthoringService,
    ComposedWorkflowImportError,
    ExactIntegerField,
    NativeWorkflowNormalizer,
)
from sugarcubes.cube_model import CubeDocument
from sugarcubes.workflow import ComposedValueMaterializer

SEED = 17_066_688_399_168_319_613
"""A 64-bit seed; a browser reads it as 17066688399168319488."""

GENERATE = "11111111-1111-4111-8111-111111111111"
REFINE = "22222222-2222-4222-8222-222222222222"
GENERATE_INSTANCE = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
REFINE_INSTANCE = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"


class _NoCatalog:
    """Refuse catalog lookups: every Cube here carries its own document."""

    def resolve(self, cube_id: str, version_pin: str | None) -> CubeDocument:
        """Fail any attempt to read a Cube from the library."""

        raise ValueError(f"Unexpected catalog lookup for '{cube_id}'")


@dataclass
class _Preparer:
    """Capture the documents handed to native import preparation."""

    documents: list[CubeDocument] = field(default_factory=list)

    def prepare(self, document: CubeDocument) -> Mapping[str, object]:
        """Return a small JSON-ready payload while retaining the document."""

        self.documents.append(document)
        return {"document": document.to_dict()}


def _document(name: str, *, refines: bool) -> dict[str, Any]:
    """Author one Cube document: a prompt and a sampler, optionally fed an image."""

    sampler_inputs: dict[str, Any] = {
        "seed": 0,
        "steps": 20,
        "sampler_name": "euler",
        "prompt": ["prompt", 0],
    }
    if refines:
        sampler_inputs["image"] = ["@binding", "input.image"]
    return {
        "cube_id": f"Example/Pack/{name}.cube",
        "version": "1.0.0",
        "metadata": {"default_alias": name},
        "implementation": {
            "nodes": {
                "prompt": {
                    "class_type": "PrimitiveStringMultiline",
                    "inputs": {"value": "" if refines else "a lighthouse at dusk"},
                },
                "sampler": {"class_type": "ExampleSampler", "inputs": sampler_inputs},
            },
            "inputs": (
                {"input.image": {"kind": "input", "targets": [["sampler", "image"]]}}
                if refines
                else {}
            ),
            "outputs": {"output.image": ["sampler", 0]},
            "layout": {},
            "definitions": {
                "PrimitiveStringMultiline": {
                    "input": {"required": {"value": ["STRING", {"multiline": True}]}},
                    "output": ["STRING"],
                },
                "ExampleSampler": {
                    "input": {
                        "required": {
                            "seed": ["INT", {"default": 0}],
                            "steps": ["INT", {"default": 20}],
                            "sampler_name": [["euler", "er_sde"]],
                            "prompt": ["STRING", {"forceInput": True}],
                        },
                        "optional": {"image": ["IMAGE"]},
                    },
                    "output": ["IMAGE"],
                },
            },
            "subgraphs": [],
        },
        "surface": {"default_flavor_id": "default", "controls": []},
        "flavors": {"authored": [{"id": "default", "name": "Default", "values": {}}]},
    }


def _instance(
    node_id: str, definition: str, instance: str, name: str, x: float, *, mode: int = 0
) -> dict[str, Any]:
    """Author one root Cube node the way the Python application saved it."""

    return {
        "id": node_id,
        "type": definition,
        "mode": mode,
        "pos": [x, 0.0],
        "size": [320.0, 200.0],
        "inputs": [],
        "outputs": [{"name": "output.image", "type": "IMAGE"}],
        "properties": {
            "sugarcubes_kind": "cube",
            "sugarcubes_cube": {
                "cube_id": f"Example/Pack/{name}.cube",
                "cube_version": "1.0.0",
                "default_alias": name,
                "instance_id": instance,
                "instance_alias": name,
            },
        },
    }


def _definition(definition: str, name: str, *, refines: bool) -> dict[str, Any]:
    """Author one Cube definition that holds a document and no native nodes."""

    return {
        "id": definition,
        "name": name,
        "nodes": [],
        "links": [],
        "inputs": [{"name": "input.image", "type": "IMAGE"}] if refines else [],
        "outputs": [{"name": "output.image", "type": "IMAGE"}],
        "extra": {
            "sugarcubes_kind": "cube",
            "sugarcubes_cube": {
                "cube_id": f"Example/Pack/{name}.cube",
                "cube_version": "1.0.0",
                "default_alias": name,
            },
            "sugarcubes_document": _document(name, refines=refines),
        },
    }


def _endpoint(instance: str, symbol: str, name: str) -> dict[str, str]:
    return {"instance_id": instance, "node_symbol": symbol, "input_name": name}


def _workflow() -> dict[str, Any]:
    """Author a two-Cube workflow: the refining Cube is stored first but drawn second."""

    return {
        "version": 0.4,
        "nodes": [
            _instance("node-refine", REFINE, REFINE_INSTANCE, "Refine", 344.0),
            _instance("node-generate", GENERATE, GENERATE_INSTANCE, "Generate", 0.0),
        ],
        "links": [],
        "definitions": {
            "subgraphs": [
                _definition(REFINE, "Refine", refines=True),
                _definition(GENERATE, "Generate", refines=False),
            ]
        },
        "extra": {
            "sugarcubes_composition": {
                "schema_version": 1,
                "value_relations": [
                    {
                        "relation_id": "link:prompt",
                        "owner": "substitute",
                        "kind": "field_link",
                        "source": _endpoint(GENERATE_INSTANCE, "prompt", "value"),
                        "targets": [_endpoint(REFINE_INSTANCE, "prompt", "value")],
                    },
                    {
                        "relation_id": "override:seed",
                        "owner": "substitute",
                        "kind": "global_override",
                        "override_key": "seed",
                        "value": SEED,
                        "targets": [
                            _endpoint(REFINE_INSTANCE, "sampler", "seed"),
                            _endpoint(GENERATE_INSTANCE, "sampler", "seed"),
                        ],
                    },
                ],
            }
        },
    }


def _service(preparer: _Preparer) -> ComposedWorkflowAuthoringService:
    return ComposedWorkflowAuthoringService(
        normalizer=NativeWorkflowNormalizer(_NoCatalog()),
        preparer=preparer,
    )


def test_cubes_are_rebuilt_in_stack_order_with_relations_applied() -> None:
    """Each Cube arrives holding the values the workflow ran with."""

    preparer = _Preparer()
    workflow = _workflow()

    rebuilt = _service(preparer).compile(workflow)

    assert [item.alias for item in rebuilt.plan.instances] == ["Generate", "Refine"]
    assert [item.instance_id for item in rebuilt.plan.instances] == [
        GENERATE_INSTANCE,
        REFINE_INSTANCE,
    ]
    generate, refine = (item.to_dict() for item in preparer.documents)
    assert generate["implementation"]["nodes"]["sampler"]["inputs"]["seed"] == SEED
    assert refine["implementation"]["nodes"]["sampler"]["inputs"]["seed"] == SEED
    assert (
        refine["implementation"]["nodes"]["prompt"]["inputs"]["value"]
        == "a lighthouse at dusk"
    )
    # The stack fed the second Cube's image input from the first Cube's image output.
    assert [
        (
            link.source_instance_id,
            link.source_binding,
            link.target_instance_id,
            link.target_binding,
        )
        for link in rebuilt.plan.connections
    ] == [(GENERATE_INSTANCE, "output.image", REFINE_INSTANCE, "input.image")]
    # The workflow passed in is never modified.
    assert workflow == _workflow()


def test_whole_numbers_a_browser_cannot_hold_are_reported_with_their_digits() -> None:
    """The page receives the digits it would otherwise lose."""

    rebuilt = _service(_Preparer()).compile(_workflow())

    assert rebuilt.exact_integers == (
        ExactIntegerField(GENERATE_INSTANCE, "sampler", "seed", str(SEED)),
        ExactIntegerField(REFINE_INSTANCE, "sampler", "seed", str(SEED)),
    )
    small = _workflow()
    small["extra"]["sugarcubes_composition"]["value_relations"][1]["value"] = 2**53 - 1
    assert _service(_Preparer()).compile(small).exact_integers == ()


def test_only_the_document_only_form_is_recognized() -> None:
    """A workflow the page saved, with native nodes, loads as it is."""

    recognizes = ComposedWorkflowAuthoringService.recognizes
    assert recognizes(_workflow())
    native = _workflow()
    native["definitions"]["subgraphs"][0]["nodes"] = [{"id": 1, "type": "KSampler"}]
    assert not recognizes(native)
    assert not recognizes({"nodes": [], "definitions": {"subgraphs": []}})
    assert not recognizes({"nodes": [{"id": 1, "type": "KSampler"}]})


def test_a_bypassed_cube_and_an_invalid_document_are_reported() -> None:
    """Bypass survives the rebuild; a broken document names its Cube."""

    bypassed = _workflow()
    bypassed["nodes"][0]["mode"] = 4
    rebuilt = _service(_Preparer()).compile(bypassed)
    assert [item.bypassed for item in rebuilt.plan.instances] == [False, True]

    broken = _workflow()
    del broken["definitions"]["subgraphs"][0]["extra"]["sugarcubes_document"][
        "implementation"
    ]
    with pytest.raises(ComposedWorkflowImportError):
        _service(_Preparer()).compile(broken)


def _rounded(workflow: dict[str, Any]) -> dict[str, Any]:
    """Pass a workflow through a browser's number type."""

    rounded: dict[str, Any] = json.loads(
        json.dumps(workflow), parse_int=lambda digits: int(float(digits))
    )
    return rounded


def _seeds(workflow: Mapping[str, Any]) -> list[object]:
    """Read every Cube sampler's seed from the workflow's documents."""

    return [
        definition["extra"]["sugarcubes_document"]["implementation"]["nodes"][
            "sampler"
        ]["inputs"]["seed"]
        for definition in workflow["definitions"]["subgraphs"]
    ]


def test_recorded_digits_restore_a_seed_the_page_rounded() -> None:
    """At execution the sampler receives the exact seed, not its rounding."""

    normalized = NativeWorkflowNormalizer(_NoCatalog()).normalize(_workflow())
    page = _rounded(ComposedValueMaterializer().materialize(normalized))
    relations = page["extra"]["sugarcubes_composition"]["value_relations"]
    relations.clear()
    relations.append(
        {
            "relation_id": "exact:seed",
            "kind": "exact_integer",
            "digits": str(SEED),
            "targets": [
                _endpoint(GENERATE_INSTANCE, "sampler", "seed"),
                _endpoint(REFINE_INSTANCE, "sampler", "seed"),
            ],
        }
    )
    assert _seeds(page) == [int(float(SEED))] * 2
    assert int(float(SEED)) != SEED

    assert _seeds(ComposedValueMaterializer().materialize(page)) == [SEED, SEED]

    # A seed the person changed afterwards is another number and is left alone.
    edited = deepcopy(page)
    edited["definitions"]["subgraphs"][0]["extra"]["sugarcubes_document"][
        "implementation"
    ]["nodes"]["sampler"]["inputs"]["seed"] = 1234
    assert _seeds(ComposedValueMaterializer().materialize(edited)) == [1234, SEED]

    # Digits that are not a whole number never reach a field.
    relations[0]["digits"] = "12e3"
    assert (
        _seeds(ComposedValueMaterializer().materialize(page)) == [int(float(SEED))] * 2
    )

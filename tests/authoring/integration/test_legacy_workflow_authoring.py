#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Characterize exact-version recovery of group-era Cube workflows."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass, field
from typing import Any

import pytest

from sugarcubes.authoring import (
    LegacyWorkflowAuthoringService,
    LegacyWorkflowImportError,
)
from sugarcubes.cube_model import CubeDocument


@dataclass(frozen=True)
class _Resolver:
    """Resolve only the exact historical Cube used by the fixture."""

    document: CubeDocument

    def resolve(self, cube_id: str, version_pin: str | None) -> CubeDocument:
        """Reject current-version fallback and mismatched historical pins."""

        if cube_id != self.document.cube_id or version_pin != self.document.version:
            raise ValueError(f"Unknown exact Cube '{cube_id}' at '{version_pin}'")
        return self.document


@dataclass
class _Preparer:
    """Capture reconciled documents at the canonical preparation boundary."""

    documents: list[CubeDocument] = field(default_factory=list)

    def prepare(self, document: CubeDocument) -> Mapping[str, object]:
        """Return a small JSON-ready payload while retaining the document."""

        self.documents.append(document)
        return {"document": document.to_dict()}


def test_anonymous_saved_values_bind_through_exact_historical_names() -> None:
    """Prevent the 047 sampler array from shifting onto newly inserted fields."""

    document = _historical_document()
    preparer = _Preparer()
    service = LegacyWorkflowAuthoringService(
        resolver=_Resolver(document),
        preparer=preparer,
    )
    workflow = _workflow(
        [
            9_508_555_241_579_063_000,
            30,
            5,
            "euler_ancestral",
            "normal",
            2,
        ]
    )

    plan = service.compile(workflow)

    assert len(plan.instances) == 1
    reconciled = preparer.documents[0].to_dict()
    sampler = reconciled["implementation"]["nodes"]["sampler"]
    assert sampler["inputs"] == {
        "seed": 9_508_555_241_579_063_000,
        "steps": 30,
        "cfg": 5,
        "sampler_name": "euler_ancestral",
        "scheduler": "normal",
        "batch_size": 2,
    }
    assert "width" not in sampler["inputs"]
    assert "height" not in sampler["inputs"]


def test_marker_links_recover_public_cube_connections_by_identity() -> None:
    """Preserve saved inter-Cube links through marker identities."""

    document = _historical_document()
    service = LegacyWorkflowAuthoringService(
        resolver=_Resolver(document),
        preparer=_Preparer(),
    )
    workflow = _workflow([1, 20, 4, "euler", "normal", 1])
    workflow["groups"].append(
        {
            "sugarcubes": {
                "managed": True,
                "cube_id": document.cube_id,
                "cube_version": document.version,
                "instance_id": "second",
                "instance_alias": "Second",
                "nodes": ["20"],
                "markers": {"inputs": ["201"], "outputs": ["202"]},
            }
        }
    )
    workflow["nodes"].extend(
        [
            _saved_sampler("20", [2, 21, 4.5, "dpmpp_2m", "karras", 1]),
            _marker("201", "input.image"),
            _marker("202", "output.image"),
        ]
    )
    workflow["links"] = [[1, 102, 0, 201, 0, "IMAGE"]]

    plan = service.compile(workflow)

    assert len(plan.connections) == 1
    connection = plan.connections[0]
    assert (
        connection.source_instance_id,
        connection.source_binding,
        connection.target_instance_id,
        connection.target_binding,
    ) == ("first", "output.image", "second", "input.image")


def test_missing_exact_version_fails_closed_before_preparation() -> None:
    """Never reinterpret anonymous values through a current node definition."""

    document = _historical_document()
    preparer = _Preparer()
    service = LegacyWorkflowAuthoringService(
        resolver=_Resolver(document),
        preparer=preparer,
    )
    workflow = _workflow([1, 20, 4, "euler", "normal", 1])
    workflow["groups"][0]["sugarcubes"]["cube_version"] = "9.9.9"

    with pytest.raises(LegacyWorkflowImportError, match="exact version"):
        service.compile(workflow)

    assert preparer.documents == []


def test_nested_047_wrapper_reconciles_added_dimensions_by_boundary_name() -> None:
    """Recover the exact 047 sampler values without shifting seed onto width."""

    document = _nested_document()
    preparer = _Preparer()
    service = LegacyWorkflowAuthoringService(
        resolver=_Resolver(document),
        preparer=preparer,
    )
    workflow: dict[str, object] = {
        "groups": [
            {
                "sugarcubes": {
                    "managed": True,
                    "cube_id": document.cube_id,
                    "cube_version": document.version,
                    "instance_id": "047",
                    "instance_alias": "047",
                    "nodes": ["1"],
                    "markers": {},
                }
            }
        ],
        "nodes": [
            {
                "id": 1,
                "type": "saved-clone",
                "inputs": [],
                "widgets_values": [
                    9_508_555_241_579_063_000,
                    30,
                    5,
                    "euler_ancestral",
                    "normal",
                    2,
                ],
                "properties": {
                    "sugarcubes_symbol": "sampler",
                    "sugarcubes_original_subgraph_id": "historical-sampler",
                },
            }
        ],
        "links": [],
        "definitions": {
            "subgraphs": [
                _sampler_subgraph(
                    "saved-clone",
                    width=1080,
                    height=1512,
                    seed=9_508_555_241_579_063_000,
                    steps=30,
                    cfg=5,
                    sampler_name="euler_ancestral",
                    scheduler="normal",
                    batch_size=2,
                    original_id="historical-sampler",
                )
            ]
        },
    }

    service.compile(workflow)

    sampler = preparer.documents[0].to_dict()["implementation"]["nodes"]["sampler"]
    assert sampler["inputs"] == {
        "width": 1080,
        "height": 1512,
        "seed": 9_508_555_241_579_063_000,
        "steps": 30,
        "cfg": 5,
        "sampler_name": "euler_ancestral",
        "batch_size": 2,
    }


def _workflow(values: list[object]) -> dict[str, Any]:
    """Build one compact group-era workflow with direct anonymous values."""

    document = _historical_document()
    return {
        "version": 0.4,
        "groups": [
            {
                "sugarcubes": {
                    "managed": True,
                    "cube_id": document.cube_id,
                    "cube_version": document.version,
                    "instance_id": "first",
                    "instance_alias": "First",
                    "nodes": ["10"],
                    "markers": {"inputs": ["101"], "outputs": ["102"]},
                }
            }
        ],
        "nodes": [
            _saved_sampler("10", values),
            _marker("101", "input.image"),
            _marker("102", "output.image"),
        ],
        "links": [],
    }


def _saved_sampler(node_id: str, values: list[object]) -> dict[str, object]:
    """Build the identity-carrying but name-less node shape from saved PNGs."""

    return {
        "id": node_id,
        "type": "KSampler",
        "inputs": [],
        "widgets_values": values,
        "properties": {"sugarcubes_symbol": "sampler"},
    }


def _marker(node_id: str, symbol: str) -> dict[str, object]:
    """Build one group-era boundary marker."""

    return {
        "id": node_id,
        "type": "SugarCubeMarker",
        "properties": {"sugarcubes_symbol": symbol},
    }


def _historical_document() -> CubeDocument:
    """Define the exact historical sampler layout without later dimensions."""

    return CubeDocument.from_dict(
        {
            "cube_id": "Artificial-Sweetener/Base-Cubes/Anima/Text to Image.cube",
            "version": "1.1.0",
            "metadata": {"default_alias": "Anima/Text to Image"},
            "implementation": {
                "nodes": {
                    "sampler": {
                        "class_type": "KSampler",
                        "inputs": {
                            "seed": 0,
                            "steps": 20,
                            "cfg": 4,
                            "sampler_name": "euler",
                            "scheduler": "normal",
                            "batch_size": 1,
                        },
                    }
                },
                "inputs": {},
                "outputs": {},
                "layout": {},
                "definitions": {
                    "KSampler": {
                        "input": {
                            "required": {
                                "seed": ["INT", {"default": 0}],
                                "steps": ["INT", {"default": 20}],
                                "cfg": ["FLOAT", {"default": 4}],
                                "sampler_name": [["euler", "euler_ancestral"]],
                                "scheduler": [["normal", "karras"]],
                                "batch_size": ["INT", {"default": 1}],
                            }
                        },
                        "output": ["LATENT"],
                    }
                },
                "subgraphs": [],
            },
            "surface": {"default_flavor_id": "default", "controls": []},
            "flavors": {
                "authored": [{"id": "default", "name": "Default", "values": {}}]
            },
        }
    )


def _nested_document() -> CubeDocument:
    """Build one Cube whose historical sampler gained width and height boundaries."""

    payload = _historical_document().to_dict()
    payload["cube_id"] = "Artificial-Sweetener/Base-Cubes/Anima/Nested.cube"
    payload["metadata"]["default_alias"] = "Nested"
    payload["implementation"]["nodes"] = {
        "sampler": {"class_type": "historical-sampler", "inputs": {}}
    }
    payload["implementation"]["subgraphs"] = [
        _sampler_subgraph(
            "historical-sampler",
            width=1024,
            height=1024,
            seed=0,
            steps=20,
            cfg=8,
            sampler_name="euler",
            scheduler="normal",
            batch_size=1,
        )
    ]
    payload["surface"]["controls"] = []
    return CubeDocument.from_dict(payload)


def _sampler_subgraph(
    definition_id: str,
    *,
    width: int,
    height: int,
    seed: int,
    steps: int,
    cfg: int,
    sampler_name: str,
    scheduler: str,
    batch_size: int,
    original_id: str | None = None,
) -> dict[str, object]:
    """Build a compact native sampler boundary with named inner targets."""

    names = [
        "width",
        "height",
        "seed",
        "steps",
        "cfg",
        "sampler_name",
        "scheduler",
        "batch_size",
    ]
    values = [width, height, seed, steps, cfg, sampler_name, scheduler, batch_size]
    result: dict[str, object] = {
        "id": definition_id,
        "nodes": [
            {
                "id": 10,
                "type": "KSampler",
                "inputs": [
                    {"name": name, "link": index + 1, "widget": {"name": name}}
                    for index, name in enumerate(names)
                ],
                "widgets_values": values,
            }
        ],
        "inputs": [
            {"name": name, "label": name, "linkIds": [index + 1]}
            for index, name in enumerate(names)
        ],
        "outputs": [],
        "links": [
            {
                "id": index + 1,
                "origin_id": -10,
                "target_id": 10,
                "target_slot": index,
            }
            for index, _name in enumerate(names)
        ],
    }
    if original_id is not None:
        result["extra"] = {"sugar": {"original_subgraph_id": original_id}}
    return result

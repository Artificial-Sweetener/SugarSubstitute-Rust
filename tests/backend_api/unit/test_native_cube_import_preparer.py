#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Protect default precedence at the native workflow preparation boundary."""

from __future__ import annotations

from collections.abc import Mapping
from typing import cast

from sugarcubes.backend.services import NativeCubeImportPreparerAdapter
from sugarcubes.cube_model import CubeDocument
from sugarcubes.importer import LoadedCube, PreparedImport


def test_cube_defaults_fill_missing_fields_without_overwriting_explicit_values() -> (
    None
):
    """Enforce explicit workflow/script value over Cube default over node default."""

    document = _document()
    prepared = PreparedImport(
        nodes=[
            {
                "symbol": "sampler",
                "class_type": "sampler-subgraph",
                "inputs": {"seed": 47},
            }
        ],
        markers=[],
        connections=[],
        layout=None,
        warnings=[],
        subgraphs=list(document.implementation.subgraphs),
    )
    adapter = NativeCubeImportPreparerAdapter(
        load_document=lambda _document: cast(LoadedCube, object()),
        prepare_import=lambda _loaded: prepared,
    )

    payload = adapter.prepare(document)

    nodes = payload["nodes"]
    assert isinstance(nodes, list)
    node = nodes[0]
    assert isinstance(node, Mapping)
    assert node["inputs"] == {"seed": 47, "steps": 30}


def test_cube_defaults_replace_portable_null_before_frontend_materialization() -> None:
    """Display the nested node default when a portable wrapper value is unset."""

    document = _document()
    prepared = PreparedImport(
        nodes=[
            {
                "symbol": "sampler",
                "class_type": "sampler-subgraph",
                "inputs": {"seed": None},
            }
        ],
        markers=[],
        connections=[],
        layout=None,
        warnings=[],
        subgraphs=list(document.implementation.subgraphs),
    )
    adapter = NativeCubeImportPreparerAdapter(
        load_document=lambda _document: cast(LoadedCube, object()),
        prepare_import=lambda _loaded: prepared,
    )

    payload = adapter.prepare(document)

    nodes = payload["nodes"]
    assert isinstance(nodes, list)
    node = nodes[0]
    assert isinstance(node, Mapping)
    assert node["inputs"] == {"seed": 123, "steps": 30}


def _document() -> CubeDocument:
    """Build one nested Cube whose wrapper owns two authored defaults."""

    subgraph = {
        "id": "sampler-subgraph",
        "name": "Sampler",
        "inputs": [
            {"name": "seed", "label": "Seed", "linkIds": [1], "type": "INT"},
            {"name": "steps", "label": "Steps", "linkIds": [2], "type": "INT"},
        ],
        "outputs": [],
        "nodes": [
            {
                "id": 10,
                "type": "SamplerNode",
                "inputs": [
                    {"name": "seed", "link": 1, "widget": {"name": "seed"}},
                    {"name": "steps", "link": 2, "widget": {"name": "steps"}},
                ],
                "widgets_values": [123, 30],
            }
        ],
        "links": [
            {
                "id": 1,
                "origin_id": -10,
                "origin_slot": 0,
                "target_id": 10,
                "target_slot": 0,
                "type": "INT",
            },
            {
                "id": 2,
                "origin_id": -10,
                "origin_slot": 1,
                "target_id": 10,
                "target_slot": 1,
                "type": "INT",
            },
        ],
    }
    return CubeDocument.from_dict(
        {
            "cube_id": "Artificial-Sweetener/Base-Cubes/test.cube",
            "version": "1.0.0",
            "metadata": {"default_alias": "test"},
            "implementation": {
                "nodes": {
                    "sampler": {
                        "class_type": "sampler-subgraph",
                        "inputs": {"seed": 47},
                    }
                },
                "inputs": {},
                "outputs": {},
                "layout": {},
                "definitions": {
                    "SamplerNode": {
                        "input": {
                            "required": {
                                "seed": ["INT", {"default": 123}],
                                "steps": ["INT", {"default": 30}],
                            }
                        },
                        "input_order": {"required": ["seed", "steps"]},
                        "output": ["LATENT"],
                    }
                },
                "subgraphs": [subgraph],
            },
            "surface": {"default_flavor_id": "default", "controls": []},
            "flavors": {
                "authored": [{"id": "default", "name": "Default", "values": {}}]
            },
        }
    )

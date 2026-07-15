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
"""Legacy-to-current migration coverage for SugarCubes."""

from __future__ import annotations

from typing import Any

import importlib.util
import json
from pathlib import Path
import sys

from sugarcubes.cube_model import CubeDocument, migrate_legacy_payload


def test_migrate_legacy_payload_synthesizes_default_authored_flavor() -> None:
    legacy = {
        "cube_id": "artificial-sweetener/base-cubes/text to image.cube",
        "version": "1.0.2",
        "description": "Text to image",
        "metadata": {"author": "Alice"},
        "nodes": {
            "vectorscopecc": {
                "class_type": "VectorscopeCC",
                "inputs": {
                    "brightness": -0.2,
                    "input_image": ["ksampler", 0],
                },
            },
            "ksampler": {
                "class_type": "KSampler",
                "inputs": {"cfg": 7},
            },
        },
        "inputs": {
            "input.image": {
                "kind": "input",
                "targets": [["vectorscopecc", "input_image"]],
            }
        },
        "outputs": {"output.image": "ksampler"},
        "definitions": {"VectorscopeCC": {}, "KSampler": {}},
        "layout": {"origin": [0, 0], "nodes": {}, "groups": []},
        "subgraphs": [],
    }

    document = migrate_legacy_payload(legacy)
    payload = document.to_dict()

    assert payload["cube_id"] == "artificial-sweetener/base-cubes/text to image.cube"
    assert payload["implementation"]["nodes"]["vectorscopecc"]["inputs"] == {
        "input_image": ["ksampler", 0]
    }
    assert (
        payload["implementation"]["nodes"]["vectorscopecc"]["label"] == "vectorscopecc"
    )
    assert payload["implementation"]["nodes"]["ksampler"]["inputs"] == {}
    assert payload["implementation"]["nodes"]["ksampler"]["label"] == "ksampler"
    assert payload["surface"]["default_flavor_id"] == "default"
    assert payload["surface"]["controls"] == [
        {
            "control_id": "ksampler.cfg",
            "symbol": "ksampler",
            "input_name": "cfg",
            "label": "cfg",
            "class_type": "KSampler",
            "value_type": "number",
        },
        {
            "control_id": "vectorscopecc.brightness",
            "symbol": "vectorscopecc",
            "input_name": "brightness",
            "label": "brightness",
            "class_type": "VectorscopeCC",
            "value_type": "number",
        },
    ]
    assert payload["flavors"]["authored"] == [
        {
            "id": "default",
            "name": "Default",
            "values": {
                "vectorscopecc.brightness": -0.2,
                "ksampler.cfg": 7,
            },
        }
    ]


def test_migrate_legacy_payload_preserves_widget_input_order_for_surface_controls() -> (
    None
):
    """Legacy migration keeps Comfy input order for persisted controls and values."""

    legacy = {
        "cube_id": "artificial-sweetener/base-cubes/text to image.cube",
        "version": "1.0.2",
        "nodes": {
            "latent_dimensions": {
                "class_type": "EmptyLatentImage",
                "inputs": {
                    "width": 1024,
                    "height": 768,
                    "batch_size": 2,
                },
            },
            "vectorscopecc": {
                "class_type": "VectorscopeCC",
                "inputs": {
                    "model": ["checkpoint_loader", 0],
                    "alt": False,
                    "brightness": 0.1,
                    "contrast": 0.2,
                    "saturation": 1.1,
                    "r": 0.3,
                    "g": 0.4,
                    "b": 0.5,
                    "method": "Straight",
                    "scaling": "Linear",
                },
            },
            "checkpoint_loader": {
                "class_type": "CheckpointLoaderSimple",
                "inputs": {
                    "ckpt_name": "demo.safetensors",
                },
            },
        },
    }

    payload = migrate_legacy_payload(legacy).to_dict()

    assert _control_input_names(payload, "latent_dimensions") == [
        "width",
        "height",
        "batch_size",
    ]
    assert _authored_value_input_names(payload, "latent_dimensions") == [
        "width",
        "height",
        "batch_size",
    ]
    assert _control_input_names(payload, "vectorscopecc") == [
        "alt",
        "brightness",
        "contrast",
        "saturation",
        "r",
        "g",
        "b",
        "method",
        "scaling",
    ]
    assert _authored_value_input_names(payload, "vectorscopecc") == [
        "alt",
        "brightness",
        "contrast",
        "saturation",
        "r",
        "g",
        "b",
        "method",
        "scaling",
    ]


def test_cube_document_round_trips_current_payload() -> None:
    payload = {
        "cube_id": "artificial-sweetener/base-cubes/text to image.cube",
        "version": "1.0.2",
        "description": "Text to image",
        "metadata": {"author": "Alice"},
        "implementation": {
            "nodes": {
                "vectorscopecc": {
                    "class_type": "VectorscopeCC",
                    "label": "Vectorscope",
                    "inputs": {},
                }
            },
            "inputs": {},
            "outputs": {},
            "layout": {},
            "definitions": {"VectorscopeCC": {}},
            "subgraphs": [],
        },
        "surface": {
            "default_flavor_id": "default",
            "controls": [
                {
                    "control_id": "vectorscopecc.brightness",
                    "symbol": "vectorscopecc",
                    "input_name": "brightness",
                    "label": "brightness",
                    "class_type": "VectorscopeCC",
                    "value_type": "number",
                }
            ],
        },
        "flavors": {
            "authored": [
                {
                    "id": "default",
                    "name": "Default",
                    "values": {"vectorscopecc.brightness": -0.2},
                }
            ]
        },
    }

    document = CubeDocument.from_dict(payload)

    assert document.to_dict() == payload


def test_cube_document_rejects_authored_values_for_unknown_surface_controls() -> None:
    payload = {
        "cube_id": "artificial-sweetener/base-cubes/text to image.cube",
        "version": "1.0.2",
        "implementation": {
            "nodes": {
                "vectorscopecc": {
                    "class_type": "VectorscopeCC",
                    "label": "vectorscopecc",
                    "inputs": {},
                }
            },
            "inputs": {},
            "outputs": {},
            "layout": {},
            "definitions": {"VectorscopeCC": {}},
            "subgraphs": [],
        },
        "surface": {
            "default_flavor_id": "default",
            "controls": [
                {
                    "control_id": "vectorscopecc.brightness",
                    "symbol": "vectorscopecc",
                    "input_name": "brightness",
                    "label": "brightness",
                    "class_type": "VectorscopeCC",
                    "value_type": "number",
                }
            ],
        },
        "flavors": {
            "authored": [
                {
                    "id": "default",
                    "name": "Default",
                    "values": {"vectorscopecc.contrast": 0.5},
                }
            ]
        },
    }

    try:
        CubeDocument.from_dict(payload)
    except ValueError as error:
        assert "unknown surface control" in str(error)
    else:
        raise AssertionError("Unknown authored flavor controls should be rejected")


def test_cube_document_rejects_surface_controls_without_labels() -> None:
    payload = _current_payload_with_surface_control(
        {
            "control_id": "vectorscopecc.brightness",
            "symbol": "vectorscopecc",
            "input_name": "brightness",
            "class_type": "VectorscopeCC",
            "value_type": "number",
        }
    )

    try:
        CubeDocument.from_dict(payload)
    except ValueError as error:
        assert "label" in str(error)
    else:
        raise AssertionError("Missing surface control labels should be rejected")


def test_cube_document_rejects_duplicate_surface_control_labels() -> None:
    payload = _current_payload_with_surface_control(
        {
            "control_id": "vectorscopecc.brightness",
            "symbol": "vectorscopecc",
            "input_name": "brightness",
            "label": "Tone",
            "class_type": "VectorscopeCC",
            "value_type": "number",
        },
        {
            "control_id": "vectorscopecc.contrast",
            "symbol": "vectorscopecc",
            "input_name": "contrast",
            "label": "Tone",
            "class_type": "VectorscopeCC",
            "value_type": "number",
        },
        authored_values={"vectorscopecc.brightness": 1, "vectorscopecc.contrast": 2},
    )

    try:
        CubeDocument.from_dict(payload)
    except ValueError as error:
        assert "Duplicate surface control label" in str(error)
    else:
        raise AssertionError("Duplicate surface control labels should be rejected")


def test_cube_document_rejects_duplicate_node_labels() -> None:
    payload = _current_payload_with_surface_control(
        {
            "control_id": "vectorscopecc.brightness",
            "symbol": "vectorscopecc",
            "input_name": "brightness",
            "label": "brightness",
            "class_type": "VectorscopeCC",
            "value_type": "number",
        }
    )
    payload["implementation"]["nodes"]["vectorscopecc"]["label"] = "Tone"
    payload["implementation"]["nodes"]["other"] = {
        "class_type": "Other",
        "label": "Tone",
        "inputs": {},
    }

    try:
        CubeDocument.from_dict(payload)
    except ValueError as error:
        assert "Duplicate implementation node label" in str(error)
    else:
        raise AssertionError("Duplicate implementation node labels should be rejected")


def test_cube_document_rejects_subgraph_interface_without_labels() -> None:
    payload = _current_payload_with_surface_control(
        {
            "control_id": "vectorscopecc.brightness",
            "symbol": "vectorscopecc",
            "input_name": "brightness",
            "label": "brightness",
            "class_type": "VectorscopeCC",
            "value_type": "number",
        }
    )
    payload["implementation"]["subgraphs"] = [
        {"id": "subgraph-a", "nodes": [], "inputs": [{"name": "value"}], "outputs": []}
    ]

    try:
        CubeDocument.from_dict(payload)
    except ValueError as error:
        assert "label" in str(error)
    else:
        raise AssertionError("Missing subgraph IO labels should be rejected")


def test_migration_tool_rewrites_file_without_creating_history_backup(
    tmp_path: Path,
) -> None:
    legacy_path = tmp_path / "demo.cube"
    legacy_path.write_text(
        json.dumps(
            {
                "cube_id": "artificial-sweetener/base-cubes/demo.cube",
                "version": "1.2.3",
                "nodes": {
                    "ksampler": {
                        "class_type": "KSampler",
                        "inputs": {"cfg": 7},
                    }
                },
            }
        ),
        encoding="utf-8",
    )
    tool = _load_migration_tool()

    result = tool.migrate_cube_file(legacy_path)
    migrated_payload = json.loads(legacy_path.read_text(encoding="utf-8"))
    assert result.status == "migrated"
    assert migrated_payload["flavors"]["authored"][0]["values"] == {"ksampler.cfg": 7}
    assert not (tmp_path / "_history").exists()


def test_migration_tool_reports_transform_failures_without_rewriting(
    tmp_path: Path,
) -> None:
    legacy_path = tmp_path / "broken.cube"
    legacy_path.write_text(
        json.dumps(
            {
                "cube_id": "artificial-sweetener/base-cubes/broken.cube",
                "version": "1.0.0",
                "nodes": [],
            }
        ),
        encoding="utf-8",
    )
    original_text = legacy_path.read_text(encoding="utf-8")
    tool = _load_migration_tool()

    result = tool.migrate_cube_file(legacy_path)

    assert result.status == "failed"
    assert legacy_path.read_text(encoding="utf-8") == original_text
    assert not (tmp_path / "_history").exists()


def _load_migration_tool() -> Any:
    """Load the migration script as a test module."""

    script_path = (
        Path(__file__).resolve().parents[1] / "scripts" / "migrate_legacy_cubes.py"
    )
    spec = importlib.util.spec_from_file_location("migrate_legacy_cubes", script_path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _current_payload_with_surface_control(
    *controls: Any, authored_values: Any = None
) -> dict[str, Any]:
    """Build a current-format payload for schema validation tests."""

    values = {"vectorscopecc.brightness": -0.2}
    if authored_values is not None:
        values = dict(authored_values)
    return {
        "cube_id": "artificial-sweetener/base-cubes/text to image.cube",
        "version": "1.0.2",
        "implementation": {
            "nodes": {
                "vectorscopecc": {
                    "class_type": "VectorscopeCC",
                    "label": "vectorscopecc",
                    "inputs": {},
                }
            },
            "inputs": {},
            "outputs": {},
            "layout": {},
            "definitions": {"VectorscopeCC": {}},
            "subgraphs": [],
        },
        "surface": {
            "default_flavor_id": "default",
            "controls": list(controls),
        },
        "flavors": {
            "authored": [{"id": "default", "name": "Default", "values": values}]
        },
    }


def _control_input_names(payload: Any, symbol: Any) -> Any:
    """Read persisted surface input names for one symbol."""

    return [
        control["input_name"]
        for control in payload["surface"]["controls"]
        if control["symbol"] == symbol
    ]


def _authored_value_input_names(payload: Any, symbol: Any) -> Any:
    """Read persisted authored flavor value names for one symbol."""

    prefix = f"{symbol}."
    values = payload["flavors"]["authored"][0]["values"]
    return [key.removeprefix(prefix) for key in values if key.startswith(prefix)]

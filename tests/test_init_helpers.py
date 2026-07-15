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
"""Backend helper characterization tests aligned to the tracked-repo model."""

from __future__ import annotations

from typing import Any

from .typing_support import BackendServicesFactory

import asyncio
import json
import logging
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest

from sugarcubes.backend import _resolve_active_comfy_node_class_mappings
from sugarcubes.backend.responses import BackendError
from sugarcubes.backend.services.cube_export_service import (
    collect_missing_node_class_types,
    collect_required_node_class_types,
    collect_subgraph_contract_violations,
)
from sugarcubes.backend.services.cube_summary import (
    derive_cube_display_name,
)
from sugarcubes.backend.services.cube_metadata import (
    normalize_default_alias,
    normalize_metadata_update,
    normalize_supported_models,
    normalize_tags,
)
from sugarcubes.backend.services.cube_version_artifact_cache import (
    CubeVersionArtifactCache,
)
from sugarcubes.backend.validation.request_parsers import (
    coerce_int,
    extract_drop_origin,
    normalize_workflow_payload,
    parse_json_body,
    parse_optional_json_body,
    parse_save_many_cube_entries,
)

from .conftest import FakeRequest, ensure_tracked_repo

CANONICAL_CUBE_ID = "artificial-sweetener/base-cubes/automask detailer.cube"


def test_entrypoint_import_does_not_load_comfy_host_modules() -> None:
    root = Path(__file__).resolve().parents[1]
    script = f"""
import importlib.util
import sys
from pathlib import Path

root = Path({str(root)!r})
spec = importlib.util.spec_from_file_location(
    'sugarcubes_entrypoint_test',
    root / '__init__.py',
    submodule_search_locations=[str(root)],
)
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
raise SystemExit(1 if 'server' in sys.modules or 'nodes' in sys.modules else 0)
"""

    result = subprocess.run(
        [sys.executable, "-c", script],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr


def test_normalize_tags_and_models() -> None:
    assert normalize_tags(["Foo Bar", "", 1]) == ["foo-bar"]
    assert normalize_tags("a, b c") == ["a", "bc"]
    assert normalize_supported_models([" sd ", "", 123]) == ["sd"]
    assert normalize_supported_models("sdxl, sd") == ["sdxl", "sd"]


@pytest.mark.parametrize(
    ("value", "expected"),
    [
        ("text to image", "text to image"),
        ("diffusion upscale", "diffusion upscale"),
        ("automask detailer", "automask detailer"),
        ("Text to Image", "Text to Image"),
        (" text to image ", "text to image"),
        ("text_to_image", "text_to_image"),
        ("sdxl lora image", "sdxl lora image"),
        ("IPAdapter helper", "IPAdapter helper"),
        ("Flux .1 D", "Flux .1 D"),
    ],
)
def test_normalize_default_alias_preserves_route_text(
    value: Any, expected: Any
) -> None:
    assert normalize_default_alias(value) == expected


def test_build_default_alias_lookup_reads_tracked_repo_display_name(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    checkout = ensure_tracked_repo(
        services,
        owner="artificial-sweetener",
        repo="base-cubes",
        default_base_repo=False,
    )
    cube_path = checkout / "automask detailer.cube"
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": CANONICAL_CUBE_ID,
                "version": "1.0.0",
                "implementation": {
                    "nodes": {},
                    "inputs": {},
                    "outputs": {},
                    "layout": {
                        "groups": [
                            {
                                "title": "automask detailer",
                                "sugarcubes": {
                                    "cube_id": CANONICAL_CUBE_ID,
                                    "default_alias": "automask detailer",
                                },
                            }
                        ]
                    },
                    "definitions": {},
                    "subgraphs": [],
                },
                "surface": {"default_flavor_id": "default", "controls": []},
                "flavors": {
                    "authored": [{"id": "default", "name": "Default", "values": {}}]
                },
            }
        ),
        encoding="utf-8",
    )

    lookup = services.library.build_default_alias_lookup([CANONICAL_CUBE_ID])

    assert lookup[CANONICAL_CUBE_ID] == "automask detailer"


def test_derive_cube_display_name_ignores_group_title_without_definition_alias() -> (
    None
):
    payload = {
        "implementation": {
            "layout": {
                "groups": [
                    {
                        "title": "Polluted Instance Alias",
                        "sugarcubes": {
                            "cube_id": CANONICAL_CUBE_ID,
                            "instance_alias": "Polluted Instance Alias",
                        },
                    }
                ]
            }
        }
    }

    assert derive_cube_display_name(payload, "automask detailer") == "automask detailer"


def test_extract_drop_origin() -> None:
    assert extract_drop_origin({"x": 1, "y": 2}) == [1, 2]
    assert extract_drop_origin([3, 4]) == [3, 4]
    assert extract_drop_origin("bad") is None


def test_normalize_metadata_update() -> None:
    updates, removals = normalize_metadata_update(
        {"author": "", "tags": "foo, bar", "supported_models": []}
    )
    assert updates["tags"] == ["foo", "bar"]
    assert "author" not in updates
    assert "author" not in removals
    assert "supported_models" in removals


def test_normalize_metadata_update_validates_target_model_against_cube_path() -> None:
    updates, _removals = normalize_metadata_update(
        {
            "target_model": "SDXL",
            "supported_models": ["SD 1.5"],
        },
        cube_id="artificial-sweetener/base-cubes/SDXL/Text to Image.cube",
    )

    assert updates["target_model"] == "SDXL"
    assert updates["supported_models"] == ["SDXL", "SD 1.5"]

    with pytest.raises(BackendError, match="metadata.target_model"):
        normalize_metadata_update(
            {"target_model": "Flux"},
            cube_id="artificial-sweetener/base-cubes/SDXL/Text to Image.cube",
        )
    with pytest.raises(BackendError, match="metadata.target_model is required"):
        normalize_metadata_update(
            {"target_model": ""},
            cube_id="artificial-sweetener/base-cubes/SDXL/Text to Image.cube",
        )


def test_parse_save_many_cube_entries_preserves_normalized_metadata() -> None:
    entries = parse_save_many_cube_entries(
        [
            {
                "cube_id": "artificial-sweetener/base-cubes/SDXL/Text to Image.cube",
                "metadata": {
                    "target_model": "SDXL",
                    "supported_models": "SD 1.5, SDXL",
                },
            }
        ]
    )

    assert entries["artificial-sweetener/base-cubes/SDXL/Text to Image.cube"][
        "metadata"
    ] == {
        "target_model": "SDXL",
        "supported_models": ["SDXL", "SD 1.5"],
    }


def test_normalize_workflow_payload_requires_value() -> None:
    with pytest.raises(Exception):
        normalize_workflow_payload(None)


def test_parse_json_body_requires_object() -> None:
    with pytest.raises(BackendError, match="Request body must be a JSON object"):
        asyncio.run(parse_json_body(FakeRequest(body=["bad"])))


def test_parse_optional_json_body_allows_empty_body() -> None:
    assert asyncio.run(parse_optional_json_body(FakeRequest(body=None))) is None


def test_parse_json_body_rejects_malformed_json() -> None:
    with pytest.raises(BackendError, match="Invalid JSON body"):
        asyncio.run(
            parse_json_body(FakeRequest(json_error=json.JSONDecodeError("bad", "", 0)))
        )


def test_coerce_int_preserves_supported_inputs() -> None:
    assert coerce_int(4) == 4
    assert coerce_int(4.8) == 4
    assert coerce_int("5") == 5
    assert coerce_int("bad", default=9) == 9


def test_collect_subgraph_contract_violations_for_uuid_wrappers() -> None:
    subgraph_id = "94f725d5-39bf-4060-be68-f573214a2055"
    graph = {
        "1": {"class_type": subgraph_id, "inputs": {}},
        "2": {"class_type": "KSampler", "inputs": {}},
    }
    workflow = {"definitions": {"subgraphs": [{"id": subgraph_id, "nodes": []}]}}
    violations = collect_subgraph_contract_violations(graph, workflow)
    assert violations == {"empty_subgraph_bodies": [subgraph_id]}


def test_collect_subgraph_contract_violations_reports_missing_definition() -> None:
    subgraph_id = "94f725d5-39bf-4060-be68-f573214a2055"
    graph = {"1": {"class_type": subgraph_id, "inputs": {}}}
    workflow: dict[str, Any] = {"definitions": {"subgraphs": []}}
    violations = collect_subgraph_contract_violations(graph, workflow)
    assert violations == {"missing_subgraphs": [subgraph_id]}


def test_collect_subgraph_contract_violations_reports_missing_labels() -> None:
    subgraph_id = "94f725d5-39bf-4060-be68-f573214a2055"
    graph = {"1": {"class_type": subgraph_id, "inputs": {}}}
    workflow = {
        "definitions": {
            "subgraphs": [
                {
                    "id": subgraph_id,
                    "nodes": [{"id": 1, "type": "KSampler"}],
                    "inputs": [{"name": "value"}],
                    "outputs": [],
                }
            ]
        }
    }
    violations = collect_subgraph_contract_violations(graph, workflow)
    assert violations == {"missing_subgraph_labels": [f"{subgraph_id}.inputs.value"]}


def test_collect_required_node_class_types_includes_subgraph_node_classes() -> None:
    subgraph_id = "94f725d5-39bf-4060-be68-f573214a2055"
    graph = {
        "1": {"class_type": "KSampler", "inputs": {}},
        "2": {"class_type": subgraph_id, "inputs": {}},
    }
    workflow = {
        "definitions": {
            "subgraphs": [
                {
                    "id": subgraph_id,
                    "nodes": [
                        {"id": 101, "type": "RegexExtract"},
                        {"id": 102, "type": subgraph_id},
                    ],
                }
            ]
        }
    }

    required = collect_required_node_class_types(graph, workflow)
    assert required == {"KSampler", "RegexExtract"}


def test_collect_missing_node_class_types_uses_registry_mapping() -> None:
    missing = collect_missing_node_class_types(
        {"KSampler", "RegexExtract"}, {"KSampler": object()}
    )
    assert missing == ["RegexExtract"]


def test_resolve_active_comfy_node_class_mappings_prefers_host_registry(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    extension_root = Path(__file__).resolve().parents[1]
    host_registry = {"KSampler": object(), "VAELoader": object()}
    host_nodes_module = SimpleNamespace(
        __file__=str(extension_root.parent / "nodes.py"),
        NODE_CLASS_MAPPINGS=host_registry,
    )

    monkeypatch.setitem(sys.modules, "nodes", host_nodes_module)

    resolved = _resolve_active_comfy_node_class_mappings(extension_root)

    assert "KSampler" in resolved
    assert "VAELoader" in resolved
    assert "SugarCubes.CubeInput" in resolved


def test_resolve_active_comfy_node_class_mappings_ignores_local_shadowing(
    monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture
) -> None:
    extension_root = Path(__file__).resolve().parents[1]
    local_shadow_module = SimpleNamespace(
        __file__=str(extension_root / "sugarcubes" / "nodes.py"),
        NODE_CLASS_MAPPINGS={"WrongLocalRegistry": object()},
    )

    monkeypatch.setitem(sys.modules, "nodes", local_shadow_module)

    with caplog.at_level(logging.WARNING, logger="sugarcubes.backend"):
        resolved = _resolve_active_comfy_node_class_mappings(extension_root)

    assert "WrongLocalRegistry" not in resolved
    assert set(resolved) == {
        "SugarCubes.CubeInput",
        "SugarCubes.CubeOutput",
    }
    assert any(
        "active Comfy nodes registry unavailable" in record.message
        for record in caplog.records
    )


def test_cube_library_service_continues_when_version_cache_prune_fails(
    monkeypatch: pytest.MonkeyPatch,
    caplog: pytest.LogCaptureFixture,
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Version-cache cleanup failures must not block backend service construction."""

    def fail_prune(self: CubeVersionArtifactCache) -> None:
        """Raise a deterministic cache cleanup failure."""

        _ = self
        raise OSError("cache locked")

    monkeypatch.setattr(CubeVersionArtifactCache, "prune", fail_prune)

    with caplog.at_level(
        logging.WARNING,
        logger="sugarcubes.backend.services.cube_library_service",
    ):
        services = backend_services_factory(tmp_path)

    assert services.library is not None
    assert any(
        "version artifact cache prune failed" in record.message
        for record in caplog.records
    )

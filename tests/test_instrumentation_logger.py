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

from __future__ import annotations

from typing import Any

from pathlib import Path
import pytest
import json
import logging

from sugarcubes.exporter import export_cubes
from sugarcubes.instrumentation import log_diagnostic
from sugarcubes.importer import load_cube


def _definition_resolver(_class_type: Any) -> Any:
    return {}


def test_exporter_structured_log_is_quiet_at_info_by_default(
    caplog: pytest.LogCaptureFixture, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.delenv("SUGARCUBES_DIAGNOSTICS", raising=False)
    cube_id = "artificial-sweetener/base-cubes/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {
            "class_type": "KSampler",
            "inputs": {"image": ["1", 0]},
        },
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["2", 0]},
        },
    }

    with caplog.at_level(logging.INFO, logger="sugarcubes.events"):
        export_cubes(prompt, definition_resolver=_definition_resolver)

    assert not any("sugarcubes.event" in record.message for record in caplog.records)


def test_exporter_structured_log_is_debug_by_default(
    caplog: pytest.LogCaptureFixture, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.delenv("SUGARCUBES_DIAGNOSTICS", raising=False)
    cube_id = "artificial-sweetener/base-cubes/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {
            "class_type": "KSampler",
            "inputs": {"image": ["1", 0]},
        },
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["2", 0]},
        },
    }

    with caplog.at_level(logging.DEBUG, logger="sugarcubes.events"):
        export_cubes(prompt, definition_resolver=_definition_resolver)

    assert any(
        '"phase": "exporter.phase2"' in record.message for record in caplog.records
    )
    assert any(
        '"event": "serialize_cube"' in record.message for record in caplog.records
    )


def test_importer_structured_log_promotes_to_info_when_diagnostics_enabled(
    tmp_path: Path, caplog: pytest.LogCaptureFixture, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("SUGARCUBES_DIAGNOSTICS", "1")
    payload = {
        "description": "demo",
        "cube_id": "artificial-sweetener/base-cubes/demo.cube",
        "version": "1.0.0",
        "metadata": {},
        "implementation": {
            "nodes": {},
            "inputs": {},
            "outputs": {},
            "layout": {},
            "definitions": {},
            "subgraphs": [],
        },
        "surface": {"default_flavor_id": "default", "controls": []},
        "flavors": {"authored": [{"id": "default", "name": "Default", "values": {}}]},
    }
    path = tmp_path / "demo.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    with caplog.at_level(logging.INFO, logger="sugarcubes.events"):
        load_cube(path)

    assert any(
        '"phase": "importer.phase3"' in record.message for record in caplog.records
    )
    assert any('"event": "load_cube"' in record.message for record in caplog.records)


def test_marker_diagnostic_is_quiet_at_info_by_default(
    caplog: pytest.LogCaptureFixture, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.delenv("SUGARCUBES_DIAGNOSTICS", raising=False)
    logger = logging.getLogger("sugarcubes.test_diagnostics")

    with caplog.at_level(logging.INFO, logger="sugarcubes.test_diagnostics"):
        log_diagnostic(
            logger,
            "SugarCubes test diagnostic",
            "example_event",
            {"count": 2},
        )

    assert "SugarCubes test diagnostic" not in caplog.text


def test_marker_diagnostic_is_debug_by_default(
    caplog: pytest.LogCaptureFixture, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.delenv("SUGARCUBES_DIAGNOSTICS", raising=False)
    logger = logging.getLogger("sugarcubes.test_diagnostics")

    with caplog.at_level(logging.DEBUG, logger="sugarcubes.test_diagnostics"):
        log_diagnostic(
            logger,
            "SugarCubes test diagnostic",
            "example_event",
            {"count": 2},
        )

    assert "SugarCubes test diagnostic event=example_event count=2" in caplog.text


def test_marker_diagnostic_promotes_to_info_when_diagnostics_enabled(
    caplog: pytest.LogCaptureFixture, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("SUGARCUBES_DIAGNOSTICS", "true")
    logger = logging.getLogger("sugarcubes.test_diagnostics")

    with caplog.at_level(logging.INFO, logger="sugarcubes.test_diagnostics"):
        log_diagnostic(
            logger,
            "SugarCubes test diagnostic",
            "example_event",
            {"count": 2},
        )

    assert "SugarCubes test diagnostic event=example_event count=2" in caplog.text

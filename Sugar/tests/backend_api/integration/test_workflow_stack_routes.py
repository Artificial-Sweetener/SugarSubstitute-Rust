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
"""Prove complete cart HTTP validation, portable artifacts and arbitrary native order."""

from __future__ import annotations

import asyncio
from copy import deepcopy
from pathlib import Path
from typing import cast

import pytest

from sugarcubes.backend.routes import build_route_handlers
from tests.backend_api.support.backend_fixtures import FakeRequest, decode_json_response
from tests.backend_api.support.typing_support import BackendServicesFactory
from tests.execution.support.execution_fixtures import (
    cube_workflow,
    image_passthrough_document,
)


def test_stack_route_plans_repeated_artifacts_without_changing_the_request(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    """Return unique aliases and canonical geometry for a complete ordered cart."""

    artifact = image_passthrough_document("First")
    workflow = cube_workflow({"first": artifact})
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    nodes[0].update({"pos": [0, 0], "size": [320, 600]})
    body = {
        "schema_version": 1,
        "workflow": workflow,
        "entries": [
            {
                "source": "new",
                "instance_id": "second",
                "alias": "First",
                "document": artifact,
                "size": [480, 600],
            },
            {"source": "existing", "instance_id": "first"},
        ],
    }
    before = deepcopy(body)
    response = asyncio.run(
        build_route_handlers(backend_services_factory(tmp_path)).apply_workflow_stack(
            FakeRequest(body=body)
        )
    )
    assert response.status == 200
    payload = decode_json_response(response)
    assert payload["segments"][0]["instance_ids"] == ["second", "first"]
    assert {entry["instance_alias"] for entry in payload["instances"]} == {
        "First",
        "First 2",
    }
    assert body == before
    assert any(
        definition["extra"]["sugarcubes_document"] == artifact
        for definition in payload["workflow"]["definitions"]["subgraphs"]
    )


@pytest.mark.parametrize(
    "entries",
    [
        None,
        [None],
        [{"source": "foreign", "instance_id": "first"}],
        [{"source": "existing", "instance_id": ""}],
        [
            {
                "source": "new",
                "instance_id": "new",
                "alias": "First",
                "document": None,
                "size": [320, 600],
            }
        ],
    ],
)
def test_stack_route_rejects_invalid_references(
    tmp_path: Path, backend_services_factory: BackendServicesFactory, entries: object
) -> None:
    """Reject malformed dynamic fields before they enter the cart application owner."""

    body = {"schema_version": 1, "workflow": cube_workflow({}), "entries": entries}
    response = asyncio.run(
        build_route_handlers(backend_services_factory(tmp_path)).apply_workflow_stack(
            FakeRequest(body=body)
        )
    )
    assert response.status == 400
    assert decode_json_response(response)["error"]["message"]


@pytest.mark.parametrize("size", [[True, 600], [320], [float("nan"), 600], [0, 600]])
def test_stack_route_rejects_invalid_native_frames(
    tmp_path: Path, backend_services_factory: BackendServicesFactory, size: list[object]
) -> None:
    """Reject incomplete, nonfinite, boolean and out-of-policy dimensions."""

    body = {
        "schema_version": 1,
        "workflow": cube_workflow({}),
        "entries": [
            {
                "source": "new",
                "instance_id": "new",
                "alias": "First",
                "document": image_passthrough_document("First"),
                "size": size,
            }
        ],
    }
    response = asyncio.run(
        build_route_handlers(backend_services_factory(tmp_path)).apply_workflow_stack(
            FakeRequest(body=body)
        )
    )
    assert response.status in (400, 422)
    assert decode_json_response(response)["error"]["message"]


def test_stack_route_preserves_actionable_canonical_failures(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    """Return missing native geometry through the structured mutation error boundary."""

    workflow = cube_workflow(
        {
            "first": image_passthrough_document("First"),
            "second": image_passthrough_document("Second"),
        },
        links=((1, 1, 0, 2, 0, "IMAGE"),),
    )
    body = {
        "schema_version": 1,
        "workflow": workflow,
        "entries": [
            {"source": "existing", "instance_id": "second"},
            {"source": "existing", "instance_id": "first"},
        ],
    }
    response = asyncio.run(
        build_route_handlers(backend_services_factory(tmp_path)).apply_workflow_stack(
            FakeRequest(body=body)
        )
    )
    assert response.status == 422
    assert "position" in decode_json_response(response)["error"]["message"]

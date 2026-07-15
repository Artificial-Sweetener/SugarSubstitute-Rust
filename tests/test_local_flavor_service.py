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
"""Local flavor service and route tests."""

from __future__ import annotations

from typing import Any

from pathlib import Path
from .typing_support import BackendServicesFactory

import asyncio

import pytest

from sugarcubes.backend.routes import build_route_handlers
from sugarcubes.backend.responses import BackendError

from .conftest import FakeRequest, decode_json_response

LOCAL_CUBE_ID = "local/personal/demo.cube"
SURFACE_SIGNATURE = "surface-a"


def test_local_flavor_service_reads_empty_state_for_missing_cube(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    """Missing local flavor JSON behaves like an empty local catalog."""

    services = backend_services_factory(tmp_path)

    state = services.local_flavors.read_cube_state(LOCAL_CUBE_ID)

    assert state["schema_version"] == 1
    assert state["cube_id"] == LOCAL_CUBE_ID
    assert state["surfaces"] == {}


def test_local_flavor_service_writes_and_reads_local_flavor(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    """Saved local flavors persist to the hashed disk-backed store."""

    services = backend_services_factory(tmp_path)

    state = services.local_flavors.save_local_flavor(
        cube_id=LOCAL_CUBE_ID,
        surface_signature=SURFACE_SIGNATURE,
        name="Portrait",
        values={"ksampler.cfg": 7},
    )
    read_back = services.local_flavors.read_cube_state(LOCAL_CUBE_ID)

    flavor_file = services.local_flavors.path_for_cube_id(LOCAL_CUBE_ID)
    assert flavor_file.exists()
    assert state == read_back
    assert read_back["surfaces"][SURFACE_SIGNATURE]["selected_flavor_id"] == "portrait"
    assert read_back["surfaces"][SURFACE_SIGNATURE]["flavors"][0]["values"] == {
        "ksampler.cfg": 7
    }


@pytest.mark.parametrize("cube_id", ["invalid", "local/flavors/demo.cube"])
def test_local_flavor_service_rejects_invalid_cube_ids(
    tmp_path: Path, backend_services_factory: BackendServicesFactory, cube_id: Any
) -> None:
    """The local flavor store honors canonical cube id validation."""

    services = backend_services_factory(tmp_path)

    with pytest.raises(BackendError):
        services.local_flavors.read_cube_state(cube_id)


def test_local_flavor_service_rejects_authored_collisions(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    """Local flavors cannot reuse authored flavor names or ids."""

    services = backend_services_factory(tmp_path)

    with pytest.raises(BackendError, match="collides"):
        services.local_flavors.save_local_flavor(
            cube_id=LOCAL_CUBE_ID,
            surface_signature=SURFACE_SIGNATURE,
            name="Portrait",
            values={},
            authored_flavors=[{"id": "portrait", "name": "Portrait", "values": {}}],
        )


def test_local_flavor_service_reconciles_authored_collisions(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    """Collision reconciliation renames local flavors deterministically."""

    services = backend_services_factory(tmp_path)
    services.local_flavors.save_local_flavor(
        cube_id=LOCAL_CUBE_ID,
        surface_signature=SURFACE_SIGNATURE,
        name="Portrait",
        values={},
    )

    result = services.local_flavors.reconcile_with_authored_flavors(
        cube_id=LOCAL_CUBE_ID,
        surface_signature=SURFACE_SIGNATURE,
        authored_flavors=[{"id": "portrait", "name": "Portrait", "values": {}}],
    )

    surface = result["state"]["surfaces"][SURFACE_SIGNATURE]
    assert result["conflict_count"] == 1
    assert surface["flavors"][0]["id"] == "portrait_local"
    assert surface["flavors"][0]["name"] == "Portrait_local"
    assert surface["selected_flavor_id"] == "portrait_local"


def test_local_flavor_routes_cover_save_delete_select_migrate_and_reconcile(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    """HTTP routes expose the backend local flavor store."""

    services = backend_services_factory(tmp_path)
    handlers = build_route_handlers(services)

    save_response = asyncio.run(
        handlers.save_local_flavor(
            FakeRequest(
                body={
                    "cube_id": LOCAL_CUBE_ID,
                    "surface_signature": SURFACE_SIGNATURE,
                    "name": "Portrait",
                    "values": {"ksampler.cfg": 7},
                }
            )
        )
    )
    assert save_response.status == 200

    select_response = asyncio.run(
        handlers.select_local_flavor(
            FakeRequest(
                body={
                    "cube_id": LOCAL_CUBE_ID,
                    "surface_signature": SURFACE_SIGNATURE,
                    "flavor_id": "portrait",
                }
            )
        )
    )
    assert select_response.status == 200

    delete_response = asyncio.run(
        handlers.delete_local_flavor(
            FakeRequest(
                body={
                    "cube_id": LOCAL_CUBE_ID,
                    "surface_signature": SURFACE_SIGNATURE,
                    "flavor_id": "portrait",
                }
            )
        )
    )
    assert delete_response.status == 200
    deleted_payload = decode_json_response(delete_response)
    assert deleted_payload["state"]["surfaces"][SURFACE_SIGNATURE]["flavors"] == []

    migrate_response = asyncio.run(
        handlers.migrate_local_flavors(
            FakeRequest(
                body={
                    "states": [
                        {
                            "cube_id": LOCAL_CUBE_ID,
                            "state": {
                                "schema": 1,
                                "cube_id": LOCAL_CUBE_ID,
                                "surfaces": {
                                    SURFACE_SIGNATURE: {
                                        "selected_flavor_id": "portrait",
                                        "flavors": [
                                            {
                                                "id": "portrait",
                                                "name": "Portrait",
                                                "values": {},
                                            }
                                        ],
                                    }
                                },
                            },
                        }
                    ]
                }
            )
        )
    )
    assert migrate_response.status == 200

    reconcile_response = asyncio.run(
        handlers.reconcile_local_flavors(
            FakeRequest(
                body={
                    "cube_id": LOCAL_CUBE_ID,
                    "surface_signature": SURFACE_SIGNATURE,
                    "authored_flavors": [
                        {"id": "portrait", "name": "Portrait", "values": {}}
                    ],
                }
            )
        )
    )
    reconcile_payload = decode_json_response(reconcile_response)
    assert reconcile_response.status == 200
    assert reconcile_payload["conflict_count"] == 1

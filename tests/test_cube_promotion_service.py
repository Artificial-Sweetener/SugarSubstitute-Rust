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
"""Verify recoverable personal-to-pack cube promotion."""

from __future__ import annotations

import asyncio
import json
from types import SimpleNamespace

import pytest

from sugarcubes.backend.responses import BackendError
from sugarcubes.backend.routes import build_route_handlers
from sugarcubes.payloads import retarget_cube_payload
from conftest import FakeRequest, claim_github_owner, decode_json_response

SOURCE_CUBE_ID = "local/personal/Text to Image.cube"
TARGET_CUBE_ID = "ExampleUser/Example-Cubes/SDXL/Text to Image.cube"


def _payload() -> dict:
    """Build one persisted personal cube with embedded identity references."""

    return {
        "cube_id": SOURCE_CUBE_ID,
        "version": "2.3.4",
        "description": "Personal draft",
        "metadata": {
            "default_alias": "Text to Image",
            "author_url": "https://example.invalid/author",
        },
        "nodes": {
            "input": {
                "class_type": "SugarCubes.CubeInput",
                "inputs": {
                    "cube_id": SOURCE_CUBE_ID,
                    "default_alias": "Text to Image",
                },
            }
        },
        "layout": {
            "groups": [
                {
                    "sugarcubes": {
                        "cube_id": SOURCE_CUBE_ID,
                        "default_alias": "Text to Image",
                    }
                }
            ]
        },
    }


def _prepare_services(tmp_path, backend_services_factory):
    """Create isolated personal and authoring Git repos for promotion tests."""

    def load_cube_artifact(path):
        payload = json.loads(path.read_text(encoding="utf-8"))
        return SimpleNamespace(version=payload["version"], payload=payload)

    def prepare_cube_import(loaded, drop_origin=(0.0, 0.0)):
        return SimpleNamespace(
            cube=loaded.payload,
            nodes=[],
            markers=[],
            connections=[],
            layout=None,
            warnings=[],
            subgraphs=[],
        )

    services = backend_services_factory(
        tmp_path,
        retarget_cube_payload=retarget_cube_payload,
        load_cube_artifact=load_cube_artifact,
        prepare_cube_import=prepare_cube_import,
    )
    claim_github_owner(services, owner="ExampleUser")
    services.tracked_repos.ensure_authoring_repo(
        owner="ExampleUser",
        repo="Example-Cubes",
    )
    source_context = services.promotion.artifacts.context(SOURCE_CUBE_ID)
    services.promotion.artifacts.write(source_context, _payload())
    services.promotion.history.commit_context(
        source_context, message="create personal cube"
    )
    return services, source_context


def test_promote_moves_personal_cube_with_version_flavors_and_redirect(
    tmp_path, backend_services_factory
):
    """Promote all identity-owned state after the target commit is durable."""

    services, source_context = _prepare_services(tmp_path, backend_services_factory)
    services.local_flavors.write_cube_state(
        SOURCE_CUBE_ID,
        {
            "cube_id": SOURCE_CUBE_ID,
            "surfaces": {
                "surface": {
                    "flavors": [{"id": "warm", "name": "Warm", "values": {"cfg": 5}}],
                    "selected_flavor_id": "warm",
                }
            },
        },
    )

    result = services.promotion.promote(
        source_cube_id=SOURCE_CUBE_ID,
        owner="ExampleUser",
        repo="Example-Cubes",
        name="Text to Image",
        target_model="SDXL",
        supported_models=["SD 1.5"],
        description_set=True,
        description="Ready to share",
        metadata={"tags": ["image", "starter"]},
    )

    target_context, payload = services.promotion.artifacts.read(TARGET_CUBE_ID)
    assert result["status"] == "complete"
    assert result["previous_cube_id"] == SOURCE_CUBE_ID
    assert result["cube"]["cube_id"] == TARGET_CUBE_ID
    assert result["version"] == "2.3.4"
    assert result["commits"]["target"]["commit_sha"]
    assert result["commits"]["source"]["commit_sha"]
    assert not source_context.cube_path.exists()
    assert target_context.cube_path.is_file()
    assert payload["cube_id"] == TARGET_CUBE_ID
    assert payload["version"] == "2.3.4"
    assert payload["description"] == "Ready to share"
    assert payload["metadata"] == {
        "default_alias": "SDXL/Text to Image",
        "target_model": "SDXL",
        "supported_models": ["SDXL", "SD 1.5"],
        "author_url": "https://example.invalid/author",
        "tags": ["image", "starter"],
    }
    assert payload["nodes"]["input"]["inputs"]["cube_id"] == TARGET_CUBE_ID
    assert (
        payload["layout"]["groups"][0]["sugarcubes"]["default_alias"]
        == "SDXL/Text to Image"
    )
    assert services.redirects.resolve(SOURCE_CUBE_ID) == TARGET_CUBE_ID
    assert services.redirects.get(SOURCE_CUBE_ID)["source_commit_sha"]
    assert (
        services.local_flavors.read_cube_state(TARGET_CUBE_ID)["cube_id"]
        == TARGET_CUBE_ID
    )
    assert not services.local_flavors.path_for_cube_id(SOURCE_CUBE_ID).exists()

    loaded = services.loader.load_cube(
        cube_id=SOURCE_CUBE_ID,
        version_pin="2.3.4",
        drop_origin=(0.0, 0.0),
    )
    assert loaded["cube"]["cube_id"] == TARGET_CUBE_ID
    assert loaded["identity_redirect"] == {
        "requested_cube_id": SOURCE_CUBE_ID,
        "resolved_cube_id": TARGET_CUBE_ID,
    }

    history = services.revisions.list_revisions(cube_id=SOURCE_CUBE_ID)
    assert history["cube_id"] == TARGET_CUBE_ID
    assert history["identity_redirect"]["requested_cube_id"] == SOURCE_CUBE_ID
    assert any(
        entry.get("history_origin") == "personal" for entry in history["revisions"]
    )


def test_promote_is_idempotent_after_completed_cleanup(
    tmp_path, backend_services_factory
):
    """Return the completed target when a client retries the same promotion."""

    services, _source_context = _prepare_services(tmp_path, backend_services_factory)
    first = services.promotion.promote(
        source_cube_id=SOURCE_CUBE_ID,
        owner="ExampleUser",
        repo="Example-Cubes",
        name="Text to Image",
        target_model="SDXL",
    )

    second = services.promotion.promote(
        source_cube_id=SOURCE_CUBE_ID,
        owner="ExampleUser",
        repo="Example-Cubes",
        name="Text to Image",
        target_model="SDXL",
    )

    assert first["status"] == "complete"
    assert second["status"] == "complete"
    assert second["cube"]["cube_id"] == TARGET_CUBE_ID


def test_promote_resumes_when_redirect_persistence_was_interrupted(
    tmp_path, backend_services_factory, monkeypatch
):
    """Recover a committed target without overwriting it or losing the personal source."""

    services, source_context = _prepare_services(tmp_path, backend_services_factory)
    original_record = services.redirects.record_promotion
    attempts = 0

    def interrupt_first_redirect(**kwargs):
        nonlocal attempts
        attempts += 1
        if attempts == 1:
            raise BackendError("simulated redirect interruption", status=500)
        return original_record(**kwargs)

    monkeypatch.setattr(
        services.redirects, "record_promotion", interrupt_first_redirect
    )

    with pytest.raises(BackendError, match="simulated redirect interruption"):
        services.promotion.promote(
            source_cube_id=SOURCE_CUBE_ID,
            owner="ExampleUser",
            repo="Example-Cubes",
            name="Text to Image",
            target_model="SDXL",
        )

    assert source_context.cube_path.is_file()
    assert services.promotion.artifacts.context(TARGET_CUBE_ID).cube_path.is_file()
    assert (
        services.redirects.get_pending(SOURCE_CUBE_ID)["target_cube_id"]
        == TARGET_CUBE_ID
    )

    resumed = services.promotion.promote(
        source_cube_id=SOURCE_CUBE_ID,
        owner="ExampleUser",
        repo="Example-Cubes",
        name="Text to Image",
        target_model="SDXL",
    )

    assert resumed["status"] == "complete"
    assert not source_context.cube_path.exists()
    assert services.redirects.get_pending(SOURCE_CUBE_ID) is None
    assert services.redirects.resolve(SOURCE_CUBE_ID) == TARGET_CUBE_ID


def test_promote_route_accepts_the_explicit_destination_contract(
    tmp_path, backend_services_factory
):
    """Expose promotion as one backend-owned application operation."""

    services, _source_context = _prepare_services(tmp_path, backend_services_factory)

    response = asyncio.run(
        build_route_handlers(services).promote_cube(
            FakeRequest(
                body={
                    "source_cube_id": SOURCE_CUBE_ID,
                    "destination": {
                        "owner": "ExampleUser",
                        "repo": "Example-Cubes",
                    },
                    "name": "Text to Image",
                    "target_model": "SDXL",
                    "supported_models": ["SD 1.5"],
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["status"] == "complete"
    assert payload["cube"]["cube_id"] == TARGET_CUBE_ID


def test_promote_rejects_nonpersonal_sources(tmp_path, backend_services_factory):
    """Keep cross-source moves behind the explicit personal promotion boundary."""

    services = backend_services_factory(tmp_path)

    with pytest.raises(BackendError, match="Only local/personal cubes"):
        services.promotion.promote(
            source_cube_id="local/imported/Demo.cube",
            owner="ExampleUser",
            repo="Example-Cubes",
            name="Demo",
            target_model="SDXL",
        )


def test_redirect_store_is_machine_local_and_resolves_chains(
    tmp_path, backend_services_factory
):
    """Persist promotion provenance outside cube repos and resolve chained identities."""

    services = backend_services_factory(tmp_path)
    intermediate = "local/personal/Renamed.cube"
    services.redirects.record_promotion(
        source_cube_id=SOURCE_CUBE_ID,
        target_cube_id=intermediate,
        source_relative_path="personal/Text to Image.cube",
        source_commit_sha="source-sha",
        target_commit_sha="middle-sha",
        version="2.3.4",
    )
    services.redirects.record_promotion(
        source_cube_id=intermediate,
        target_cube_id=TARGET_CUBE_ID,
        source_relative_path="personal/Renamed.cube",
        source_commit_sha="middle-sha",
        target_commit_sha="target-sha",
        version="2.3.4",
    )

    assert services.redirects.resolve(SOURCE_CUBE_ID) == TARGET_CUBE_ID
    store = json.loads(services.redirects.store_path().read_text(encoding="utf-8"))
    assert store["schema_version"] == 1
    assert store["redirects"][SOURCE_CUBE_ID]["target_cube_id"] == intermediate

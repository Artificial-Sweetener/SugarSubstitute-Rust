#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Prove Comfy-facing workflow library routes preserve embedded authority."""

from __future__ import annotations

import asyncio
from dataclasses import dataclass, field, replace
import json
from pathlib import Path

from sugarcubes.backend.routes import build_route_handlers
from sugarcubes.library import CubeSourceSyncService
from sugarcubes.workflow import read_canonical_workflow
from tests.backend_api.support.backend_fixtures import FakeRequest, decode_json_response
from tests.backend_api.support.typing_support import BackendServicesFactory
from tests.workflow.support.workflow_fixtures import cube_document, cube_workflow


@dataclass
class _SourceSyncPort:
    """Record route-driven sync order without touching a remote repository."""

    semantic_hash: str
    calls: list[str] = field(default_factory=list)

    def preflight(self, repo_ref: str) -> None:
        """Record non-mutating source validation."""

        self.calls.append(f"preflight:{repo_ref}")

    def sync(self, repo_ref: str) -> None:
        """Record the approved mutation boundary."""

        self.calls.append(f"sync:{repo_ref}")

    def resolve_semantic_hash(
        self, *, repo_ref: str, cube_id: str, cube_version: str
    ) -> str | None:
        """Return the controlled post-sync source hash."""

        self.calls.append(f"resolve:{repo_ref}:{cube_id}@{cube_version}")
        return self.semantic_hash


def test_classification_compares_installed_documents_by_semantic_meaning(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Report a real synced exact match and preserve divergent embedded truth."""

    services = backend_services_factory(tmp_path)
    handlers = build_route_handlers(services)
    cube_path = (
        services.tracked_repos.checkout_path("Artificial-Sweetener", "Base-Cubes")
        / "SDXL"
        / "Text to Image.cube"
    )
    cube_path.parent.mkdir(parents=True, exist_ok=True)
    cube_path.write_text(
        json.dumps(cube_document(), indent=4, sort_keys=True),
        encoding="utf-8",
    )

    exact_response = asyncio.run(
        handlers.classify_workflow(FakeRequest(body={"workflow": cube_workflow()}))
    )
    exact = decode_json_response(exact_response)["definitions"][0]

    assert exact["primary_class"] == "synced"
    assert exact["access"] == "read_only"
    assert exact["matches"][0]["state"] == "exact"

    divergent_document = cube_document()
    divergent_document["description"] = "Different installed behavior claim."
    cube_path.write_text(json.dumps(divergent_document), encoding="utf-8")
    divergent_response = asyncio.run(
        handlers.classify_workflow(FakeRequest(body={"workflow": cube_workflow()}))
    )
    divergent = decode_json_response(divergent_response)["definitions"][0]

    assert divergent["primary_class"] == "none"
    assert divergent["divergent_matches"][0]["state"] == "divergent"


def test_classify_capture_and_fork_are_distinct_sugarcubes_operations(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Expose exact capture separately from writable derivation."""

    services = backend_services_factory(tmp_path)
    handlers = build_route_handlers(services)
    workflow = cube_workflow()

    classification_response = asyncio.run(
        handlers.classify_workflow(FakeRequest(body={"workflow": workflow}))
    )
    classification_payload = decode_json_response(classification_response)
    definition = classification_payload["definitions"][0]

    assert classification_response.status == 200
    assert definition["primary_class"] == "none"
    assert definition["access"] == "read_only"

    captured_response = asyncio.run(
        handlers.capture_workflow_cube(
            FakeRequest(
                body={
                    "workflow": workflow,
                    "definition_id": definition["definition_id"],
                    "expected_semantic_hash": definition["semantic_hash"],
                }
            )
        )
    )
    captured_payload = decode_json_response(captured_response)

    assert captured_response.status == 200
    assert captured_payload["classification"]["primary_class"] == "captured"
    assert "lineage" not in captured_payload
    assert "rebinds" not in captured_payload

    fork_cube_id = "local/personal/SDXL/Workflow Fork.cube"
    fork_response = asyncio.run(
        handlers.fork_workflow_cube(
            FakeRequest(
                body={
                    "workflow": workflow,
                    "definition_id": definition["definition_id"],
                    "expected_semantic_hash": definition["semantic_hash"],
                    "new_cube_id": fork_cube_id,
                    "selected_instance_ids": ["cube-sdxl-text-1"],
                    "destination": "local",
                }
            )
        )
    )
    fork_payload = decode_json_response(fork_response)

    assert fork_response.status == 201
    assert fork_payload["fork_cube_id"] == fork_cube_id
    assert (
        fork_payload["lineage"]["parent_semantic_hash"] == definition["semantic_hash"]
    )
    assert fork_payload["rebinds"][0]["instance_id"] == "cube-sdxl-text-1"
    persisted = (
        services.tracked_repos.local_repo_root()
        / "personal"
        / "SDXL"
        / "Workflow Fork.cube"
    )
    document = json.loads(persisted.read_text(encoding="utf-8"))
    assert document["cube_id"] == fork_cube_id
    assert document["metadata"]["lineage"]["parent_cube_id"] == definition["cube_id"]


def test_source_sync_route_requires_approval_and_preserves_embedded_content(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Expose repository mutation only after explicit Comfy-side approval."""

    base_services = backend_services_factory(tmp_path)
    workflow = cube_workflow()
    classification = base_services.workflow_library.classify_workflow(
        read_canonical_workflow(workflow)
    ).definitions[0]
    port = _SourceSyncPort(classification.semantic_hash)
    services = replace(
        base_services,
        workflow_source_sync=CubeSourceSyncService(port),
    )
    handlers = build_route_handlers(services)
    request = {
        "workflow": workflow,
        "definition_id": classification.definition_id,
        "expected_semantic_hash": classification.semantic_hash,
    }

    denied = asyncio.run(
        handlers.sync_workflow_cube_source(
            FakeRequest(body={**request, "approved": False})
        )
    )
    approved = asyncio.run(
        handlers.sync_workflow_cube_source(
            FakeRequest(body={**request, "approved": True})
        )
    )

    assert denied.status == 403
    assert port.calls == [
        "preflight:Artificial-Sweetener/Base-Cubes",
        "sync:Artificial-Sweetener/Base-Cubes",
        (
            "resolve:Artificial-Sweetener/Base-Cubes:"
            "Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube@1.2.0"
        ),
    ]
    assert approved.status == 200
    assert decode_json_response(approved)["status"] == "exact"
    assert workflow == cube_workflow()

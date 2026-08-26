#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Verify catalog artifacts use canonical Cube meaning instead of file bytes."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from sugarcubes.backend.services.workflow_catalog_artifact_provider import (
    WorkflowCatalogArtifactProvider,
)
from sugarcubes.cube_model import CubeDocument, sanitize_authored_defaults_document
from sugarcubes.workflow.semantic_hash import semantic_hash
from tests.workflow.support.workflow_fixtures import cube_document


@dataclass
class _Listing:
    """Return controlled source-aware summaries through the real listing port."""

    summaries: list[dict[str, Any]]

    def list_catalog_cube_summaries(
        self, *, include_disabled: bool, include_internal_payload: bool
    ) -> list[dict[str, Any]]:
        """Require internal canonical payloads for semantic comparison."""

        assert include_disabled is False
        assert include_internal_payload is True
        return self.summaries


def test_projects_local_and_synced_artifacts_from_normalized_documents() -> None:
    """Derive source class, access, and hash from existing library authority."""

    document = cube_document()
    reordered = {key: document[key] for key in reversed(document)}
    provider = WorkflowCatalogArtifactProvider(
        _Listing(
            [
                {
                    "cube_id": document["cube_id"],
                    "version": document["version"],
                    "is_writable": True,
                    "source": {
                        "type": "local",
                        "namespace": "personal",
                        "repo_relative_path": "SDXL/Text to Image.cube",
                    },
                    "_payload": reordered,
                },
                {
                    "cube_id": document["cube_id"],
                    "version": document["version"],
                    "is_writable": False,
                    "source": {
                        "type": "github",
                        "repo_ref": "Artificial-Sweetener/Base-Cubes",
                        "repo_relative_path": "SDXL/Text to Image.cube",
                    },
                    "_payload": document,
                },
            ]
        )
    )

    artifacts = provider.list_artifacts()

    assert [(item.library_class, item.access) for item in artifacts] == [
        ("synced", "read_only"),
        ("local", "writable"),
    ]
    canonical = CubeDocument.from_dict(document).to_dict()
    assert {item.semantic_hash for item in artifacts} == {semantic_hash(canonical)}
    assert artifacts[0].source_ref == (
        "github:Artificial-Sweetener/Base-Cubes:SDXL/Text to Image.cube"
    )


def test_ignores_invalid_catalog_payloads_without_claiming_a_match() -> None:
    """Keep malformed installed artifacts from shadowing workflow content."""

    document = cube_document()
    provider = WorkflowCatalogArtifactProvider(
        _Listing(
            [
                {
                    "cube_id": document["cube_id"],
                    "version": document["version"],
                    "source": {"type": "local", "namespace": "personal"},
                    "_payload": {"cube_id": document["cube_id"]},
                }
            ]
        )
    )

    assert provider.list_artifacts() == ()


def test_hashes_the_same_portable_document_that_cube_loading_embeds() -> None:
    """Exclude machine-local picker values from catalog matching semantics."""

    document = cube_document()
    implementation = document["implementation"]
    assert isinstance(implementation, dict)
    nodes = implementation["nodes"]
    assert isinstance(nodes, dict)
    checkpoint = nodes["checkpoint"]
    assert isinstance(checkpoint, dict)
    checkpoint["inputs"] = {"ckpt_name": "machine-local.safetensors"}
    provider = WorkflowCatalogArtifactProvider(
        _Listing(
            [
                {
                    "cube_id": document["cube_id"],
                    "version": document["version"],
                    "is_writable": False,
                    "source": {
                        "type": "github",
                        "repo_ref": "Artificial-Sweetener/Base-Cubes",
                        "repo_relative_path": "SDXL/Text to Image.cube",
                    },
                    "_payload": document,
                }
            ]
        )
    )

    (artifact,) = provider.list_artifacts()

    portable = sanitize_authored_defaults_document(CubeDocument.from_dict(document))
    assert artifact.semantic_hash == semantic_hash(portable.to_dict())


def test_hash_preserves_browser_meaning_for_unsafe_comfy_definition_integers() -> None:
    """Treat an IEEE-754-equivalent browser round trip as identical content."""

    before = {"maximum": 18_446_744_073_709_551_615}
    after_browser_round_trip = {"maximum": 18_446_744_073_709_552_000}

    assert semantic_hash(before) == semantic_hash(after_browser_round_trip)

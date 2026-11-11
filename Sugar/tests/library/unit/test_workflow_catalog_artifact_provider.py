#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Verify catalog artifacts use canonical Cube meaning and reuse unchanged file identities."""

from __future__ import annotations

import json
import os
from collections.abc import Mapping
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import pytest

from sugarcubes.backend.services import cube_file_io, workflow_catalog_artifact_provider
from sugarcubes.backend.services.cube_library_listing import CatalogCubeFile
from sugarcubes.backend.services.workflow_catalog_artifact_provider import (
    WorkflowCatalogArtifactProvider,
)
from sugarcubes.cube_model import CubeDocument, sanitize_authored_defaults_document
from sugarcubes.workflow.semantic_hash import semantic_hash
from tests.workflow.support.workflow_fixtures import cube_document


@dataclass
class _Listing:
    """Return controlled discovered files through the real listing port."""

    files: list[CatalogCubeFile]
    writable: set[Path] = field(default_factory=set)

    def list_catalog_cube_files(
        self, *, include_disabled: bool
    ) -> list[CatalogCubeFile]:
        """Expose only enabled sources, as classification requires."""

        assert include_disabled is False
        return list(self.files)

    def catalog_file_writable(self, file: CatalogCubeFile) -> bool:
        """Answer ownership from the controlled writable set."""

        return file.path in self.writable


def _write(path: Path, document: object) -> Path:
    """Write one cube file and return its resolved path."""

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document), encoding="utf-8")
    return path.resolve()


def _local(root: Path, document: object) -> CatalogCubeFile:
    """Place one cube in the personal local namespace."""

    base = root / "local" / "personal"
    path = _write(base / "SDXL" / "Text to Image.cube", document)
    return CatalogCubeFile(
        path=path, base_dir=base.resolve(), source_kind="local", namespace="personal"
    )


def _synced(root: Path, document: object) -> CatalogCubeFile:
    """Place one cube in a tracked repository checkout."""

    base = root / "repos" / "Base-Cubes"
    path = _write(base / "SDXL" / "Text to Image.cube", document)
    return CatalogCubeFile(
        path=path,
        base_dir=base.resolve(),
        source_kind="github",
        owner="Artificial-Sweetener",
        repo="Base-Cubes",
    )


@pytest.fixture
def reads(monkeypatch: pytest.MonkeyPatch) -> list[Path]:
    """Record every cube file the provider reads."""

    recorded: list[Path] = []
    original = cube_file_io.read_cube_payload

    def counting(path: Path) -> tuple[Mapping[str, Any] | None, str | None]:
        recorded.append(path)
        return original(path)

    monkeypatch.setattr(
        workflow_catalog_artifact_provider, "read_cube_payload", counting
    )
    return recorded


def test_projects_local_and_synced_artifacts_from_normalized_documents(
    tmp_path: Path,
) -> None:
    """Derive source class, access, and hash from existing library authority."""

    document = cube_document()
    reordered = {key: document[key] for key in reversed(document)}
    local = _local(tmp_path, reordered)
    provider = WorkflowCatalogArtifactProvider(
        _Listing([local, _synced(tmp_path, document)], writable={local.path})
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
    assert artifacts[1].source_ref == "local:personal:SDXL/Text to Image.cube"


def test_ignores_invalid_catalog_payloads_without_claiming_a_match(
    tmp_path: Path,
) -> None:
    """Keep malformed installed artifacts from shadowing workflow content."""

    document = cube_document()
    invalid = _local(tmp_path, {"cube_id": document["cube_id"]})
    provider = WorkflowCatalogArtifactProvider(_Listing([invalid]))

    assert provider.list_artifacts() == ()


def test_hashes_the_same_portable_document_that_cube_loading_embeds(
    tmp_path: Path,
) -> None:
    """Exclude machine-local picker values from catalog matching semantics."""

    document = cube_document()
    implementation = document["implementation"]
    assert isinstance(implementation, dict)
    nodes = implementation["nodes"]
    assert isinstance(nodes, dict)
    checkpoint = nodes["checkpoint"]
    assert isinstance(checkpoint, dict)
    checkpoint["inputs"] = {"ckpt_name": "machine-local.safetensors"}
    provider = WorkflowCatalogArtifactProvider(_Listing([_synced(tmp_path, document)]))

    (artifact,) = provider.list_artifacts()

    portable = sanitize_authored_defaults_document(CubeDocument.from_dict(document))
    assert artifact.semantic_hash == semantic_hash(portable.to_dict())


def test_reuses_unchanged_file_identities_across_classifications(
    tmp_path: Path, reads: list[Path]
) -> None:
    """Read each installed Cube once while its modification time and size are unchanged."""

    listing = _Listing(
        [_local(tmp_path, cube_document()), _synced(tmp_path, cube_document())]
    )
    provider = WorkflowCatalogArtifactProvider(listing)

    first = provider.list_artifacts()
    for _ in range(10):
        assert provider.list_artifacts() == first

    assert len(reads) == 2


def test_rereads_only_a_changed_file(tmp_path: Path, reads: list[Path]) -> None:
    """Invalidate one cached identity when its stat facts change."""

    local = _local(tmp_path, cube_document())
    synced = _synced(tmp_path, cube_document())
    provider = WorkflowCatalogArtifactProvider(_Listing([local, synced]))
    before = provider.list_artifacts()
    changed = cube_document()
    changed["description"] = (
        "A changed local description that alters canonical content."
    )
    _write(local.path, changed)
    stat = local.path.stat()
    os.utime(local.path, ns=(stat.st_atime_ns, stat.st_mtime_ns + 1_000_000_000))

    after = provider.list_artifacts()

    assert reads[2:] == [local.path]
    assert after[0] == before[0]
    assert after[1].semantic_hash != before[1].semantic_hash


def test_follows_added_and_removed_sources_and_current_ownership(
    tmp_path: Path, reads: list[Path]
) -> None:
    """Keep discovery and ownership current while reusing unchanged identities."""

    local = _local(tmp_path, cube_document())
    listing = _Listing([local])
    provider = WorkflowCatalogArtifactProvider(listing)
    assert [item.library_class for item in provider.list_artifacts()] == ["local"]

    synced = _synced(tmp_path, cube_document())
    listing.files.append(synced)
    assert [item.library_class for item in provider.list_artifacts()] == [
        "synced",
        "local",
    ]

    listing.files.remove(synced)
    listing.writable.add(local.path)
    (artifact,) = provider.list_artifacts()
    assert artifact.access == "writable"

    listing.files.append(synced)
    provider.list_artifacts()
    assert reads == [local.path, synced.path, synced.path]


def test_hash_preserves_browser_meaning_for_unsafe_comfy_definition_integers() -> None:
    """Treat an IEEE-754-equivalent browser round trip as identical content."""

    before = {"maximum": 18_446_744_073_709_551_615}
    after_browser_round_trip = {"maximum": 18_446_744_073_709_552_000}

    assert semantic_hash(before) == semantic_hash(after_browser_round_trip)

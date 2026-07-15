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
"""Backend-facing Cube Library service contract tests."""

from __future__ import annotations

from typing import Any

from .typing_support import BackendServicesFactory

import json
import logging
import os
from pathlib import Path

import pytest

from sugarcubes.backend.responses import BackendError
from sugarcubes.backend.services import cube_library_service, cube_summary
from sugarcubes.backend.services.cube_file_io import (
    compute_cube_content_hash_bytes,
)

CANONICAL_CUBE_ID = "Artificial-Sweetener/Base-Cubes/demo.cube"
SDXL_CUBE_ID = "Artificial-Sweetener/Base-Cubes/SDXL/demo.cube"


def _cube_payload(
    *,
    cube_id: str = CANONICAL_CUBE_ID,
    version: str = "1.0.0",
    default_alias: str = "demo",
    target_model: str = "",
    supported_models: list[str] | None = None,
    python_module: str = "custom_nodes.ComfyUI-Impact-Pack",
) -> dict[str, object]:
    """Return a compact current-format cube payload for service tests."""

    metadata: dict[str, object] = {"default_alias": default_alias}
    if target_model:
        metadata["target_model"] = target_model
    if supported_models is not None:
        metadata["supported_models"] = supported_models
    return {
        "cube_id": cube_id,
        "version": version,
        "description": "Demo cube",
        "metadata": metadata,
        "implementation": {
            "nodes": {},
            "inputs": {},
            "outputs": {},
            "layout": {},
            "definitions": {
                "ImpactNode": {"python_module": python_module},
            },
            "subgraphs": [],
        },
        "surface": {"default_flavor_id": "default", "controls": []},
        "flavors": {
            "authored": [{"id": "default", "name": "Default", "values": {}}],
        },
    }


def _cube_payload_with_cnr(
    *,
    cube_id: str = CANONICAL_CUBE_ID,
    cnr_id: str = "comfyui-impact-pack",
    version: str = "",
    python_module: str = "custom_nodes.ComfyUI-Impact-Pack",
) -> dict[str, object]:
    """Return a cube payload whose embedded graph records a Comfy Registry id."""

    payload = _cube_payload(cube_id=cube_id, python_module=python_module)
    implementation = payload["implementation"]
    assert isinstance(implementation, dict)
    implementation["layout"] = {
        "nodes": [
            {
                "type": "ImpactNode",
                "properties": {
                    "cnr_id": cnr_id,
                    "ver": version,
                    "Node name for S&R": "Impact Detailer",
                },
            }
        ]
    }
    return payload


def _write_cube(path: Path, payload: dict[str, object]) -> None:
    """Write one cube payload as stable JSON."""

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def _revision_text(payload: dict[str, object]) -> str:
    """Return the exact text emitted by fake git show for a cube revision."""

    return json.dumps(payload, indent=2) + "\n"


def test_backend_catalog_includes_hash_source_revision_and_dirty_state(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Catalog entries should be remote-safe and diagnostic enough for Substitute."""

    def fake_git(args: Any, *, cwd: Any) -> Any:
        class Result:
            stdout = ""

        if args == ["rev-parse", "HEAD"]:
            Result.stdout = "abc123\n"
        elif args == ["status", "--porcelain"]:
            Result.stdout = " M demo.cube\n"
        elif args == ["status", "--porcelain", "--", "demo.cube"]:
            Result.stdout = " M demo.cube\n"
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    (checkout / ".git").mkdir(parents=True)
    _write_cube(checkout / "demo.cube", _cube_payload())

    catalog = services.library.list_library_catalog()

    assert catalog["schemaVersion"] == 1
    assert catalog["catalogRevision"].startswith("sha256:")
    assert catalog["packs"] == {"count": 1, "enabledCount": 1}
    assert catalog["cubes"][0]["cubeId"] == CANONICAL_CUBE_ID
    assert catalog["cubes"][0]["displayName"] == "demo"
    assert catalog["cubes"][0]["targetModel"] == ""
    assert catalog["cubes"][0]["supportedModels"] == []
    assert catalog["cubes"][0]["contentHash"].startswith("sha256:")
    assert catalog["cubes"][0]["source"] == {
        "kind": "github",
        "repoRef": "Artificial-Sweetener/Base-Cubes",
        "owner": "Artificial-Sweetener",
        "repo": "Base-Cubes",
        "branch": "main",
        "path": "demo.cube",
        "localHeadSha": "abc123",
        "remoteHeadSha": "",
        "dirty": True,
    }
    assert catalog["cubes"][0]["requiredCustomNodes"] == ["ComfyUI-Impact-Pack"]


def test_backend_local_cube_sources_report_shared_repo_git_state(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Local catalog and load payloads should report their shared Git state."""

    def fake_git(args: Any, *, cwd: Any) -> Any:
        class Result:
            stdout = ""

        if args == ["rev-parse", "HEAD"]:
            Result.stdout = "local123\n"
        elif args == ["status", "--porcelain"]:
            Result.stdout = " M personal/dirty.cube\n"
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    local_root = services.library.local_workspace_root()
    (local_root / ".git").mkdir()
    personal_root = local_root / "personal"
    _write_cube(
        personal_root / "clean.cube",
        _cube_payload(
            cube_id="local/personal/clean.cube",
            default_alias="Clean",
        ),
    )
    _write_cube(
        personal_root / "dirty.cube",
        _cube_payload(
            cube_id="local/personal/dirty.cube",
            default_alias="Dirty",
        ),
    )

    catalog = services.library.list_library_catalog()
    entries = {entry["cubeId"]: entry for entry in catalog["cubes"]}

    assert entries["local/personal/clean.cube"]["source"] == {
        "kind": "local",
        "namespace": "personal",
        "path": "clean.cube",
        "localHeadSha": "local123",
        "remoteHeadSha": "",
        "dirty": False,
    }
    assert entries["local/personal/dirty.cube"]["source"]["dirty"] is True

    loaded = services.library.load_library_cube("local/personal/clean.cube")

    assert loaded["source"]["localHeadSha"] == "local123"
    assert loaded["source"]["dirty"] is False


def test_backend_catalog_includes_target_model_fields(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Catalog entries expose the target namespace separately from display text."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(
        checkout / "SDXL" / "demo.cube",
        _cube_payload(
            cube_id=SDXL_CUBE_ID,
            target_model="SDXL",
            supported_models=["SD 1.5"],
        ),
    )

    catalog = services.library.list_library_catalog()
    entry = catalog["cubes"][0]

    assert entry["cubeId"] == SDXL_CUBE_ID
    assert entry["displayName"] == "SDXL/demo"
    assert entry["targetModel"] == "SDXL"
    assert entry["supportedModels"] == ["SDXL", "SD 1.5"]


def test_backend_catalog_repairs_stale_managed_checkout_path(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Catalog listing should survive extension directory renames."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    stale_checkout = (
        tmp_path
        / "ComfyUI-SugarCubes"
        / ".sugarcubes"
        / "Artificial-Sweetener"
        / "Base-Cubes"
    )
    services.tracked_repos.manifest_path().parent.mkdir(parents=True, exist_ok=True)
    services.tracked_repos.manifest_path().write_text(
        json.dumps(
            {
                "repos": [
                    {
                        "owner": "Artificial-Sweetener",
                        "repo": "Base-Cubes",
                        "branch": "main",
                        "enabled": True,
                        "default_base_repo": True,
                        "local_checkout_path": str(stale_checkout),
                    }
                ]
            }
        ),
        encoding="utf-8",
    )
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(checkout / "demo.cube", _cube_payload())

    catalog = services.library.list_library_catalog()

    assert [entry["cubeId"] for entry in catalog["cubes"]] == [CANONICAL_CUBE_ID]


def test_backend_catalog_revision_changes_when_cube_content_changes(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Catalog revision should track artifact content without using generatedAt."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    cube_path = checkout / "demo.cube"
    _write_cube(cube_path, _cube_payload(version="1.0.0"))
    first_revision = services.library.catalog_revision()

    _write_cube(cube_path, _cube_payload(version="1.0.1"))
    second_revision = services.library.catalog_revision()

    assert first_revision != second_revision


def test_backend_catalog_revision_changes_for_same_second_same_size_edit(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Nanosecond mtime facts should catch direct same-size cube edits."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    cube_path = checkout / "demo.cube"
    first_payload = _cube_payload(version="1.0.0")
    second_payload = _cube_payload(version="1.0.0")
    second_payload["description"] = "Demo cubf"
    first_text = json.dumps(first_payload, indent=2) + "\n"
    second_text = json.dumps(second_payload, indent=2) + "\n"
    assert len(first_text) == len(second_text)
    cube_path.parent.mkdir(parents=True, exist_ok=True)

    cube_path.write_text(first_text, encoding="utf-8")
    os.utime(
        cube_path,
        ns=(1_700_000_000_100_000_000, 1_700_000_000_100_000_000),
    )
    first_revision = services.library.catalog_revision()
    cube_path.write_text(second_text, encoding="utf-8")
    os.utime(
        cube_path,
        ns=(1_700_000_000_900_000_000, 1_700_000_000_900_000_000),
    )
    second_revision = services.library.catalog_revision()

    assert first_revision != second_revision


def test_backend_catalog_reuses_hashes_for_unchanged_status_and_catalog(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Status and catalog calls should share one unchanged catalog snapshot."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(checkout / "demo.cube", _cube_payload())
    original_read = cube_library_service.read_cube_payload_with_hash
    read_paths: list[Path] = []

    def count_catalog_read(path: Path) -> Any:
        """Count catalog row reads while preserving the real digest."""

        read_paths.append(path)
        return original_read(path)

    monkeypatch.setattr(
        cube_library_service,
        "read_cube_payload_with_hash",
        count_catalog_read,
    )

    status = services.library.library_status()
    catalog = services.library.list_library_catalog()
    second_status = services.library.library_status()

    assert status["catalogRevision"] == catalog["catalogRevision"]
    assert second_status["catalogRevision"] == catalog["catalogRevision"]
    assert read_paths == [checkout / "demo.cube"]


def test_backend_catalog_invalidation_rebuilds_unchanged_stat_rows(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Explicit invalidation clears row reuse when stat facts are unchanged."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(checkout / "demo.cube", _cube_payload())
    original_read = cube_library_service.read_cube_payload_with_hash
    read_paths: list[Path] = []

    def count_catalog_read(path: Path) -> Any:
        """Count catalog row reads while preserving the real digest."""

        read_paths.append(path)
        return original_read(path)

    monkeypatch.setattr(
        cube_library_service,
        "read_cube_payload_with_hash",
        count_catalog_read,
    )

    first = services.library.catalog_revision()
    services.library.invalidate_catalog_state(reason="test")
    second = services.library.catalog_revision()

    assert first == second
    assert read_paths == [checkout / "demo.cube", checkout / "demo.cube"]


def test_backend_load_library_cube_returns_canonical_artifact(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Loaded artifacts should contain raw cube JSON plus remote-safe metadata."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(checkout / "demo.cube", _cube_payload())

    artifact = services.library.load_library_cube(CANONICAL_CUBE_ID)

    assert artifact["schemaVersion"] == 1
    assert artifact["cubeId"] == CANONICAL_CUBE_ID
    assert artifact["displayName"] == "demo"
    assert artifact["targetModel"] == ""
    assert artifact["supportedModels"] == []
    assert artifact["contentHash"].startswith("sha256:")
    assert artifact["source"]["repoRef"] == "Artificial-Sweetener/Base-Cubes"
    assert artifact["cube"]["cube_id"] == CANONICAL_CUBE_ID


def test_backend_lists_current_and_committed_cube_refs(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Cube refs expose exact revision and content identity."""

    historical = _revision_text(_cube_payload(version="0.9.0"))

    def fake_git(args: Any, *, cwd: Any) -> Any:
        class Result:
            stdout = ""

        if args == ["log", "--format=%H%x1f%cI%x1f%s", "--", "demo.cube"]:
            Result.stdout = "abc123\x1f2024-01-01T00:00:00+00:00\x1fold\n"
        elif args == ["show", "abc123:demo.cube"]:
            Result.stdout = historical
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    (checkout / ".git").mkdir(parents=True)
    _write_cube(checkout / "demo.cube", _cube_payload(version="1.0.0"))

    payload = services.library.list_library_cube_refs(CANONICAL_CUBE_ID)

    assert payload["schemaVersion"] == 1
    assert [ref["revisionRef"] for ref in payload["refs"]] == ["WORKTREE", "abc123"]
    assert [ref["version"] for ref in payload["refs"]] == ["1.0.0", "0.9.0"]
    assert payload["refs"][1]["contentHash"] == compute_cube_content_hash_bytes(
        historical.encode("utf-8")
    )


def test_backend_lists_unique_cube_versions_newest_first(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Version listing should collapse same-version refs in newest-first order."""

    historical_same = _revision_text(_cube_payload(version="1.0.0"))
    historical_old = _revision_text(_cube_payload(version="0.9.0"))

    def fake_git(args: Any, *, cwd: Any) -> Any:
        class Result:
            stdout = ""

        if args == ["log", "--format=%H%x1f%cI%x1f%s", "--", "demo.cube"]:
            Result.stdout = (
                "new123\x1f2024-02-01T00:00:00+00:00\x1fnew\n"
                "old123\x1f2024-01-01T00:00:00+00:00\x1fold\n"
            )
        elif args == ["show", "new123:demo.cube"]:
            Result.stdout = historical_same
        elif args == ["show", "old123:demo.cube"]:
            Result.stdout = historical_old
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    (checkout / ".git").mkdir(parents=True)
    _write_cube(checkout / "demo.cube", _cube_payload(version="1.1.0"))

    payload = services.library.list_library_cube_versions(CANONICAL_CUBE_ID)

    assert payload == {
        "schemaVersion": 1,
        "cubeId": CANONICAL_CUBE_ID,
        "versions": ["1.1.0", "1.0.0", "0.9.0"],
        "count": 3,
    }


def test_backend_loads_cube_artifact_by_revision_ref(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Revision selector returns the historical artifact, not the current file."""

    historical = _revision_text(_cube_payload(version="0.9.0"))

    def fake_git(args: Any, *, cwd: Any) -> Any:
        class Result:
            stdout = ""

        if args == ["log", "--format=%H%x1f%cI%x1f%s", "--", "demo.cube"]:
            Result.stdout = "abc123\x1f2024-01-01T00:00:00+00:00\x1fold\n"
        elif args == ["show", "abc123:demo.cube"]:
            Result.stdout = historical
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    (checkout / ".git").mkdir(parents=True)
    _write_cube(
        checkout / "demo.cube",
        _cube_payload(version="1.0.0", default_alias="current"),
    )

    artifact = services.library.load_library_cube_ref(
        cube_id=CANONICAL_CUBE_ID,
        revision_ref="abc123",
    )

    assert artifact["cubeId"] == CANONICAL_CUBE_ID
    assert artifact["version"] == "0.9.0"
    assert artifact["contentHash"] == compute_cube_content_hash_bytes(
        historical.encode("utf-8")
    )


def test_backend_loads_cube_artifact_by_unique_version(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """A version selector loads the newest matching artifact."""

    historical = _revision_text(_cube_payload(version="0.9.0"))

    def fake_git(args: Any, *, cwd: Any) -> Any:
        class Result:
            stdout = ""

        if args == ["log", "--format=%H%x1f%cI%x1f%s", "--", "demo.cube"]:
            Result.stdout = "abc123\x1f2024-01-01T00:00:00+00:00\x1fold\n"
        elif args == ["show", "abc123:demo.cube"]:
            Result.stdout = historical
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    (checkout / ".git").mkdir(parents=True)
    _write_cube(checkout / "demo.cube", _cube_payload(version="1.0.0"))

    artifact = services.library.load_library_cube_version(
        cube_id=CANONICAL_CUBE_ID,
        version="0.9.0",
    )

    assert artifact["version"] == "0.9.0"


def test_backend_loads_current_artifact_for_duplicate_current_version(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """A duplicate version selector should choose the current artifact first."""

    historical = _revision_text(_cube_payload(version="1.0.0"))
    historical_hash = compute_cube_content_hash_bytes(historical.encode("utf-8"))

    def fake_git(args: Any, *, cwd: Any) -> Any:
        if args[0] in {"log", "show"}:
            raise AssertionError("current-version load must not query git history")

        class Result:
            stdout = ""

        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    (checkout / ".git").mkdir(parents=True)
    _write_cube(
        checkout / "demo.cube",
        _cube_payload(version="1.0.0", default_alias="current"),
    )

    artifact = services.library.load_library_cube_version(
        cube_id=CANONICAL_CUBE_ID,
        version="1.0.0",
    )

    assert artifact["version"] == "1.0.0"
    assert artifact["contentHash"] != historical_hash


def test_backend_warm_historical_version_uses_disk_cache_without_git_show(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Repeated historical version loads should reuse the durable artifact cache."""

    historical = _revision_text(_cube_payload(version="1.0.0", default_alias="old"))
    calls: list[tuple[str, ...]] = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        calls.append(tuple(args))

        class Result:
            stdout = ""

        if args == ["rev-parse", "HEAD"]:
            Result.stdout = "head123\n"
        elif args == ["log", "--format=%H%x1f%cI%x1f%s", "--", "demo.cube"]:
            Result.stdout = "old123\x1f2024-01-01T00:00:00+00:00\x1fold\n"
        elif args == ["show", "old123:demo.cube"]:
            Result.stdout = historical
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    (checkout / ".git").mkdir(parents=True)
    _write_cube(checkout / "demo.cube", _cube_payload(version="2.0.0"))

    first = services.library.load_library_cube_version(
        cube_id=CANONICAL_CUBE_ID,
        version="1.0.0",
    )
    calls.clear()
    second = services.library.load_library_cube_version(
        cube_id=CANONICAL_CUBE_ID,
        version="1.0.0",
    )

    assert first["contentHash"] == second["contentHash"]
    assert ("log", "--format=%H%x1f%cI%x1f%s", "--", "demo.cube") not in calls
    assert ("show", "old123:demo.cube") not in calls


def test_backend_historical_version_cache_rebuilds_corrupt_artifact(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Corrupt cache files are treated as misses and rebuilt from git."""

    historical = _revision_text(_cube_payload(version="1.0.0", default_alias="old"))
    show_count = 0

    def fake_git(args: Any, *, cwd: Any) -> Any:
        nonlocal show_count

        class Result:
            stdout = ""

        if args == ["rev-parse", "HEAD"]:
            Result.stdout = "head123\n"
        elif args == ["log", "--format=%H%x1f%cI%x1f%s", "--", "demo.cube"]:
            Result.stdout = "old123\x1f2024-01-01T00:00:00+00:00\x1fold\n"
        elif args == ["show", "old123:demo.cube"]:
            show_count += 1
            Result.stdout = historical
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    (checkout / ".git").mkdir(parents=True)
    _write_cube(checkout / "demo.cube", _cube_payload(version="2.0.0"))

    services.library.load_library_cube_version(
        cube_id=CANONICAL_CUBE_ID,
        version="1.0.0",
    )
    cache_files = list(
        (services.library.artifacts.version_artifact_cache.artifact_root).glob("*.json")
    )
    assert cache_files
    cache_files[0].write_text("not json", encoding="utf-8")

    artifact = services.library.load_library_cube_version(
        cube_id=CANONICAL_CUBE_ID,
        version="1.0.0",
    )

    assert artifact["version"] == "1.0.0"
    assert show_count == 2


def test_backend_loads_newest_committed_artifact_for_duplicate_version(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """When current has another version, duplicate commits resolve by log order."""

    newest = _revision_text(_cube_payload(version="1.0.0", default_alias="newest"))
    older = _revision_text(_cube_payload(version="1.0.0", default_alias="older"))
    newest_hash = compute_cube_content_hash_bytes(newest.encode("utf-8"))

    def fake_git(args: Any, *, cwd: Any) -> Any:
        class Result:
            stdout = ""

        if args == ["log", "--format=%H%x1f%cI%x1f%s", "--", "demo.cube"]:
            Result.stdout = (
                "new123\x1f2024-02-01T00:00:00+00:00\x1fnew\n"
                "old123\x1f2024-01-01T00:00:00+00:00\x1fold\n"
            )
        elif args == ["show", "new123:demo.cube"]:
            Result.stdout = newest
        elif args == ["show", "old123:demo.cube"]:
            Result.stdout = older
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    (checkout / ".git").mkdir(parents=True)
    _write_cube(checkout / "demo.cube", _cube_payload(version="2.0.0"))

    artifact = services.library.load_library_cube_version(
        cube_id=CANONICAL_CUBE_ID,
        version="1.0.0",
    )

    assert artifact["contentHash"] == newest_hash


def test_backend_library_change_subscription_receives_notification(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Generic library-change listeners should receive immediate save metadata."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    events: list[dict[str, object]] = []

    unsubscribe = services.library.subscribe_library_changed(events.append)
    services.library.notify_library_changed(
        affected_cube_ids=[CANONICAL_CUBE_ID],
        saved_versions={CANONICAL_CUBE_ID: "1.2.3"},
        reason="cube_saved",
    )
    unsubscribe()
    services.library.notify_library_changed(
        affected_cube_ids=[CANONICAL_CUBE_ID],
        saved_versions={CANONICAL_CUBE_ID: "1.2.4"},
        reason="cube_saved",
    )

    assert len(events) == 1
    assert events[0]["affectedCubeIds"] == [CANONICAL_CUBE_ID]
    assert events[0]["savedVersions"] == {CANONICAL_CUBE_ID: "1.2.3"}
    assert events[0]["reason"] == "cube_saved"
    assert isinstance(events[0]["catalogRevision"], str)


def test_backend_rejects_revision_hash_mismatch(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Exact selectors must identify the same artifact payload."""

    historical = _revision_text(_cube_payload(version="0.9.0"))

    def fake_git(args: Any, *, cwd: Any) -> Any:
        class Result:
            stdout = ""

        if args == ["log", "--format=%H%x1f%cI%x1f%s", "--", "demo.cube"]:
            Result.stdout = "abc123\x1f2024-01-01T00:00:00+00:00\x1fold\n"
        elif args == ["show", "abc123:demo.cube"]:
            Result.stdout = historical
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    (checkout / ".git").mkdir(parents=True)
    _write_cube(checkout / "demo.cube", _cube_payload(version="1.0.0"))

    with pytest.raises(BackendError) as excinfo:
        services.library.load_library_cube_ref(
            cube_id=CANONICAL_CUBE_ID,
            revision_ref="abc123",
            content_hash="sha256:not-the-hash",
        )

    assert excinfo.value.status == 409


def test_backend_readiness_reports_target_missing_custom_nodes_without_install(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Readiness reports install-capable Base-Cubes dependency plans."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    (checkout / ".git").mkdir(parents=True)
    _write_cube(checkout / "demo.cube", _cube_payload_with_cnr())
    custom_nodes_root = tmp_path / "custom_nodes"
    custom_nodes_root.mkdir()

    readiness = services.library.library_readiness(custom_nodes_root)

    assert readiness["ready"] is False
    assert readiness["catalogRevision"] == services.library.catalog_revision()
    assert readiness["requiredCustomNodes"] == ["comfyui-impact-pack"]
    assert readiness["missingCustomNodes"] == ["comfyui-impact-pack"]
    assert readiness["installedCustomNodes"] == []
    assert readiness["canInstall"] is True
    assert readiness["installSupported"] is True
    assert readiness["restartRequired"] is True
    assert readiness["installPlan"] == [
        {
            "nodeId": "comfyui-impact-pack",
            "displayName": "comfyui-impact-pack",
            "existingFolderName": "",
            "requiredByPacks": ["Artificial-Sweetener/Base-Cubes"],
            "requiredByCubeIds": [CANONICAL_CUBE_ID],
            "defaultBaseOnly": True,
            "confirmationRequired": False,
            "installable": True,
            "installed": False,
            "remediation": "",
        }
    ]


def test_backend_readiness_reuses_summary_payload_for_dependency_facts(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Readiness should not resolve and reread cubes after summarizing them."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    cube_path = checkout / "demo.cube"
    _write_cube(cube_path, _cube_payload_with_cnr())
    custom_nodes_root = tmp_path / "custom_nodes"
    custom_nodes_root.mkdir()

    original_read_with_hash = cube_summary.read_cube_payload_with_hash
    read_paths: list[Path] = []

    def counted_read_with_hash(path: Path) -> Any:
        """Record payload reads used by the readiness summary pass."""

        read_paths.append(path)
        return original_read_with_hash(path)

    def fail_resolve_after_summary(cube_id: str) -> Path:
        """Readiness has enough summary facts to avoid this fallback path."""

        raise AssertionError(f"unexpected cube resolve for {cube_id}")

    monkeypatch.setattr(
        cube_summary,
        "read_cube_payload_with_hash",
        counted_read_with_hash,
    )
    monkeypatch.setattr(
        services.library, "resolve_cube_by_id", fail_resolve_after_summary
    )

    readiness = services.library.library_readiness(custom_nodes_root)

    assert readiness["requiredCustomNodes"] == ["comfyui-impact-pack"]
    assert read_paths == [cube_path]


def test_backend_readiness_reuses_tracked_repo_lookup_for_pack_summaries(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Readiness should not reload tracked repo state once per cube in a pack."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(checkout / "first.cube", _cube_payload_with_cnr())
    _write_cube(
        checkout / "second.cube",
        _cube_payload_with_cnr(
            cube_id="Artificial-Sweetener/Base-Cubes/second.cube",
            cnr_id="comfyui-second-pack",
        ),
    )
    custom_nodes_root = tmp_path / "custom_nodes"
    custom_nodes_root.mkdir()

    original_get_repo = services.tracked_repos.get_repo
    get_repo_calls: list[tuple[str, str]] = []

    def counted_get_repo(owner: str, repo: str) -> Any:
        """Record tracked-repo lookups while preserving real repo facts."""

        get_repo_calls.append((owner, repo))
        return original_get_repo(owner, repo)

    monkeypatch.setattr(services.tracked_repos, "get_repo", counted_get_repo)

    readiness = services.library.library_readiness(custom_nodes_root)

    assert "comfyui-second-pack" in readiness["requiredCustomNodes"]
    assert get_repo_calls == [("Artificial-Sweetener", "Base-Cubes")]


def test_backend_readiness_reuses_durable_dependency_requirement_cache(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Fresh service instances should reuse unchanged dependency requirements."""

    first_services = backend_services_factory(
        tmp_path, git_runner=lambda args, cwd: None
    )
    checkout = first_services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    cube_path = checkout / "demo.cube"
    _write_cube(cube_path, _cube_payload_with_cnr())
    custom_nodes_root = tmp_path / "custom_nodes"
    custom_nodes_root.mkdir()

    first_readiness = first_services.library.library_readiness(custom_nodes_root)

    assert first_readiness["requiredCustomNodes"] == ["comfyui-impact-pack"]

    second_services = backend_services_factory(
        tmp_path, git_runner=lambda args, cwd: None
    )

    def fail_payload_read(path: Path) -> None:
        """Fail if the durable requirement cache misses for an unchanged cube."""

        raise AssertionError(f"unexpected cube payload read for {path}")

    monkeypatch.setattr(
        cube_summary,
        "read_cube_payload_with_hash",
        fail_payload_read,
    )

    second_readiness = second_services.library.library_readiness(custom_nodes_root)

    assert second_readiness["requiredCustomNodes"] == ["comfyui-impact-pack"]
    assert second_readiness["catalogRevision"] == first_readiness["catalogRevision"]


def test_backend_readiness_invalidates_durable_dependency_requirement_cache(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Cube stat changes should force dependency requirements to be rebuilt."""

    first_services = backend_services_factory(
        tmp_path, git_runner=lambda args, cwd: None
    )
    checkout = first_services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    cube_path = checkout / "demo.cube"
    _write_cube(cube_path, _cube_payload_with_cnr(cnr_id="comfyui-impact-pack"))
    custom_nodes_root = tmp_path / "custom_nodes"
    custom_nodes_root.mkdir()

    first_readiness = first_services.library.library_readiness(custom_nodes_root)

    _write_cube(
        cube_path,
        _cube_payload_with_cnr(
            cnr_id="comfyui-second-pack-with-longer-cache-key",
            python_module="custom_nodes.comfyui-second-pack-with-longer-cache-key",
        ),
    )
    second_services = backend_services_factory(
        tmp_path, git_runner=lambda args, cwd: None
    )

    second_readiness = second_services.library.library_readiness(custom_nodes_root)

    assert first_readiness["requiredCustomNodes"] == ["comfyui-impact-pack"]
    assert second_readiness["requiredCustomNodes"] == [
        "comfyui-second-pack-with-longer-cache-key"
    ]
    assert second_readiness["catalogRevision"] != first_readiness["catalogRevision"]


def test_backend_readiness_cache_invalidates_when_cube_dependencies_change(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Process-local readiness reuse must not hide changed cube dependencies."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    cube_path = checkout / "demo.cube"
    _write_cube(cube_path, _cube_payload_with_cnr(cnr_id="comfyui-impact-pack"))
    custom_nodes_root = tmp_path / "custom_nodes"
    custom_nodes_root.mkdir()

    first_readiness = services.library.library_readiness(custom_nodes_root)
    _write_cube(
        cube_path,
        _cube_payload_with_cnr(
            cnr_id="comfyui-new-pack",
            python_module="custom_nodes.comfyui-new-pack",
        ),
    )
    second_readiness = services.library.library_readiness(custom_nodes_root)

    assert first_readiness["requiredCustomNodes"] == ["comfyui-impact-pack"]
    assert second_readiness["requiredCustomNodes"] == ["comfyui-new-pack"]
    assert second_readiness["catalogRevision"] != first_readiness["catalogRevision"]


def test_backend_readiness_cache_invalidates_when_new_cube_is_synced(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Process-local readiness reuse must notice newly added cube files."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(checkout / "demo.cube", _cube_payload_with_cnr())
    custom_nodes_root = tmp_path / "custom_nodes"
    custom_nodes_root.mkdir()

    first_readiness = services.library.library_readiness(custom_nodes_root)
    _write_cube(
        checkout / "new.cube",
        _cube_payload_with_cnr(
            cube_id="Artificial-Sweetener/Base-Cubes/new.cube",
            cnr_id="comfyui-new-pack",
            python_module="custom_nodes.comfyui-new-pack",
        ),
    )
    second_readiness = services.library.library_readiness(custom_nodes_root)

    assert first_readiness["requiredCustomNodes"] == ["comfyui-impact-pack"]
    assert second_readiness["requiredCustomNodes"] == [
        "comfyui-impact-pack",
        "comfyui-new-pack",
    ]
    assert second_readiness["catalogRevision"] != first_readiness["catalogRevision"]


def test_backend_readiness_logs_dependency_requirement_set_timing(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
    caplog: pytest.LogCaptureFixture,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Readiness diagnostics should expose dependency requirement subphase costs."""

    monkeypatch.setenv("SUGARCUBES_DIAGNOSTICS", "1")
    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    (checkout / ".git").mkdir(parents=True)
    _write_cube(checkout / "demo.cube", _cube_payload_with_cnr())
    (tmp_path / "custom_nodes" / "comfyui-impact-pack").mkdir(parents=True)

    with caplog.at_level(logging.INFO):
        readiness = services.library.library_readiness(tmp_path / "custom_nodes")

    assert readiness["requiredCustomNodes"] == ["comfyui-impact-pack"]
    messages = [record.message for record in caplog.records]
    diagnostic = next(
        message
        for message in messages
        if "event=sugarcubes_dependency_requirement_sets_timing" in message
    )
    assert "summary_count=1" in diagnostic
    assert "requirement_record_count=1" in diagnostic
    assert "version_requirement_count=1" in diagnostic
    assert "list_catalog_cube_summaries=" in diagnostic
    assert "source_metadata_for_summary=" in diagnostic
    assert "readiness_catalog_revision=" in diagnostic
    assert any(
        "event=sugarcubes_catalog_summary_listing_timing" in message
        and "repo_cube_count=1" in message
        and "total_cube_count=1" in message
        for message in messages
    )
    assert any(
        "event=sugarcubes_repo_cube_listing_timing" in message
        and "cube_count=1" in message
        and "list_cube_files=" in message
        and "summarize_cube_file=" in message
        and "annotate_cube_payload=" in message
        for message in messages
    )
    assert any(
        "event=sugarcubes_repo_dirty_paths_timing" in message
        and "cached=False" in message
        and "dirty_path_count=0" in message
        for message in messages
    )
    assert any(
        "event=sugarcubes_installed_dependency_inventory_timing" in message
        and "entry_count=1" in message
        and "list_custom_node_entries=" in message
        and "read_tracking_metadata=" in message
        and "probe_git_dir=" in message
        for message in messages
    )
    assert any(
        "event=sugarcubes_dependency_version_readiness_timing" in message
        and "installed_dependency_inventory=" in message
        and "plan_count=1" in message
        for message in messages
    )


def test_backend_readiness_requires_confirmation_for_non_default_packs(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Dependencies from non-default cube packs require user approval."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    services.tracked_repos.add_repo(
        owner="Example",
        repo="Cubes",
        branch="main",
        enabled=True,
        default_base_repo=False,
    )
    checkout = services.tracked_repos.checkout_path("Example", "Cubes")
    _write_cube(
        checkout / "demo.cube",
        _cube_payload_with_cnr(
            cube_id="Example/Cubes/demo.cube",
            cnr_id="comfyui-example",
            python_module="custom_nodes.comfyui-example",
        ),
    )

    readiness = services.library.library_readiness(tmp_path / "custom_nodes")

    assert readiness["missingCustomNodes"] == ["comfyui-example"]
    assert readiness["installPlan"][0]["requiredByPacks"] == ["Example/Cubes"]
    assert readiness["installPlan"][0]["confirmationRequired"] is True
    assert readiness["installPlan"][0]["defaultBaseOnly"] is False


def test_backend_readiness_omits_core_and_sugarcubes_markers(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Core Comfy nodes and SugarCubes marker modules are not install targets."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(
        checkout / "demo.cube",
        _cube_payload_with_cnr(cnr_id="comfy-core", python_module="nodes"),
    )

    readiness = services.library.library_readiness(tmp_path / "custom_nodes")

    assert readiness["ready"] is True
    assert readiness["requiredCustomNodes"] == []
    assert readiness["installPlan"] == []
    assert readiness["comfyRuntimeReadiness"]["requiredVersion"] == ""


def test_backend_readiness_preserves_versioned_custom_node_requirements(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Version readiness keeps cube `cnr_id` and sibling `ver` facts."""

    def fake_git(args: Any, *, cwd: Any) -> Any:
        class Result:
            returncode = 0
            stdout = ""

        if args == ["rev-parse", "HEAD"]:
            Result.stdout = "f561f164543f927e0452e14658a0509e8e4866d6\n"
        elif args == ["config", "--get", "remote.origin.url"]:
            Result.stdout = "https://github.com/Artificial-Sweetener/SimpleSyrup.git\n"
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(
        checkout / "demo.cube",
        _cube_payload_with_cnr(
            cnr_id="SimpleSyrup",
            version="37bcd403c5172adc2505b38d1d31c05969a69443",
            python_module="custom_nodes.SimpleSyrup",
        ),
    )
    custom_nodes_root = tmp_path / "custom_nodes"
    (custom_nodes_root / "SimpleSyrup" / ".git").mkdir(parents=True)

    readiness = services.library.library_readiness(custom_nodes_root)
    version_item = readiness["dependencyVersionPlan"][0]

    assert readiness["versionedRequirementsSupported"] is True
    assert version_item["nodeId"] == "SimpleSyrup"
    assert version_item["requiredVersionKind"] == "git_sha"
    assert (
        version_item["installedVersion"] == "f561f164543f927e0452e14658a0509e8e4866d6"
    )
    assert version_item["status"] == "satisfied"
    assert version_item["requiredByNodes"] == ["Impact Detailer"]


def test_backend_readiness_skips_git_runner_for_unrelated_installed_nodes(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Readiness should inspect full git state only for required custom nodes."""

    git_cwds: list[Path] = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        _ = args
        git_cwds.append(Path(cwd))

        class Result:
            returncode = 0
            stdout = "f561f164543f927e0452e14658a0509e8e4866d6\n"

        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(
        checkout / "demo.cube",
        _cube_payload_with_cnr(
            cnr_id="SimpleSyrup",
            version="37bcd403c5172adc2505b38d1d31c05969a69443",
            python_module="custom_nodes.SimpleSyrup",
        ),
    )
    custom_nodes_root = tmp_path / "custom_nodes"
    (custom_nodes_root / "SimpleSyrup" / ".git").mkdir(parents=True)
    unrelated_git = custom_nodes_root / "UnrelatedNode" / ".git"
    unrelated_git.mkdir(parents=True)
    (unrelated_git / "HEAD").write_text(
        "f561f164543f927e0452e14658a0509e8e4866d6\n",
        encoding="utf-8",
    )

    services.library.library_readiness(custom_nodes_root)

    assert custom_nodes_root / "SimpleSyrup" in git_cwds
    assert custom_nodes_root / "UnrelatedNode" not in git_cwds


def test_backend_readiness_reads_plain_git_metadata_without_subprocess(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Readiness should avoid git subprocesses for HEAD and origin metadata."""

    head = "f561f164543f927e0452e14658a0509e8e4866d6"
    git_calls: list[list[str]] = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        _ = cwd
        git_calls.append(list(args))

        class Result:
            returncode = 0
            stdout = ""

        if args == ["status", "--porcelain"]:
            Result.stdout = ""
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(
        checkout / "demo.cube",
        _cube_payload_with_cnr(
            cnr_id="SimpleSyrup",
            version=head,
            python_module="custom_nodes.SimpleSyrup",
        ),
    )
    custom_nodes_root = tmp_path / "custom_nodes"
    git_dir = custom_nodes_root / "SimpleSyrup" / ".git"
    git_dir.mkdir(parents=True)
    (git_dir / "HEAD").write_text(f"{head}\n", encoding="utf-8")
    (git_dir / "config").write_text(
        '[remote "origin"]\n'
        "\turl = https://github.com/Artificial-Sweetener/SimpleSyrup.git\n",
        encoding="utf-8",
    )

    readiness = services.library.library_readiness(custom_nodes_root)
    version_item = readiness["dependencyVersionPlan"][0]

    assert version_item["installedVersion"] == head
    assert version_item["installedEvidence"]["repositoryUrl"].endswith(
        "/SimpleSyrup.git"
    )
    assert ["rev-parse", "HEAD"] not in git_calls
    assert ["config", "--get", "remote.origin.url"] not in git_calls


def test_backend_readiness_skips_git_requirement_ancestry_when_checkout_dirty(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Dirty git checkouts should block readiness without merge-base probes."""

    installed_head = "225d0e5024a7751e80692f1c52dd3519be73cbab"
    git_calls: list[list[str]] = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        _ = cwd
        git_calls.append(list(args))

        class Result:
            returncode = 0
            stdout = ""

        if args == ["status", "--porcelain"]:
            Result.stdout = " M nodes.py\n"
        if args[:2] == ["merge-base", "--is-ancestor"]:
            raise AssertionError("dirty checkout should not require ancestry checks")
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(
        checkout / "first.cube",
        _cube_payload_with_cnr(
            cube_id="Artificial-Sweetener/Base-Cubes/first.cube",
            cnr_id="SimpleSyrup",
            version="37bcd403c5172adc2505b38d1d31c05969a69443",
            python_module="custom_nodes.SimpleSyrup",
        ),
    )
    _write_cube(
        checkout / "second.cube",
        _cube_payload_with_cnr(
            cube_id="Artificial-Sweetener/Base-Cubes/second.cube",
            cnr_id="SimpleSyrup",
            version="f561f164543f927e0452e14658a0509e8e4866d6",
            python_module="custom_nodes.SimpleSyrup",
        ),
    )
    custom_nodes_root = tmp_path / "custom_nodes"
    git_dir = custom_nodes_root / "SimpleSyrup" / ".git"
    git_dir.mkdir(parents=True)
    (git_dir / "HEAD").write_text(f"{installed_head}\n", encoding="utf-8")
    (git_dir / "config").write_text(
        '[remote "origin"]\n'
        "\turl = https://github.com/Artificial-Sweetener/SimpleSyrup.git\n",
        encoding="utf-8",
    )

    readiness = services.library.library_readiness(custom_nodes_root)
    version_item = readiness["dependencyVersionPlan"][0]

    assert version_item["status"] == "blocked"
    assert version_item["conflicts"] == []
    assert not any(call[:2] == ["merge-base", "--is-ancestor"] for call in git_calls)


def test_backend_readiness_reports_comfy_core_runtime_requirement(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """`comfy-core` is runtime readiness, not a custom-node install target."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(
        checkout / "demo.cube",
        _cube_payload_with_cnr(
            cnr_id="comfy-core",
            version="0.3.66",
            python_module="nodes",
        ),
    )

    readiness = services.library.library_readiness(tmp_path / "custom_nodes")

    assert readiness["requiredCustomNodes"] == []
    assert readiness["installPlan"] == []
    assert readiness["dependencyVersionPlan"] == []
    assert readiness["comfyRuntimeReadiness"]["requiredVersion"] == "0.3.66"
    assert readiness["comfyRuntimeReadiness"]["status"] == "installed_version_unknown"


def test_backend_pack_records_do_not_expose_local_checkout_paths(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Pack list payloads should not leak absolute target checkout paths."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(checkout / "demo.cube", _cube_payload())

    packs = services.library.list_library_packs()

    assert packs["packs"][0]["repoRef"] == "Artificial-Sweetener/Base-Cubes"
    assert packs["packs"][0]["cubeCount"] == 1
    assert "localCheckoutPath" not in packs["packs"][0]
    assert str(checkout) not in json.dumps(packs)

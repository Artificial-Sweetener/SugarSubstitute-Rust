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
"""Source-aware cube library services and metadata summarization."""

from __future__ import annotations

import json
import logging
import shutil
from datetime import datetime, timezone
from pathlib import Path
from time import perf_counter
from typing import Any, Callable, Collection, Mapping, Optional, Sequence

from ...cube_model import (
    CubeIdentityError,
    CanonicalCubeId,
    RESERVED_SOURCE_NAMES,
    apply_cube_identity_projection,
    parse_canonical_cube_id,
)
from ...importer import CubeImportError
from ...instrumentation import log_diagnostic, log_event
from ..responses import BackendError
from .cube_icon_service import (
    CubeIconError,
    attach_icon_url,
    normalize_existing_icon_metadata,
    normalize_icon_metadata,
    resolve_icon_asset_path,
)
from .cube_catalog_state_service import CubeCatalogStateService
from .cube_dependency_manifest import (
    iter_custom_node_slugs,
)
from .cube_file_io import (
    cleanup_failed_import,
    format_display_path,
    list_cube_files,
    read_cube_payload,
    read_cube_payload_with_hash as read_cube_payload_with_hash,
    safe_relative_path,
)
from .cube_metadata import (
    normalize_metadata_string,
)
from .cube_library_readiness_service import CubeLibraryReadinessService
from .cube_library_artifact_service import CubeLibraryArtifactService
from .cube_summary import (
    build_cube_identity_fields,
    dedupe_warnings,
    derive_cube_display_name,
    summarize_cube_file,
)
from .ownership_policy_service import OwnershipPolicyService
from .tracked_repo_service import TrackedRepo, TrackedRepoService

_logger = logging.getLogger(__name__)
CUBE_LIBRARY_TRACE_MARKER = "SugarCubes cube library diagnostic"
_LIBRARY_READINESS_CACHE_TTL_SECONDS = 30.0
_DEPENDENCY_REQUIREMENT_CACHE_SCHEMA_VERSION = 1
_DEPENDENCY_REQUIREMENT_CACHE_FILENAME = "dependency-requirements.json"


class DuplicateCubeIdConflict(RuntimeError):
    """Retain the historical duplicate-id error type during the migration."""


_LOCAL_WORKSPACE_NAME = "local"
_DEFAULT_BASE_REPO_REF = "Artificial-Sweetener/Base-Cubes"
_CURRENT_REVISION_REF = "WORKTREE"


def _path_mtime_ns(path: Path) -> int:
    """Return a file timestamp suitable for cheap cache invalidation."""

    try:
        return path.stat().st_mtime_ns
    except OSError:
        return 0


def _git_status_path(line: str) -> str:
    """Return the normalized path component from one porcelain status line."""

    if len(line) < 4:
        return ""
    path = line[3:].strip()
    if " -> " in path:
        path = path.rsplit(" -> ", 1)[-1]
    return path.strip('"').replace("\\", "/")


def _log_cube_library_diagnostic(event: str, **fields: object) -> None:
    """Emit a structured cube library diagnostic line in standard Comfy logs."""

    log_diagnostic(_logger, CUBE_LIBRARY_TRACE_MARKER, event, fields)


def _runtime_version() -> str:
    """Return the SugarCubes runtime version exposed to backend adapters."""

    from .. import __version__

    return normalize_metadata_string(__version__)




def _utc_now() -> str:
    """Return the current UTC timestamp for library API payloads."""

    return datetime.now(tz=timezone.utc).isoformat(timespec="seconds")


class CubeLibraryService:
    """Own source-aware cube listing, preview, import, delete, and path resolution."""

    def __init__(
        self,
        extension_root: Path,
        *,
        load_cube_artifact: Callable[[Path], Any],
        prepare_cube_import: Callable[..., Any],
        tracked_repo_service: TrackedRepoService,
        ownership_policy_service: OwnershipPolicyService,
        registry_factory: Optional[Callable[[Path], Any]] = None,
    ) -> None:
        """Initialize the cube library service."""

        self.extension_root = extension_root.resolve()
        self.load_cube_artifact = load_cube_artifact
        self.prepare_cube_import = prepare_cube_import
        self.tracked_repo_service = tracked_repo_service
        self.ownership_policy_service = ownership_policy_service
        self.registry_factory = registry_factory
        self.artifacts = CubeLibraryArtifactService(self)
        self._library_change_listeners: list[Callable[[dict[str, Any]], None]] = []
        self._catalog_state = CubeCatalogStateService(
            list_summaries=lambda include_disabled: self._list_catalog_cube_summaries(
                include_disabled=include_disabled
            ),
            build_entry=self._catalog_entry_for_summary,
            revision_pack_facts=lambda include_disabled: self._revision_pack_facts(
                include_disabled=include_disabled
            ),
            pack_counts=self._pack_counts,
            generated_at=_utc_now,
        )
        self._readiness = CubeLibraryReadinessService(self)
        self._repo_dirty_paths_cache: dict[Path, frozenset[str]] = {}

    def subscribe_library_changed(
        self,
        listener: Callable[[dict[str, Any]], None],
    ) -> Callable[[], None]:
        """Register a generic library-change listener and return an unsubscribe."""

        self._library_change_listeners.append(listener)

        def unsubscribe() -> None:
            """Remove the registered library-change listener."""

            try:
                self._library_change_listeners.remove(listener)
            except ValueError:
                return

        return unsubscribe

    def notify_library_changed(
        self,
        *,
        affected_cube_ids: Sequence[str],
        saved_versions: Mapping[str, str],
        reason: str,
    ) -> None:
        """Publish a generic library-change event to in-process consumers."""

        self.invalidate_catalog_state(
            reason=reason,
            affected_cube_ids=affected_cube_ids,
        )
        event = {
            "schemaVersion": 1,
            "affectedCubeIds": list(affected_cube_ids),
            "savedVersions": dict(saved_versions),
            "generatedAt": _utc_now(),
            "reason": reason,
            "catalogRevision": self.catalog_revision(),
        }
        for listener in tuple(self._library_change_listeners):
            try:
                listener(dict(event))
            except (RuntimeError, TypeError, ValueError):
                _logger.exception("SugarCubes library change listener failed")

    def invalidate_catalog_state(
        self,
        *,
        reason: str,
        affected_cube_ids: Sequence[str] = (),
    ) -> None:
        """Invalidate cached catalog state after a library-visible mutation."""

        self._catalog_state.invalidate(
            reason,
            affected_cube_ids=affected_cube_ids,
        )
        self._library_readiness_cache = None
        self._repo_dirty_paths_cache.clear()

    def repo_workspace_root(self) -> Path:
        """Return the managed tracked-repo workspace root."""

        return self.tracked_repo_service.workspace_root()

    def local_workspace_root(self) -> Path:
        """Return the managed local source workspace root."""

        return self.tracked_repo_service.ensure_local_repo()

    def list_cubes(self) -> dict[str, Any]:
        """Return the source-aware cube library response payload."""

        try:
            cubes: list[dict[str, Any]] = []
            repo_entries = self.tracked_repo_service.list_repos()["repos"]
            for repo_entry in repo_entries:
                if not repo_entry.get("enabled"):
                    continue
                tracked = TrackedRepo(
                    owner=repo_entry["owner"],
                    repo=repo_entry["repo"],
                    branch=repo_entry["branch"],
                    enabled=bool(repo_entry["enabled"]),
                    default_base_repo=bool(repo_entry["default_base_repo"]),
                    local_checkout_path=repo_entry["local_checkout_path"],
                    last_sync_at=repo_entry["last_sync_at"],
                    last_sync_status=repo_entry["last_sync_status"],
                    last_sync_error=repo_entry["last_sync_error"],
                )
                cubes.extend(self._list_repo_cubes(tracked))
            cubes.extend(self._list_local_cubes())
        except Exception as exc:  # pragma: no cover - defensive
            _logger.exception("SugarCubes: failed to list cubes")
            raise BackendError("Failed to list SugarCubes", status=500) from exc

        log_event(
            "frontend.phase5",
            "list_cubes",
            {
                "count": len(cubes),
                "directory": format_display_path(
                    self.repo_workspace_root(), self.extension_root
                ),
            },
        )
        return {
            "cubes": cubes,
            "directory": format_display_path(
                self.repo_workspace_root(), self.extension_root
            ),
            "exists": self.repo_workspace_root().exists()
            or self.local_workspace_root().exists(),
            "count": len(cubes),
        }

    def library_status(self) -> dict[str, Any]:
        """Return target-owned Cube Library availability for backend adapters."""

        try:
            catalog_revision = self.catalog_revision()
            available = True
            errors: list[dict[str, str]] = []
        except BackendError as exc:
            available = False
            catalog_revision = ""
            errors = [{"code": "catalog-unavailable", "message": exc.message}]
        except (OSError, RuntimeError, TypeError, ValueError):
            _logger.exception("SugarCubes: library status failed")
            available = False
            catalog_revision = ""
            errors = [
                {
                    "code": "catalog-unavailable",
                    "message": "SugarCubes catalog is unavailable.",
                }
            ]
        return {
            "schemaVersion": 1,
            "available": available,
            "source": "SugarCubes",
            "sugarCubesVersion": _runtime_version(),
            "catalogRevision": catalog_revision,
            "packManagementSupported": available,
            "localAuthoringSupported": available,
            "readinessSupported": available,
            "dependencyReadinessSupported": available,
            "dependencyRepairSupported": available,
            "versionedDependencyReadinessSupported": available,
            "syncDependencyOrchestrationSupported": available,
            "errors": errors,
        }

    def library_capabilities_status(self) -> dict[str, Any]:
        """Return Cube Library capability facts without building catalog state."""

        return {
            "schemaVersion": 1,
            "available": True,
            "source": "SugarCubes",
            "sugarCubesVersion": _runtime_version(),
            "catalogRevision": "",
            "packManagementSupported": True,
            "localAuthoringSupported": True,
            "readinessSupported": True,
            "dependencyReadinessSupported": True,
            "dependencyRepairSupported": True,
            "versionedDependencyReadinessSupported": True,
            "syncDependencyOrchestrationSupported": True,
            "errors": [],
        }

    def catalog_revision(self, *, include_disabled: bool = False) -> str:
        """Return a deterministic revision for catalog-relevant library state."""

        revision = self._catalog_state.current_revision(
            include_disabled=include_disabled
        )
        _log_cube_library_diagnostic(
            "sugarcubes_catalog_revision",
            include_disabled=include_disabled,
            catalog_revision=revision,
        )
        return revision

    def list_library_catalog(self, *, include_disabled: bool = False) -> dict[str, Any]:
        """Return backend-facing catalog metadata for enabled library cubes."""

        _log_cube_library_diagnostic(
            "sugarcubes_list_catalog_start",
            include_disabled=include_disabled,
        )
        payload = self._catalog_state.current_catalog(include_disabled=include_disabled)
        _log_cube_library_diagnostic(
            "sugarcubes_list_catalog_return",
            include_disabled=include_disabled,
            cube_count=len(payload["cubes"]),
            catalog_revision=payload["catalogRevision"],
        )
        return payload

    def load_library_cube(self, cube_id: str) -> dict[str, Any]:
        """Return the canonical cube document and source metadata for one cube id."""

        return self.artifacts.load_library_cube(cube_id)

    def list_library_cube_refs(self, cube_id: str) -> dict[str, Any]:
        """Return exact artifact refs available for one cube id."""

        return self.artifacts.list_library_cube_refs(cube_id)

    def list_library_cube_versions(self, cube_id: str) -> dict[str, Any]:
        """Return unique versions available for one cube id, newest first."""

        return self.artifacts.list_library_cube_versions(cube_id)

    def load_library_cube_version(
        self,
        *,
        cube_id: str,
        version: str,
    ) -> dict[str, Any]:
        """Load the newest artifact for a cube id and version."""

        return self.artifacts.load_library_cube_version(
            cube_id=cube_id,
            version=version,
        )

    def load_library_cube_ref(
        self,
        *,
        cube_id: str,
        revision_ref: str = "",
        content_hash: str = "",
        version: str = "",
    ) -> dict[str, Any]:
        """Load a Cube Library artifact by an exact or resolved selector."""

        return self.artifacts.load_library_cube_ref(
            cube_id=cube_id,
            revision_ref=revision_ref,
            content_hash=content_hash,
            version=version,
        )

    def warm_library_cube_version(self, *, cube_id: str, version: str) -> None:
        """Schedule a best-effort historical cube version cache fill."""

        self.artifacts.warm_library_cube_version(cube_id=cube_id, version=version)


    def list_library_packs(self) -> dict[str, Any]:
        """Return tracked Cube Pack records without exposing checkout paths."""

        packs = [
            self._pack_record(repo_entry)
            for repo_entry in self.tracked_repo_service.list_repos()["repos"]
        ]
        packs.sort(key=lambda pack: str(pack.get("repoRef", "")).casefold())
        return {
            "schemaVersion": 1,
            "packs": packs,
            "catalogRevision": self.catalog_revision(),
        }

    def preflight_library_pack(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
    ) -> dict[str, Any]:
        """Return preflight information for a candidate Cube Pack."""

        payload = self.tracked_repo_service.preflight_repo(
            owner=owner,
            repo=repo,
            branch=branch,
        )
        return {"schemaVersion": 1, **payload}

    def add_library_pack(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
        enabled: bool,
        auto_update: bool,
        sync_immediately: bool,
    ) -> dict[str, Any]:
        """Track a Cube Pack and optionally perform the first synchronous sync."""

        payload = self.tracked_repo_service.add_repo(
            owner=owner,
            repo=repo,
            branch=branch,
            enabled=enabled,
            default_base_repo=False,
            auto_update=auto_update,
        )
        if sync_immediately:
            payload = {
                **payload,
                "repo": self.tracked_repo_service.sync_repo(
                    owner=owner,
                    repo=repo,
                )["repo"],
            }
        self.invalidate_catalog_state(reason="pack_added")
        return {
            "schemaVersion": 1,
            "pack": self._pack_record(payload["repo"]),
            "preflight": payload.get("preflight", {}),
            "catalogRevision": self.catalog_revision(),
        }

    def update_library_pack(
        self,
        *,
        owner: str,
        repo: str,
        branch: str | None,
        enabled: bool | None,
        auto_update: bool | None,
    ) -> dict[str, Any]:
        """Update a tracked Cube Pack and return refreshed library state."""

        payload = self.tracked_repo_service.update_repo(
            owner=owner,
            repo=repo,
            branch=branch,
            enabled=enabled,
            auto_update=auto_update,
        )
        self.invalidate_catalog_state(reason="pack_updated")
        return {
            "schemaVersion": 1,
            "pack": self._pack_record(payload["repo"]),
            "catalogRevision": self.catalog_revision(),
        }

    def remove_library_pack(self, *, owner: str, repo: str) -> dict[str, Any]:
        """Remove a tracked Cube Pack through SugarCubes policy enforcement."""

        payload = self.tracked_repo_service.remove_repo(owner=owner, repo=repo)
        self.invalidate_catalog_state(reason="pack_removed")
        return {
            "schemaVersion": 1,
            **payload,
            "catalogRevision": self.catalog_revision(),
        }

    def sync_library_pack(self, *, owner: str, repo: str) -> dict[str, Any]:
        """Synchronously sync one tracked Cube Pack."""

        payload = self.tracked_repo_service.sync_repo(owner=owner, repo=repo)
        self.invalidate_catalog_state(reason="pack_synced")
        return {
            "schemaVersion": 1,
            "pack": self._pack_record(payload["repo"]),
            "catalogRevision": self.catalog_revision(),
        }

    def sync_all_library_packs(self) -> dict[str, Any]:
        """Synchronously sync all enabled Cube Packs and return per-pack results."""

        payload = self.tracked_repo_service.sync_all_repos()
        self.invalidate_catalog_state(reason="all_packs_synced")
        return {
            "schemaVersion": 1,
            "packs": [self._pack_record(repo) for repo in payload["repos"]],
            "catalogRevision": self.catalog_revision(),
        }

    def library_readiness(self, custom_nodes_root: Path) -> dict[str, Any]:
        """Return target dependency readiness and install plan for enabled cubes."""

        return self._readiness.library_readiness(custom_nodes_root)


    def summarize_cube(self, cube_path: Path) -> dict[str, Any]:
        """Summarize a single cube file for browser payloads."""

        source = self.resolve_source_descriptor_by_path(cube_path)
        summary = summarize_cube_file(
            cube_path,
            Path(source["base_dir"]),
            self.extension_root,
            source_kind=source["source_kind"],
            owner=source["owner"],
            repo=source["repo"],
            namespace=source["namespace"],
        )
        return self.ownership_policy_service.annotate_cube_payload(summary)


    def resolve_cube_by_id(self, cube_id: str) -> Path:
        """Resolve a source-owned cube path by canonical cube id."""

        normalized = normalize_metadata_string(cube_id)
        if not normalized:
            raise BackendError("Cube id is required", status=400)
        try:
            parsed = parse_canonical_cube_id(normalized)
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        base_dir = self.resolve_source_base_dir(parsed)
        cube_path = (base_dir / Path(parsed.path)).resolve()
        try:
            cube_path.relative_to(base_dir)
        except ValueError as exc:
            raise BackendError(
                "Cube id path must stay within the managed source", status=400
            ) from exc
        if not cube_path.exists() or not cube_path.is_file():
            raise BackendError(f"Cube '{normalized}' not found", status=404)
        _log_cube_library_diagnostic(
            "sugarcubes_resolve_cube_by_id",
            cube_id=normalized,
            cube_path=format_display_path(cube_path, self.extension_root),
        )
        return cube_path

    def resolve_cube_target_path(self, cube_id: str) -> Path:
        """Resolve the managed destination path for one canonical cube id."""

        normalized = normalize_metadata_string(cube_id)
        if not normalized:
            raise BackendError("Cube id is required", status=400)
        try:
            parsed = parse_canonical_cube_id(normalized)
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        base_dir = self.resolve_source_base_dir(parsed)
        target_path = (base_dir / Path(parsed.path)).resolve()
        try:
            target_path.relative_to(base_dir)
        except ValueError as exc:
            raise BackendError(
                "Cube id path must stay within the managed source", status=400
            ) from exc
        return target_path

    def build_default_alias_lookup(self, cube_ids: Collection[str]) -> dict[str, str]:
        """Build a cube-id to display-name lookup for export flows."""

        lookup: dict[str, str] = {}
        for cube_id in cube_ids:
            normalized = normalize_metadata_string(cube_id)
            if not normalized:
                continue
            try:
                path = self.resolve_cube_by_id(normalized)
            except BackendError as exc:
                if exc.status == 404:
                    continue
                raise
            payload, error = read_cube_payload(path)
            name = (
                derive_cube_display_name(payload, path.stem)
                if not error
                else normalize_metadata_string(path.stem)
            )
            if not name:
                continue
            lookup[normalized] = name
        return lookup

    def preview_cube(self, cube_id: str) -> dict[str, Any]:
        """Return the lightweight preview payload used by the cube browser."""

        cube_path = self.resolve_cube_by_id(cube_id)
        try:
            loaded_cube = self.load_cube_artifact(cube_path)
            prepared = self.prepare_cube_import(loaded_cube, drop_origin=(0.0, 0.0))
        except CubeImportError:
            raise
        except Exception as exc:  # pragma: no cover - defensive
            _logger.exception("SugarCubes: preview failed for cube '%s'", cube_id)
            raise BackendError("Preview failed", status=500) from exc

        layout = loaded_cube.layout
        collapsed_nodes = 0
        styled_nodes = 0
        if layout:
            for entry in layout.nodes.values():
                if entry.extra.get("collapsed"):
                    collapsed_nodes += 1
                if any(key in entry.extra for key in ("color", "bgcolor", "style")):
                    styled_nodes += 1

        layout_summary = {
            "present": bool(layout),
            "groups": len(layout.groups) if layout else 0,
            "collapsed_nodes": collapsed_nodes,
            "styled_nodes": styled_nodes,
            "ds": layout.ds if layout else {"scale": 1.0, "offset": [0.0, 0.0]},
        }
        stats = {
            "nodes": len(loaded_cube.nodes),
            "markers": len(loaded_cube.markers),
            "inputs": len(loaded_cube.inputs),
            "outputs": len(loaded_cube.outputs),
            "definitions": len(loaded_cube.definitions),
            "prepared_nodes": len(prepared.nodes),
            "prepared_markers": len(prepared.markers),
            "connections": len(prepared.connections),
        }
        warnings = dedupe_warnings(list(loaded_cube.warnings) + list(prepared.warnings))
        source = self.resolve_source_descriptor_by_path(cube_path)
        base_dir = Path(source["base_dir"])
        icon = attach_icon_url(
            normalize_existing_icon_metadata(loaded_cube.metadata.get("icon")),
            loaded_cube.cube_id,
        )

        log_event(
            "frontend.phase5",
            "preview_cube",
            {
                "path": safe_relative_path(cube_path, base_dir)
                or format_display_path(cube_path, self.extension_root),
                "node_count": stats["nodes"],
                "marker_count": stats["markers"],
                "layout_present": layout_summary["present"],
            },
        )
        return {
            "cube": {
                "name": cube_path.stem,
                **build_cube_identity_fields(
                    cube_id=loaded_cube.cube_id,
                    default_alias=normalize_metadata_string(
                        loaded_cube.metadata.get("default_alias")
                    )
                    or cube_path.stem,
                    metadata=loaded_cube.metadata,
                ),
                "description": loaded_cube.description,
                "metadata": loaded_cube.metadata,
                "icon": icon,
                "cube_id": loaded_cube.cube_id,
                "version": loaded_cube.version,
            },
            "stats": stats,
            "layout": layout_summary,
            "warnings": warnings,
            "source": {
                "path": format_display_path(cube_path, self.extension_root),
                "relative_path": safe_relative_path(cube_path, base_dir),
                "type": source["source_kind"],
                "owner": source["owner"],
                "repo": source["repo"],
                "repo_ref": source["repo_ref"],
                "namespace": source["namespace"],
            },
        }

    def resolve_cube_icon_asset(self, cube_id: str) -> tuple[Path, str]:
        """Return the resolved icon file and media type for one cube."""

        normalized_cube_id = normalize_metadata_string(cube_id)
        if not normalized_cube_id:
            raise BackendError("'cube_id' query parameter is required", status=400)

        cube_path = self.resolve_cube_by_id(normalized_cube_id)
        payload, error = read_cube_payload(cube_path)
        if error or not payload:
            raise BackendError(error or "Invalid cube payload", status=400)
        metadata = payload.get("metadata")
        icon_source = metadata.get("icon") if isinstance(metadata, Mapping) else None
        try:
            icon = normalize_icon_metadata(icon_source)
        except CubeIconError as exc:
            raise BackendError(str(exc), status=404) from exc
        if not icon:
            raise BackendError("Cube icon not found", status=404)

        source = self.resolve_source_descriptor_by_path(cube_path)
        try:
            icon_path = resolve_icon_asset_path(Path(source["base_dir"]), icon)
        except CubeIconError as exc:
            raise BackendError(str(exc), status=404) from exc
        if not icon_path.exists() or not icon_path.is_file():
            raise BackendError("Cube icon asset not found", status=404)
        return icon_path, icon["media_type"]

    def import_cube_file(
        self,
        *,
        source_value: str,
        target_cube_id: str,
        overwrite: bool,
    ) -> dict[str, Any]:
        """Copy one external `.cube` file into a canonical managed source location."""

        source_path = Path(source_value).expanduser()
        if not source_path.exists() or not source_path.is_file():
            raise BackendError(f"Source cube '{source_value}' not found", status=404)
        if source_path.suffix.lower() != ".cube":
            raise BackendError("Source must be a .cube file", status=400)

        normalized_target_cube_id = normalize_metadata_string(target_cube_id)
        if not normalized_target_cube_id:
            raise BackendError("'cube_id' field is required", status=400)
        try:
            parsed = parse_canonical_cube_id(normalized_target_cube_id)
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        self.ownership_policy_service.assert_cube_id_writable(
            normalized_target_cube_id,
            action="import a cube into that destination",
        )

        resolved_dir = self.resolve_source_base_dir(parsed)
        resolved_dir.mkdir(parents=True, exist_ok=True)
        dest_path = (resolved_dir / Path(parsed.path)).resolve()
        try:
            dest_path.relative_to(resolved_dir)
        except ValueError as exc:
            raise BackendError("Invalid destination", status=400) from exc
        dest_path.parent.mkdir(parents=True, exist_ok=True)
        if dest_path.exists() and not overwrite:
            raise BackendError(
                f"Cube '{normalized_target_cube_id}' already exists", status=409
            )

        try:
            shutil.copy2(source_path, dest_path)
        except OSError as exc:
            _logger.exception(
                "SugarCubes: failed to import cube from %s to %s",
                source_path,
                dest_path,
            )
            raise BackendError("Failed to import cube", status=500) from exc

        payload, error = read_cube_payload(dest_path)
        if payload and not error:
            payload_dict = dict(payload)
            previous_cube_id = normalize_metadata_string(payload_dict.get("cube_id"))
            payload_dict["cube_id"] = normalized_target_cube_id
            try:
                apply_cube_identity_projection(
                    payload_dict, previous_cube_id=previous_cube_id
                )
                with dest_path.open("w", encoding="utf-8") as handle:
                    json.dump(payload_dict, handle, indent=2)
                    handle.write("\n")
            except (OSError, TypeError, ValueError) as exc:
                cleanup_failed_import(dest_path)
                _logger.exception(
                    "SugarCubes: failed to persist imported cube identity for %s",
                    dest_path,
                )
                raise BackendError("Failed to import cube", status=500) from exc

        try:
            self.load_cube_artifact(dest_path)
        except CubeImportError:
            cleanup_failed_import(dest_path)
            raise
        except Exception as exc:
            cleanup_failed_import(dest_path)
            _logger.exception(
                "SugarCubes: imported cube failed validation for %s",
                dest_path,
            )
            raise BackendError("Imported cube failed validation", status=500) from exc

        log_event(
            "frontend.phase5",
            "import_cube_file",
            {
                "source": str(source_path.name),
                "dest": safe_relative_path(dest_path, resolved_dir)
                or normalized_target_cube_id,
            },
        )
        self.invalidate_catalog_state(
            reason="cube_imported",
            affected_cube_ids=[normalized_target_cube_id],
        )
        return {"cube": self.summarize_cube(dest_path)}

    def delete_cube(
        self,
        *,
        cube_id: str,
    ) -> dict[str, Any]:
        """Delete a tracked cube by canonical id."""

        self.ownership_policy_service.assert_cube_id_writable(
            cube_id,
            action="delete this cube",
        )
        cube_path = self.resolve_cube_by_id(cube_id)

        try:
            cube_path.unlink()
        except FileNotFoundError as exc:
            raise BackendError("Cube already removed", status=404) from exc
        except OSError as exc:
            _logger.exception("SugarCubes: failed to delete cube %s", cube_path)
            raise BackendError("Failed to delete cube", status=500) from exc

        source = self.resolve_source_descriptor_by_path(cube_path)
        base_dir = Path(source["base_dir"])
        log_event(
            "frontend.phase5",
            "delete_cube",
            {
                "path": safe_relative_path(cube_path, base_dir)
                or format_display_path(cube_path, self.extension_root)
            },
        )
        self.invalidate_catalog_state(
            reason="cube_deleted", affected_cube_ids=[cube_id]
        )
        return {
            "status": "deleted",
            "cube": format_display_path(cube_path, self.extension_root),
        }

    def _list_repo_cubes(
        self,
        tracked: TrackedRepo,
        *,
        include_internal_payload: bool = False,
    ) -> list[dict[str, Any]]:
        """List all cube files under one tracked repo checkout."""

        started_at = perf_counter()
        phase_started_at = started_at

        def record_phase(name: str) -> None:
            """Accumulate elapsed milliseconds for one repo cube listing phase."""

            nonlocal phase_started_at
            now = perf_counter()
            phase_timings[name] = round(
                phase_timings.get(name, 0.0) + ((now - phase_started_at) * 1000),
                3,
            )
            phase_started_at = now

        phase_timings: dict[str, float] = {}
        checkout_path = Path(tracked.local_checkout_path).resolve()
        if not checkout_path.exists():
            return []
        cube_files = [
            path for path in list_cube_files(checkout_path) if ".git" not in path.parts
        ]
        record_phase("list_cube_files")
        cubes: list[dict[str, Any]] = []
        for path in cube_files:
            summary = summarize_cube_file(
                path,
                checkout_path,
                self.extension_root,
                source_kind="github",
                owner=tracked.owner,
                repo=tracked.repo,
                include_internal_payload=include_internal_payload,
            )
            record_phase("summarize_cube_file")
            cubes.append(self.ownership_policy_service.annotate_cube_payload(summary))
            record_phase("annotate_cube_payload")
        _log_cube_library_diagnostic(
            "sugarcubes_repo_cube_listing_timing",
            total_duration_ms=round((perf_counter() - started_at) * 1000, 3),
            repo_ref=tracked.repo_ref,
            cube_count=len(cubes),
            include_internal_payload=include_internal_payload,
            **phase_timings,
        )
        return cubes

    def _list_local_cubes(
        self,
        *,
        include_internal_payload: bool = False,
    ) -> list[dict[str, Any]]:
        """List all cube files under the managed local source workspace."""

        workspace_root = self.local_workspace_root().resolve()
        if not workspace_root.exists():
            return []
        cubes: list[dict[str, Any]] = []
        namespace_dirs = [path for path in workspace_root.iterdir() if path.is_dir()]
        for namespace_dir in sorted(namespace_dirs):
            namespace = namespace_dir.name
            if namespace.lower() in RESERVED_SOURCE_NAMES:
                continue
            for path in list_cube_files(namespace_dir):
                cubes.append(
                    self.ownership_policy_service.annotate_cube_payload(
                        summarize_cube_file(
                            path,
                            namespace_dir,
                            self.extension_root,
                            source_kind="local",
                            namespace=namespace,
                            include_internal_payload=include_internal_payload,
                        )
                    )
                )
        return cubes

    def _list_catalog_cube_summaries(
        self,
        *,
        include_disabled: bool,
        include_internal_payload: bool = False,
    ) -> list[dict[str, Any]]:
        """List cube summaries for the backend-facing Cube Library catalog."""

        started_at = perf_counter()
        phase_started_at = started_at
        phase_timings: dict[str, float] = {}

        def record_phase(name: str) -> None:
            """Record elapsed milliseconds for one catalog summary listing phase."""

            nonlocal phase_started_at
            now = perf_counter()
            phase_timings[name] = round((now - phase_started_at) * 1000, 3)
            phase_started_at = now

        cubes: list[dict[str, Any]] = []
        repo_entries = self.tracked_repo_service.list_repos()["repos"]
        record_phase("list_repos")
        enabled_repo_count = 0
        repo_cube_count = 0
        for repo_entry in repo_entries:
            if not include_disabled and not repo_entry.get("enabled"):
                continue
            enabled_repo_count += 1
            tracked = self._tracked_repo_from_payload(repo_entry)
            repo_cubes = self._list_repo_cubes(
                tracked,
                include_internal_payload=include_internal_payload,
            )
            repo_cube_count += len(repo_cubes)
            cubes.extend(repo_cubes)
        record_phase("list_repo_cubes")
        local_cubes = self._list_local_cubes(
            include_internal_payload=include_internal_payload
        )
        record_phase("list_local_cubes")
        cubes.extend(local_cubes)
        _log_cube_library_diagnostic(
            "sugarcubes_catalog_summary_listing_timing",
            total_duration_ms=round((perf_counter() - started_at) * 1000, 3),
            include_disabled=include_disabled,
            include_internal_payload=include_internal_payload,
            repo_count=len(repo_entries),
            enabled_repo_count=enabled_repo_count,
            repo_cube_count=repo_cube_count,
            local_cube_count=len(local_cubes),
            total_cube_count=len(cubes),
            **phase_timings,
        )
        return cubes

    def _catalog_entry_for_summary(self, summary: Mapping[str, Any]) -> dict[str, Any]:
        """Convert a browser cube summary into a backend catalog entry."""

        cube_id = normalize_metadata_string(summary.get("cube_id"))
        payload, _, content_hash = self._summary_payload_with_hash(summary)
        icon = summary.get("icon") if isinstance(summary.get("icon"), Mapping) else None
        entry: dict[str, Any] = {
            "cubeId": cube_id,
            "displayName": normalize_metadata_string(summary.get("display_name"))
            or normalize_metadata_string(summary.get("name")),
            "version": normalize_metadata_string(summary.get("version")),
            "description": normalize_metadata_string(summary.get("description")),
            "targetModel": normalize_metadata_string(summary.get("target_model")),
            "supportedModels": list(summary.get("supported_models") or []),
            "source": self._source_metadata_for_summary(summary),
            "contentHash": content_hash,
            "updatedAt": normalize_metadata_string(summary.get("mtime")),
        }
        _log_cube_library_diagnostic(
            "sugarcubes_catalog_entry",
            cube_id=cube_id,
            version=entry["version"],
            content_hash=entry["contentHash"],
            source_kind=(
                entry["source"].get("kind", "")
                if isinstance(entry.get("source"), Mapping)
                else ""
            ),
            source_path=(
                entry["source"].get("path", "")
                if isinstance(entry.get("source"), Mapping)
                else ""
            ),
        )
        if icon:
            entry["icon"] = dict(icon)
        if payload:
            requirements = iter_custom_node_slugs(payload)
            if requirements:
                entry["requiredCustomNodes"] = list(requirements)
        return entry

    def _summary_payload_with_hash(
        self,
        summary: Mapping[str, Any],
    ) -> tuple[Optional[Mapping[str, Any]], Optional[str], str]:
        """Return payload and hash from internal summary facts or a fallback read."""

        if "_content_hash" in summary:
            payload = summary.get("_payload")
            return (
                dict(payload) if isinstance(payload, Mapping) else None,
                normalize_metadata_string(summary.get("error")) or None,
                normalize_metadata_string(summary.get("_content_hash")),
            )
        cube_id = normalize_metadata_string(summary.get("cube_id"))
        cube_path = self.resolve_cube_by_id(cube_id)
        return read_cube_payload_with_hash(cube_path)

    def _source_metadata_for_summary(
        self,
        summary: Mapping[str, Any],
        *,
        repo_cache: dict[tuple[str, str], TrackedRepo] | None = None,
    ) -> dict[str, Any]:
        """Build API source metadata for one summarized cube."""

        cube_id = normalize_metadata_string(summary.get("cube_id"))
        source_value = summary.get("source")
        source = source_value if isinstance(source_value, Mapping) else {}
        source_kind = normalize_metadata_string(
            source.get("type")
        ) or normalize_metadata_string(summary.get("source_kind"))
        if source_kind == "github":
            owner = normalize_metadata_string(
                source.get("owner") or summary.get("owner")
            )
            repo = normalize_metadata_string(source.get("repo") or summary.get("repo"))
            tracked = self._tracked_repo_for_source(
                owner=owner,
                repo=repo,
                repo_cache=repo_cache,
            )
            base_dir = Path(tracked.local_checkout_path).resolve()
            relative_path = normalize_metadata_string(
                source.get("repo_relative_path") or summary.get("relative_path")
            )
            return {
                "kind": "github",
                "repoRef": f"{owner}/{repo}",
                "owner": owner,
                "repo": repo,
                "branch": tracked.branch,
                "path": relative_path,
                "localHeadSha": self._local_head_sha(tracked),
                "remoteHeadSha": tracked.remote_head_sha,
                "dirty": self._is_repo_path_dirty(base_dir, relative_path),
            }
        namespace = normalize_metadata_string(
            source.get("namespace") or summary.get("namespace")
        )
        source_path = self._local_source_relative_path(cube_id)
        return self._local_source_metadata(
            namespace=namespace,
            source_path=source_path,
            repo_root=self.local_workspace_root().resolve(),
            repo_relative_path=f"{namespace}/{source_path}",
        )

    def _tracked_repo_for_source(
        self,
        *,
        owner: str,
        repo: str,
        repo_cache: dict[tuple[str, str], TrackedRepo] | None,
    ) -> TrackedRepo:
        """Return tracked repo facts, reusing manifest lookups within one pass."""

        if repo_cache is None:
            return self.tracked_repo_service.get_repo(owner, repo)
        cache_key = (owner.casefold(), repo.casefold())
        cached = repo_cache.get(cache_key)
        if cached is not None:
            return cached
        tracked = self.tracked_repo_service.get_repo(owner, repo)
        repo_cache[cache_key] = tracked
        return tracked

    def _pack_record(self, repo_entry: Mapping[str, Any]) -> dict[str, Any]:
        """Return an API-safe Cube Pack record from SugarCubes repo state."""

        owner = normalize_metadata_string(repo_entry.get("owner"))
        repo = normalize_metadata_string(repo_entry.get("repo"))
        repo_ref = (
            normalize_metadata_string(repo_entry.get("repo_ref")) or f"{owner}/{repo}"
        )
        checkout_path_raw = normalize_metadata_string(
            repo_entry.get("local_checkout_path")
        )
        checkout_path = (
            Path(checkout_path_raw) if checkout_path_raw else Path("__missing__")
        )
        return {
            "repoRef": repo_ref,
            "owner": owner,
            "repo": repo,
            "branch": normalize_metadata_string(repo_entry.get("branch")) or "main",
            "enabled": bool(repo_entry.get("enabled")),
            "defaultBaseRepo": bool(repo_entry.get("default_base_repo")),
            "autoUpdate": bool(repo_entry.get("auto_update")),
            "localHeadSha": normalize_metadata_string(repo_entry.get("local_head_sha")),
            "remoteHeadSha": normalize_metadata_string(
                repo_entry.get("remote_head_sha")
            ),
            "updateAvailable": bool(repo_entry.get("update_available")),
            "lastSyncAt": normalize_metadata_string(repo_entry.get("last_sync_at")),
            "lastSyncStatus": normalize_metadata_string(
                repo_entry.get("last_sync_status")
            )
            or "never",
            "lastSyncError": normalize_metadata_string(
                repo_entry.get("last_sync_error")
            ),
            "lastCheckedAt": normalize_metadata_string(
                repo_entry.get("last_checked_at")
            ),
            "lastCheckStatus": normalize_metadata_string(
                repo_entry.get("last_check_status")
            )
            or "never",
            "lastCheckError": normalize_metadata_string(
                repo_entry.get("last_check_error")
            ),
            "cubeCount": self._count_cube_files(checkout_path),
        }

    def _pack_counts(self) -> dict[str, int]:
        """Return count metadata for tracked Cube Packs."""

        repos = self.tracked_repo_service.list_repos()["repos"]
        return {
            "count": len(repos),
            "enabledCount": sum(1 for repo in repos if repo.get("enabled")),
        }

    def _revision_pack_facts(self, *, include_disabled: bool) -> list[dict[str, Any]]:
        """Return normalized pack facts used to compute catalog revisions."""

        facts: list[dict[str, Any]] = []
        for repo in self.tracked_repo_service.list_repos()["repos"]:
            if not include_disabled and not repo.get("enabled"):
                continue
            facts.append(
                {
                    "repo_ref": repo.get("repo_ref"),
                    "branch": repo.get("branch"),
                    "enabled": bool(repo.get("enabled")),
                    "local_head_sha": repo.get("local_head_sha"),
                    "remote_head_sha": repo.get("remote_head_sha"),
                    "update_available": bool(repo.get("update_available")),
                }
            )
        return sorted(facts, key=lambda fact: str(fact.get("repo_ref", "")).casefold())

    def _revision_cube_facts(self, *, include_disabled: bool) -> list[dict[str, Any]]:
        """Return normalized cube facts used to compute catalog revisions."""

        facts: list[dict[str, Any]] = []
        repo_cache: dict[tuple[str, str], TrackedRepo] = {}
        for summary in self._list_catalog_cube_summaries(
            include_disabled=include_disabled,
            include_internal_payload=True,
        ):
            cube_id = normalize_metadata_string(summary.get("cube_id"))
            _, _, content_hash = self._summary_payload_with_hash(summary)
            source = self._source_metadata_for_summary(summary, repo_cache=repo_cache)
            facts.append(
                {
                    "cube_id": cube_id,
                    "version": normalize_metadata_string(summary.get("version")),
                    "content_hash": content_hash,
                    "source": source,
                }
            )
        return sorted(facts, key=lambda fact: str(fact.get("cube_id", "")).casefold())


    def _local_source_relative_path(self, cube_id: str) -> str:
        """Return the source-relative path for a local canonical cube id."""

        try:
            parsed = parse_canonical_cube_id(cube_id)
        except CubeIdentityError:
            return ""
        return parsed.path if parsed.source_kind == "local" else ""

    def _local_head_sha(self, tracked: TrackedRepo) -> str:
        """Return persisted or live HEAD SHA for a tracked checkout."""

        if tracked.local_head_sha:
            return tracked.local_head_sha
        checkout = Path(tracked.local_checkout_path).resolve()
        return self._repo_head_sha(checkout)

    def _local_source_metadata(
        self,
        *,
        namespace: str,
        source_path: str,
        repo_root: Path,
        repo_relative_path: str,
    ) -> dict[str, Any]:
        """Report one local cube against its shared repository state."""

        return {
            "kind": "local",
            "namespace": namespace,
            "path": source_path,
            "localHeadSha": self._repo_head_sha(repo_root),
            "remoteHeadSha": "",
            "dirty": self._is_repo_path_dirty(repo_root, repo_relative_path),
        }

    def _repo_head_sha(self, checkout: Path) -> str:
        """Return HEAD for one initialized local repository when available."""

        if not (checkout / ".git").exists():
            return ""
        try:
            result = self.tracked_repo_service.git_runner(
                ["rev-parse", "HEAD"], cwd=checkout
            )
        except (OSError, RuntimeError):
            return ""
        return normalize_metadata_string(getattr(result, "stdout", ""))

    def _is_repo_path_dirty(self, checkout: Path, relative_path: str) -> bool:
        """Return whether a repo-relative cube artifact differs from clean HEAD."""

        if not relative_path or not (checkout / ".git").exists():
            return False
        return relative_path.replace("\\", "/") in self._repo_dirty_paths(checkout)

    def _repo_dirty_paths(self, checkout: Path) -> frozenset[str]:
        """Return dirty repo paths from one cached git status scan."""

        started_at = perf_counter()
        checkout = checkout.resolve()
        cached = self._repo_dirty_paths_cache.get(checkout)
        if cached is not None:
            _log_cube_library_diagnostic(
                "sugarcubes_repo_dirty_paths_timing",
                total_duration_ms=round((perf_counter() - started_at) * 1000, 3),
                checkout=checkout.name,
                cached=True,
                dirty_path_count=len(cached),
            )
            return cached
        try:
            result = self.tracked_repo_service.git_runner(
                ["status", "--porcelain"],
                cwd=checkout,
            )
        except (OSError, RuntimeError):
            dirty_paths: frozenset[str] = frozenset()
        else:
            dirty_paths = frozenset(
                _git_status_path(line)
                for line in str(getattr(result, "stdout", "")).splitlines()
                if _git_status_path(line)
            )
        self._repo_dirty_paths_cache[checkout] = dirty_paths
        _log_cube_library_diagnostic(
            "sugarcubes_repo_dirty_paths_timing",
            total_duration_ms=round((perf_counter() - started_at) * 1000, 3),
            checkout=checkout.name,
            cached=False,
            dirty_path_count=len(dirty_paths),
        )
        return dirty_paths

    def _count_cube_files(self, root: Path) -> int:
        """Return the number of loadable cube files under one checkout path."""

        if not root.exists() or not root.is_dir():
            return 0
        return len([path for path in list_cube_files(root) if ".git" not in path.parts])

    def _tracked_repo_from_payload(self, repo_entry: Mapping[str, Any]) -> TrackedRepo:
        """Convert serialized manifest state back into a tracked repo record."""

        return TrackedRepo(
            owner=str(repo_entry["owner"]),
            repo=str(repo_entry["repo"]),
            branch=str(repo_entry["branch"]),
            enabled=bool(repo_entry["enabled"]),
            default_base_repo=bool(repo_entry["default_base_repo"]),
            auto_update=bool(repo_entry.get("auto_update", False)),
            local_checkout_path=str(repo_entry["local_checkout_path"]),
            last_sync_at=str(repo_entry["last_sync_at"]),
            last_sync_status=str(repo_entry["last_sync_status"]),
            last_sync_error=str(repo_entry["last_sync_error"]),
            last_checked_at=str(repo_entry.get("last_checked_at") or ""),
            last_check_status=str(repo_entry.get("last_check_status") or "never"),
            last_check_error=str(repo_entry.get("last_check_error") or ""),
            remote_head_sha=str(repo_entry.get("remote_head_sha") or ""),
            local_head_sha=str(repo_entry.get("local_head_sha") or ""),
            update_available=bool(repo_entry.get("update_available", False)),
        )

    def resolve_source_base_dir(self, parsed_cube_id: CanonicalCubeId) -> Path:
        """Resolve the managed base directory for one parsed canonical cube id."""

        if parsed_cube_id.source_kind == "github":
            tracked = self.tracked_repo_service.get_repo(
                parsed_cube_id.owner, parsed_cube_id.repo
            )
            return Path(
                tracked.local_checkout_path
                or self.tracked_repo_service.checkout_path(
                    parsed_cube_id.owner, parsed_cube_id.repo
                )
            ).resolve()
        local_root = self.local_workspace_root().resolve()
        namespace_root = (local_root / parsed_cube_id.namespace).resolve()
        try:
            namespace_root.relative_to(local_root)
        except ValueError as exc:
            raise BackendError(
                "Cube id path must stay within the managed source", status=400
            ) from exc
        return namespace_root

    def resolve_source_descriptor_by_path(self, cube_path: Path) -> dict[str, str]:
        """Resolve source ownership metadata for one local cube path."""

        resolved_path = cube_path.resolve()
        for repo_entry in self.tracked_repo_service.list_repos()["repos"]:
            checkout_path = Path(repo_entry["local_checkout_path"]).resolve()
            try:
                resolved_path.relative_to(checkout_path)
            except ValueError:
                continue
            return {
                "source_kind": "github",
                "base_dir": str(checkout_path),
                "owner": repo_entry["owner"],
                "repo": repo_entry["repo"],
                "repo_ref": f"{repo_entry['owner']}/{repo_entry['repo']}",
                "namespace": "",
            }

        local_root = self.local_workspace_root().resolve()
        try:
            relative_path = resolved_path.relative_to(local_root)
        except ValueError as exc:
            raise BackendError(
                "Cube path is not owned by a managed source", status=404
            ) from exc
        if not relative_path.parts:
            raise BackendError("Cube path is not owned by a managed source", status=404)
        namespace = relative_path.parts[0]
        return {
            "source_kind": "local",
            "base_dir": str((local_root / namespace).resolve()),
            "owner": "",
            "repo": "",
            "repo_ref": "",
            "namespace": namespace,
        }

    def _resolve_tracked_repo_for_path(self, cube_path: Path) -> TrackedRepo:
        """Resolve which tracked repo owns one local cube path."""

        resolved_path = cube_path.resolve()
        for repo_entry in self.tracked_repo_service.list_repos()["repos"]:
            checkout_path = Path(repo_entry["local_checkout_path"]).resolve()
            try:
                resolved_path.relative_to(checkout_path)
            except ValueError:
                continue
            return TrackedRepo(
                owner=repo_entry["owner"],
                repo=repo_entry["repo"],
                branch=repo_entry["branch"],
                enabled=bool(repo_entry["enabled"]),
                default_base_repo=bool(repo_entry["default_base_repo"]),
                local_checkout_path=repo_entry["local_checkout_path"],
                last_sync_at=repo_entry["last_sync_at"],
                last_sync_status=repo_entry["last_sync_status"],
                last_sync_error=repo_entry["last_sync_error"],
            )
        raise BackendError("Cube path is not owned by a tracked repo", status=404)

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
"""Coordinate source-aware Cube library collaborators behind one stable API."""

from __future__ import annotations

from collections.abc import Callable, Collection, Mapping, Sequence
from pathlib import Path
from typing import Any, Optional

from ...cube_model import CanonicalCubeId
from .cube_library_artifact_service import CubeLibraryArtifactService
from .cube_library_catalog_projection import CubeLibraryCatalogProjection
from .cube_library_listing import CubeLibraryListing
from .cube_library_mutation_service import CubeLibraryMutationService
from .cube_library_pack_service import CubeLibraryPackService
from .cube_library_preview_service import CubeLibraryPreviewService
from .cube_library_readiness_service import CubeLibraryReadinessService
from .cube_library_source_resolver import CubeLibrarySourceResolver
from .cube_library_state_service import CubeLibraryStateService
from .ownership_policy_service import OwnershipPolicyService
from .tracked_repo_service import TrackedRepoService


class CubeLibraryService:
    """Coordinate cohesive Cube library owners behind the host-facing API."""

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
        """Compose Cube library owners around stable host dependencies."""

        self.extension_root = extension_root.resolve()
        self.load_cube_artifact = load_cube_artifact
        self.prepare_cube_import = prepare_cube_import
        self.tracked_repo_service = tracked_repo_service
        self.ownership_policy_service = ownership_policy_service
        self.registry_factory = registry_factory
        self.sources = CubeLibrarySourceResolver(
            self.extension_root,
            tracked_repo_service=tracked_repo_service,
        )
        self.catalog_listing = CubeLibraryListing(
            self.extension_root,
            tracked_repo_service=tracked_repo_service,
            ownership_policy_service=ownership_policy_service,
            sources=self.sources,
        )
        self.catalog_projection = CubeLibraryCatalogProjection(
            tracked_repo_service=tracked_repo_service,
            sources=self.sources,
        )
        self._state = CubeLibraryStateService(
            listing=self.catalog_listing,
            projection=self.catalog_projection,
            sources=self.sources,
        )
        self.artifacts = CubeLibraryArtifactService(self)
        self._packs = CubeLibraryPackService(
            tracked_repo_service=tracked_repo_service,
            projection=self.catalog_projection,
            invalidate_catalog=self.invalidate_catalog_state,
            catalog_revision=self.catalog_revision,
        )
        self._readiness = CubeLibraryReadinessService(self)
        self._previews = CubeLibraryPreviewService(
            self.extension_root,
            load_cube_artifact=load_cube_artifact,
            prepare_cube_import=prepare_cube_import,
            sources=self.sources,
        )
        self._mutations = CubeLibraryMutationService(
            self.extension_root,
            load_cube_artifact=load_cube_artifact,
            ownership_policy_service=ownership_policy_service,
            sources=self.sources,
            summarize_cube=self.summarize_cube,
            invalidate_catalog=self.invalidate_catalog_state,
        )

    def subscribe_library_changed(
        self,
        listener: Callable[[dict[str, Any]], None],
    ) -> Callable[[], None]:
        """Register a generic library-change listener and return an unsubscribe."""

        return self._state.subscribe(listener)

    def notify_library_changed(
        self,
        *,
        affected_cube_ids: Sequence[str],
        saved_versions: Mapping[str, str],
        reason: str,
    ) -> None:
        """Publish a generic library-change event to in-process consumers."""

        self._state.notify(
            affected_cube_ids=affected_cube_ids,
            saved_versions=saved_versions,
            reason=reason,
        )

    def invalidate_catalog_state(
        self,
        *,
        reason: str,
        affected_cube_ids: Sequence[str] = (),
    ) -> None:
        """Invalidate cached catalog state after a library-visible mutation."""

        self._state.invalidate(reason=reason, affected_cube_ids=affected_cube_ids)

    def repo_workspace_root(self) -> Path:
        """Return the managed tracked-repo workspace root."""

        return self.sources.repo_workspace_root()

    def local_workspace_root(self) -> Path:
        """Return the managed local source workspace root."""

        return self.sources.local_workspace_root()

    def list_cubes(self) -> dict[str, Any]:
        """Return the source-aware cube library response payload."""

        return self.catalog_listing.list_cubes()

    def library_status(self) -> dict[str, Any]:
        """Return target-owned Cube Library availability for backend adapters."""

        return self._state.library_status()

    def library_capabilities_status(self) -> dict[str, Any]:
        """Return Cube Library capability facts without building catalog state."""

        return self._state.capabilities_status()

    def catalog_revision(self, *, include_disabled: bool = False) -> str:
        """Return a deterministic revision for catalog-relevant library state."""

        return self._state.catalog_revision(include_disabled=include_disabled)

    def list_library_catalog(self, *, include_disabled: bool = False) -> dict[str, Any]:
        """Return backend-facing catalog metadata for enabled library cubes."""

        return self._state.list_catalog(include_disabled=include_disabled)

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
        self, *, cube_id: str, version: str
    ) -> dict[str, Any]:
        """Load the newest artifact for a cube id and version."""

        return self.artifacts.load_library_cube_version(
            cube_id=cube_id, version=version
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

        return self._packs.list_library_packs()

    def preflight_library_pack(
        self, *, owner: str, repo: str, branch: str
    ) -> dict[str, Any]:
        """Return preflight information for a candidate Cube Pack."""

        return self._packs.preflight_library_pack(owner=owner, repo=repo, branch=branch)

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

        return self._packs.add_library_pack(
            owner=owner,
            repo=repo,
            branch=branch,
            enabled=enabled,
            auto_update=auto_update,
            sync_immediately=sync_immediately,
        )

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

        return self._packs.update_library_pack(
            owner=owner,
            repo=repo,
            branch=branch,
            enabled=enabled,
            auto_update=auto_update,
        )

    def remove_library_pack(self, *, owner: str, repo: str) -> dict[str, Any]:
        """Remove a tracked Cube Pack through SugarCubes policy enforcement."""

        return self._packs.remove_library_pack(owner=owner, repo=repo)

    def sync_library_pack(self, *, owner: str, repo: str) -> dict[str, Any]:
        """Synchronously sync one tracked Cube Pack."""

        return self._packs.sync_library_pack(owner=owner, repo=repo)

    def sync_all_library_packs(self) -> dict[str, Any]:
        """Synchronously sync all enabled Cube Packs and return per-pack results."""

        return self._packs.sync_all_library_packs()

    def library_readiness(self, custom_nodes_root: Path) -> dict[str, Any]:
        """Return target dependency readiness and install plan for enabled cubes."""

        return self._readiness.library_readiness(custom_nodes_root)

    def summarize_cube(self, cube_path: Path) -> dict[str, Any]:
        """Summarize a single cube file for browser payloads."""

        return self.catalog_listing.summarize_cube(cube_path)

    def resolve_cube_by_id(self, cube_id: str) -> Path:
        """Resolve a source-owned cube path by canonical cube id."""

        return self.sources.resolve_cube_by_id(cube_id)

    def resolve_cube_target_path(self, cube_id: str) -> Path:
        """Resolve the managed destination path for one canonical cube id."""

        return self.sources.resolve_cube_target_path(cube_id)

    def resolve_source_base_dir(self, parsed_cube_id: CanonicalCubeId) -> Path:
        """Resolve the managed base directory for one parsed canonical cube id."""

        return self.sources.resolve_source_base_dir(parsed_cube_id)

    def resolve_source_descriptor_by_path(self, cube_path: Path) -> dict[str, str]:
        """Resolve source ownership metadata for one local cube path."""

        return self.sources.resolve_source_descriptor_by_path(cube_path)

    def build_default_alias_lookup(self, cube_ids: Collection[str]) -> dict[str, str]:
        """Build a cube-id to display-name lookup for export flows."""

        return self._previews.build_default_alias_lookup(cube_ids)

    def preview_cube(self, cube_id: str) -> dict[str, Any]:
        """Return the lightweight preview payload used by the cube browser."""

        return self._previews.preview_cube(cube_id)

    def resolve_cube_icon_asset(self, cube_id: str) -> tuple[Path, str]:
        """Return the resolved icon file and media type for one cube."""

        return self._previews.resolve_cube_icon_asset(cube_id)

    def import_cube_file(
        self,
        *,
        source_value: str,
        target_cube_id: str,
        overwrite: bool,
    ) -> dict[str, Any]:
        """Copy one external `.cube` file into a managed source location."""

        return self._mutations.import_cube_file(
            source_value=source_value,
            target_cube_id=target_cube_id,
            overwrite=overwrite,
        )

    def delete_cube(self, *, cube_id: str) -> dict[str, Any]:
        """Delete a tracked cube by canonical id."""

        return self._mutations.delete_cube(cube_id=cube_id)

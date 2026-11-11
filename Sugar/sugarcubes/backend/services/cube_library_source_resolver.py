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
"""Resolve Cube source ownership, managed paths, and repository state."""

from __future__ import annotations

from pathlib import Path
from time import perf_counter
from typing import Any, Mapping

from ...cube_model import CubeIdentityError, CanonicalCubeId, parse_canonical_cube_id
from ..responses import BackendError
from .cube_file_io import format_display_path
from .cube_library_diagnostics import log_cube_library_diagnostic
from .cube_metadata import normalize_metadata_string
from .tracked_repo_models import TrackedRepo
from .tracked_repo_service import TrackedRepoService


class CubeLibrarySourceResolver:
    """Own managed source paths and their live repository metadata."""

    def __init__(
        self, extension_root: Path, tracked_repo_service: TrackedRepoService
    ) -> None:
        """Initialize source resolution for one managed repository workspace."""

        self.extension_root = extension_root.resolve()
        self.tracked_repo_service = tracked_repo_service
        self._repo_dirty_paths_cache: dict[Path, frozenset[str]] = {}

    def invalidate(self) -> None:
        """Discard cached Git status facts after a library-visible mutation."""

        self._repo_dirty_paths_cache.clear()

    def repo_workspace_root(self) -> Path:
        """Return the managed tracked-repo workspace root."""

        return self.tracked_repo_service.workspace_root()

    def local_workspace_root(self) -> Path:
        """Return the managed local source workspace root."""

        return self.tracked_repo_service.ensure_local_repo()

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
        log_cube_library_diagnostic(
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

    def source_metadata_for_summary(
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
                "localHeadSha": self.local_head_sha(tracked),
                "remoteHeadSha": tracked.remote_head_sha,
                "dirty": self.is_repo_path_dirty(base_dir, relative_path),
            }
        namespace = normalize_metadata_string(
            source.get("namespace") or summary.get("namespace")
        )
        source_path = self._local_source_relative_path(cube_id)
        return self.local_source_metadata(
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

    def _local_source_relative_path(self, cube_id: str) -> str:
        """Return the source-relative path for a local canonical cube id."""

        try:
            parsed = parse_canonical_cube_id(cube_id)
        except CubeIdentityError:
            return ""
        return parsed.path if parsed.source_kind == "local" else ""

    def local_head_sha(self, tracked: TrackedRepo) -> str:
        """Return persisted or live HEAD SHA for a tracked checkout."""

        if tracked.local_head_sha:
            return tracked.local_head_sha
        checkout = Path(tracked.local_checkout_path).resolve()
        return self._repo_head_sha(checkout)

    def local_source_metadata(
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
            "dirty": self.is_repo_path_dirty(repo_root, repo_relative_path),
        }

    def _repo_head_sha(self, checkout: Path) -> str:
        """Return HEAD for one initialized local repository when available."""

        if not (checkout / ".git").exists():
            return ""
        try:
            return self.tracked_repo_service.head_commit_id(repo_root=checkout)
        except (OSError, RuntimeError):
            return ""

    def is_repo_path_dirty(self, checkout: Path, relative_path: str) -> bool:
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
            log_cube_library_diagnostic(
                "sugarcubes_repo_dirty_paths_timing",
                total_duration_ms=round((perf_counter() - started_at) * 1000, 3),
                checkout=checkout.name,
                cached=True,
                dirty_path_count=len(cached),
            )
            return cached
        try:
            dirty_paths = frozenset(
                self.tracked_repo_service.changed_paths(repo_root=checkout)
            )
        except (OSError, RuntimeError):
            dirty_paths = frozenset()
        self._repo_dirty_paths_cache[checkout] = dirty_paths
        log_cube_library_diagnostic(
            "sugarcubes_repo_dirty_paths_timing",
            total_duration_ms=round((perf_counter() - started_at) * 1000, 3),
            checkout=checkout.name,
            cached=False,
            dirty_path_count=len(dirty_paths),
        )
        return dirty_paths

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

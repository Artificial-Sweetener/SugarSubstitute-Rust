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
"""List and summarize Cube artifacts across managed sources."""

from __future__ import annotations

import logging
from pathlib import Path
from time import perf_counter
from typing import Any, Mapping

from ...cube_model import RESERVED_SOURCE_NAMES
from ...instrumentation import log_event
from ..responses import BackendError
from .cube_file_io import format_display_path, list_cube_files
from .cube_library_diagnostics import log_cube_library_diagnostic
from .cube_library_source_resolver import CubeLibrarySourceResolver
from .cube_summary import summarize_cube_file
from .ownership_policy_service import OwnershipPolicyService
from .tracked_repo_models import TrackedRepo
from .tracked_repo_service import TrackedRepoService

_logger = logging.getLogger(__name__)


class CubeLibraryListing:
    """Own source-aware Cube discovery and summary construction."""

    def __init__(
        self,
        extension_root: Path,
        *,
        tracked_repo_service: TrackedRepoService,
        ownership_policy_service: OwnershipPolicyService,
        sources: CubeLibrarySourceResolver,
    ) -> None:
        """Initialize listing against authoritative source collaborators."""

        self.extension_root = extension_root.resolve()
        self.tracked_repo_service = tracked_repo_service
        self.ownership_policy_service = ownership_policy_service
        self.sources = sources

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
                    self.sources.repo_workspace_root(), self.extension_root
                ),
            },
        )
        return {
            "cubes": cubes,
            "directory": format_display_path(
                self.sources.repo_workspace_root(), self.extension_root
            ),
            "exists": self.sources.repo_workspace_root().exists()
            or self.sources.local_workspace_root().exists(),
            "count": len(cubes),
        }

    def summarize_cube(self, cube_path: Path) -> dict[str, Any]:
        """Summarize a single cube file for browser payloads."""

        source = self.sources.resolve_source_descriptor_by_path(cube_path)
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
        log_cube_library_diagnostic(
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

        workspace_root = self.sources.local_workspace_root().resolve()
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

    def list_catalog_cube_summaries(
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
            tracked = self.tracked_repo_from_payload(repo_entry)
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
        log_cube_library_diagnostic(
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

    def tracked_repo_from_payload(self, repo_entry: Mapping[str, Any]) -> TrackedRepo:
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

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
"""Adapt tracked repositories to approved workflow source synchronization."""

from __future__ import annotations

from collections.abc import Callable

from ...cube_model import parse_canonical_cube_id
from ...library import CatalogCubeArtifact
from ..responses import BackendError
from .tracked_repo_models import DEFAULT_BRANCH, TrackedRepo
from .tracked_repo_service import TrackedRepoService


class WorkflowSourceSyncPort:
    """Own tracked-pack preflight, enrollment, sync, and source resolution."""

    def __init__(
        self,
        tracked_repos: TrackedRepoService,
        *,
        catalog_artifacts: Callable[[], tuple[CatalogCubeArtifact, ...]],
    ) -> None:
        """Bind existing repository and catalog authorities."""

        self._tracked_repos = tracked_repos
        self._catalog_artifacts = catalog_artifacts

    def preflight(self, repo_ref: str) -> None:
        """Validate one existing or prospective Cube pack without mutation."""

        owner, repo = _repo_parts(repo_ref)
        tracked = self._find_tracked(owner, repo)
        branch = tracked.branch if tracked is not None else DEFAULT_BRANCH
        self._tracked_repos.require_repo_contains_cubes(
            owner=owner,
            repo=repo,
            branch=branch,
        )

    def sync(self, repo_ref: str) -> None:
        """Enroll a preflighted source when needed and synchronize it."""

        owner, repo = _repo_parts(repo_ref)
        tracked = self._find_tracked(owner, repo)
        if tracked is None:
            self._tracked_repos.add_repo(
                owner=owner,
                repo=repo,
                branch=DEFAULT_BRANCH,
                enabled=True,
                default_base_repo=False,
            )
        elif not tracked.enabled:
            self._tracked_repos.update_repo(owner=owner, repo=repo, enabled=True)
        self._tracked_repos.sync_repo(owner=owner, repo=repo)

    def resolve_semantic_hash(
        self, *, repo_ref: str, cube_id: str, cube_version: str
    ) -> str | None:
        """Resolve one synchronized identity from its claimed repository only."""

        prefix = f"github:{repo_ref}:"
        hashes = {
            artifact.semantic_hash
            for artifact in self._catalog_artifacts()
            if artifact.cube_id == cube_id
            and artifact.cube_version == cube_version
            and artifact.source_ref.startswith(prefix)
        }
        if len(hashes) > 1:
            raise ValueError(
                f"Synchronized source '{repo_ref}' contains conflicting Cube definitions"
            )
        return next(iter(hashes), None)

    def _find_tracked(self, owner: str, repo: str) -> TrackedRepo | None:
        """Return existing tracked state while preserving other backend errors."""

        try:
            return self._tracked_repos.get_repo(owner, repo)
        except BackendError as exc:
            if exc.status == 404:
                return None
            raise


def _repo_parts(repo_ref: str) -> tuple[str, str]:
    """Validate one owner/repository reference through canonical identity rules."""

    parsed = parse_canonical_cube_id(f"{repo_ref}/source.cube")
    if parsed.source_kind != "github":
        raise ValueError("Workflow Cube source must be a GitHub repository")
    return parsed.owner, parsed.repo

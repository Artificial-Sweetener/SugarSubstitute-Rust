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
"""Commit cohesive cube mutations through one Git-history boundary."""

from __future__ import annotations

from pathlib import Path
from typing import Sequence

from ..responses import BackendError
from .cube_git_context import CubeGitContext
from .tracked_repo_service import CubeCommitResult, TrackedRepoService


class CubeHistoryService:
    """Own Git commits for saved, renamed, promoted, and retired cube artifacts."""

    def __init__(self, tracked_repo_service: TrackedRepoService) -> None:
        """Initialize history persistence with the managed Git repository owner."""

        self.tracked_repo_service = tracked_repo_service

    def commit_context(
        self,
        context: CubeGitContext,
        *,
        message: str,
        additional_paths: Sequence[str] = (),
    ) -> CubeCommitResult:
        """Commit the context artifact and any related paths as one mutation."""

        return self.tracked_repo_service.commit_paths(
            repo_root=context.repo_root,
            repo_relative_paths=[context.repo_relative_path, *additional_paths],
            commit_message=message,
        )

    def commit_paths(
        self,
        *,
        repo_root: Path,
        relative_paths: Sequence[str],
        message: str,
    ) -> CubeCommitResult:
        """Commit a validated cohesive path set within one managed repository."""

        return self.tracked_repo_service.commit_paths(
            repo_root=repo_root,
            repo_relative_paths=relative_paths,
            commit_message=message,
        )

    def latest_path_commit(self, context: CubeGitContext) -> str:
        """Return the newest commit containing one artifact path."""

        try:
            result = self.tracked_repo_service.git_runner(
                ["log", "-1", "--format=%H", "--", context.repo_relative_path],
                cwd=context.repo_root,
            )
        except RuntimeError as exc:
            raise BackendError(
                "Failed to inspect cube promotion history", status=500
            ) from exc
        return (result.stdout or "").strip()

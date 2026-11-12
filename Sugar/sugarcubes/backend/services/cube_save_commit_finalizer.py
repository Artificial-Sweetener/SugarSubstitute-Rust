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
"""Classify Cube changes and finalize isolated repository commits."""

from __future__ import annotations

import logging
from copy import deepcopy
from pathlib import Path
from typing import Any, Mapping, Optional

from ..responses import BackendError
from .cube_git_context import CubeGitContext, resolve_cube_git_context
from .cube_library_service import CubeLibraryService
from .cube_metadata import normalize_metadata_string
from .cube_save_models import CubeSaveCommitState, CubeSaveTarget

_logger = logging.getLogger(__name__)


class CubeSaveCommitFinalizer:
    """Own Cube change classification and Git commit finalization."""

    def __init__(self, library_service: CubeLibraryService) -> None:
        """Initialize commit policy against tracked repository services."""

        self.library_service = library_service

    def build_commit_state(
        self,
        *,
        save_target: CubeSaveTarget,
        previous_payload: Optional[Mapping[str, Any]],
    ) -> CubeSaveCommitState:
        """Build commit eligibility metadata from the previous and next payloads."""

        previous_version = normalize_metadata_string(
            previous_payload.get("version") if previous_payload else ""
        )
        resulting_version = normalize_metadata_string(
            save_target.exported.cube.get("version")
        )
        version_changed = (
            bool(resulting_version) and previous_version != resulting_version
        )
        change_scope = self._classify_saved_change_scope(
            previous_payload=previous_payload,
            next_payload=save_target.exported.cube,
            version_changed=version_changed,
        )
        return CubeSaveCommitState(
            should_commit=False,
            action_kind=save_target.action_kind,
            previous_version=previous_version,
            resulting_version=resulting_version,
            version_changed=version_changed,
            change_scope=change_scope,
            previous_cube_id=save_target.previous_cube_id,
        )

    def _classify_saved_change_scope(
        self,
        *,
        previous_payload: Optional[Mapping[str, Any]],
        next_payload: Mapping[str, Any],
        version_changed: bool,
    ) -> str:
        """Return a commit-message scope for one saved cube change."""

        if version_changed:
            return "version"
        if previous_payload is None:
            return "content"
        previous_without_layout = self._without_layout_fields(previous_payload)
        next_without_layout = self._without_layout_fields(next_payload)
        if previous_without_layout == next_without_layout:
            return "layout"
        return "content"

    def _without_layout_fields(self, payload: Mapping[str, Any]) -> dict[str, Any]:
        """Return a payload copy with persisted layout-only fields removed."""

        stripped = deepcopy(dict(payload))
        stripped.pop("layout", None)
        implementation = stripped.get("implementation")
        if isinstance(implementation, dict):
            implementation.pop("layout", None)
        metadata = stripped.get("metadata")
        if isinstance(metadata, dict):
            metadata.pop("layout", None)
        return stripped

    def finalize_commit(
        self,
        *,
        save_target: CubeSaveTarget,
        commit_state: CubeSaveCommitState,
    ) -> CubeSaveCommitState:
        """Commit one saved cube file when the persisted content changed."""

        try:
            git_context = resolve_cube_git_context(
                self.library_service.tracked_repo_service,
                save_target.cube_id,
            )
            if not self.library_service.tracked_repo_service.has_file_changes(
                repo_root=git_context.repo_root,
                repo_relative_path=git_context.repo_relative_path,
            ):
                return CubeSaveCommitState(
                    should_commit=False,
                    action_kind=commit_state.action_kind,
                    previous_version=commit_state.previous_version,
                    resulting_version=commit_state.resulting_version,
                    version_changed=commit_state.version_changed,
                    change_scope="none",
                    previous_cube_id=commit_state.previous_cube_id,
                )
            commit_result = self.library_service.tracked_repo_service.commit_file(
                repo_root=git_context.repo_root,
                repo_relative_path=git_context.repo_relative_path,
                commit_message=self._build_commit_message(
                    save_target=save_target,
                    commit_state=commit_state,
                    git_context=git_context,
                ),
            )
            return CubeSaveCommitState(
                should_commit=True,
                action_kind=commit_state.action_kind,
                previous_version=commit_state.previous_version,
                resulting_version=commit_state.resulting_version,
                version_changed=commit_state.version_changed,
                change_scope=commit_state.change_scope,
                previous_cube_id=commit_state.previous_cube_id,
                commit_result=commit_result,
            )
        except BackendError as exc:
            _logger.warning(
                "SugarCubes: save wrote cube '%s' but git commit failed",
                save_target.cube_id,
                exc_info=exc,
            )
            return CubeSaveCommitState(
                should_commit=True,
                action_kind=commit_state.action_kind,
                previous_version=commit_state.previous_version,
                resulting_version=commit_state.resulting_version,
                version_changed=commit_state.version_changed,
                change_scope=commit_state.change_scope,
                previous_cube_id=commit_state.previous_cube_id,
                commit_error=exc.message,
            )

    def _build_commit_message(
        self,
        *,
        save_target: CubeSaveTarget,
        commit_state: CubeSaveCommitState,
        git_context: CubeGitContext,
    ) -> str:
        """Build the concise commit subject for one cube save."""

        target_name = Path(git_context.repo_relative_path).name
        version = commit_state.resulting_version
        version_suffix = (
            f" v{version}" if commit_state.version_changed and version else ""
        )
        scope_suffix = (
            f" {commit_state.change_scope}"
            if not version_suffix and commit_state.change_scope in {"layout", "content"}
            else ""
        )
        if commit_state.action_kind == "rename" and commit_state.previous_cube_id:
            previous_name = Path(
                commit_state.previous_cube_id.split("/", maxsplit=3)[-1]
            ).name
            return (
                f"rename {previous_name} to {target_name}{version_suffix}{scope_suffix}"
            )
        if commit_state.action_kind == "fork":
            previous_name = (
                Path(commit_state.previous_cube_id.split("/", maxsplit=3)[-1]).name
                if commit_state.previous_cube_id
                else target_name
            )
            return (
                f"fork {previous_name} as {save_target.cube_id}"
                f"{version_suffix}{scope_suffix}"
            )
        return f"{commit_state.action_kind} {target_name}{version_suffix}{scope_suffix}"

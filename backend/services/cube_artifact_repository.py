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
"""Resolve and mutate managed cube artifacts independently from catalog reads."""

from __future__ import annotations

import json
import logging
import os
from typing import Any, Mapping
from uuid import uuid4

try:
    from ...cube_model import CubeIdentityError, parse_canonical_cube_id
    from ..responses import BackendError
    from .cube_git_context import CubeGitContext, resolve_cube_git_context
    from .cube_library_service import read_cube_payload
    from .tracked_repo_service import TrackedRepoService
except ImportError:
    from cube_model import CubeIdentityError, parse_canonical_cube_id
    from backend.responses import BackendError
    from backend.services.cube_git_context import (
        CubeGitContext,
        resolve_cube_git_context,
    )
    from backend.services.cube_library_service import read_cube_payload
    from backend.services.tracked_repo_service import TrackedRepoService

_logger = logging.getLogger(__name__)


class CubeArtifactRepository:
    """Own safe path resolution and atomic persistence for managed cube files."""

    def __init__(self, tracked_repo_service: TrackedRepoService) -> None:
        """Initialize the artifact repository with the managed source owner."""

        self.tracked_repo_service = tracked_repo_service

    def context(self, cube_id: str) -> CubeGitContext:
        """Return the validated managed source context for one canonical cube id."""

        return resolve_cube_git_context(self.tracked_repo_service, cube_id)

    def read(self, cube_id: str) -> tuple[CubeGitContext, dict[str, Any]]:
        """Read one existing cube or raise an actionable backend error."""

        context = self.context(cube_id)
        if not context.cube_path.is_file():
            raise BackendError(f"Cube '{cube_id}' not found", status=404)
        payload, error = read_cube_payload(context.cube_path)
        if error or not payload:
            raise BackendError(error or "Invalid cube payload", status=400)
        return context, dict(payload)

    def assert_available(self, cube_id: str) -> CubeGitContext:
        """Return a target context only when no artifact already occupies it."""

        context = self.context(cube_id)
        if context.cube_path.exists():
            raise BackendError(f"Cube '{cube_id}' already exists", status=409)
        return context

    def write(self, context: CubeGitContext, payload: Mapping[str, Any]) -> None:
        """Atomically write one cube payload inside its validated managed source."""

        target_path = context.cube_path
        target_path.parent.mkdir(parents=True, exist_ok=True)
        temp_path = target_path.with_name(f"{target_path.name}.{uuid4().hex}.tmp")
        try:
            with temp_path.open("w", encoding="utf-8") as handle:
                json.dump(dict(payload), handle, indent=2)
                handle.write("\n")
            os.replace(temp_path, target_path)
        except (OSError, TypeError, ValueError) as exc:
            _logger.exception(
                "SugarCubes: failed to persist managed cube artifact '%s'",
                context.cube_id,
            )
            raise BackendError("Failed to persist cube artifact", status=500) from exc
        finally:
            try:
                if temp_path.exists():
                    temp_path.unlink()
            except OSError:
                _logger.warning(
                    "SugarCubes: failed to remove cube artifact temp file",
                    exc_info=True,
                )

    def delete(self, context: CubeGitContext) -> None:
        """Delete one existing managed cube artifact without hiding IO failures."""

        try:
            context.cube_path.unlink()
        except FileNotFoundError:
            return
        except OSError as exc:
            _logger.exception(
                "SugarCubes: failed to remove managed cube artifact '%s'",
                context.cube_id,
            )
            raise BackendError("Failed to remove cube artifact", status=500) from exc

    def restore(self, context: CubeGitContext, payload: Mapping[str, Any]) -> None:
        """Restore a previously captured artifact during recoverable mutation cleanup."""

        self.write(context, payload)

    def parse(self, cube_id: str) -> Any:
        """Parse one canonical id and translate identity failures to HTTP errors."""

        try:
            return parse_canonical_cube_id(cube_id)
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc

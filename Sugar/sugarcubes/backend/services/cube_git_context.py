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
"""Shared git-repo context resolution for canonical SugarCube ids."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from ...cube_model import CubeIdentityError, parse_canonical_cube_id
from ..responses import BackendError
from .tracked_repo_service import TrackedRepoService


@dataclass(frozen=True)
class CubeGitContext:
    """Describe the owning git repo and repo-relative file path for one cube."""

    cube_id: str
    cube_path: Path
    repo_root: Path
    repo_relative_path: str
    source_kind: str
    owner: str = ""
    repo: str = ""
    namespace: str = ""


def resolve_cube_git_context(
    tracked_repo_service: TrackedRepoService,
    cube_id: str,
) -> CubeGitContext:
    """Resolve git ownership for one canonical cube id.

    Args:
        tracked_repo_service: Authoritative tracked-repo owner used by the backend.
        cube_id: Canonical `<owner>/<repo>/...` or `local/...` cube id.

    Returns:
        Shared git context used by revision, save, and commit flows.

    Raises:
        BackendError: The cube id is invalid or points outside the managed source.
    """

    try:
        parsed = parse_canonical_cube_id(cube_id)
    except CubeIdentityError as exc:
        raise BackendError(str(exc), status=400) from exc

    if parsed.source_kind == "github":
        tracked = tracked_repo_service.get_repo(parsed.owner, parsed.repo)
        repo_root = Path(
            tracked.local_checkout_path
            or tracked_repo_service.checkout_path(parsed.owner, parsed.repo)
        ).resolve()
        cube_path = (repo_root / Path(parsed.path)).resolve()
        _assert_within_repo(cube_path, repo_root)
        return CubeGitContext(
            cube_id=cube_id,
            cube_path=cube_path,
            repo_root=repo_root,
            repo_relative_path=parsed.path,
            source_kind="github",
            owner=parsed.owner,
            repo=parsed.repo,
        )

    repo_root = tracked_repo_service.ensure_local_repo().resolve()
    repo_relative_path = f"{parsed.namespace}/{parsed.path}"
    cube_path = (repo_root / Path(repo_relative_path)).resolve()
    _assert_within_repo(cube_path, repo_root)
    return CubeGitContext(
        cube_id=cube_id,
        cube_path=cube_path,
        repo_root=repo_root,
        repo_relative_path=repo_relative_path,
        source_kind="local",
        namespace=parsed.namespace,
    )


def _assert_within_repo(cube_path: Path, repo_root: Path) -> None:
    """Reject repo-relative paths that escape the managed repo root."""

    try:
        cube_path.relative_to(repo_root)
    except ValueError as exc:
        raise BackendError(
            "Cube id path must stay within the managed source", status=400
        ) from exc

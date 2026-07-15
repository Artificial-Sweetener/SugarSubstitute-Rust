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
"""Preflight GitHub cube packs before tracking or syncing them."""

from __future__ import annotations

from dataclasses import dataclass
import json
import logging
from pathlib import Path, PurePosixPath
import shutil
import tempfile
from typing import Any, Callable, Mapping, Optional, Protocol
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from ...cube_model.cube_identity import CubeIdentityError, validate_github_repo_ref
from ..responses import BackendError

_logger = logging.getLogger(__name__)

_GITHUB_TREE_URL = (
    "https://api.github.com/repos/{owner}/{repo}/git/trees/{branch}?recursive=1"
)
_HTTP_TIMEOUT_SECONDS = 10
_RETURNED_PATH_LIMIT = 20
_IGNORED_TOP_LEVEL_DIRS = frozenset({"old", "backup", "_old", "_history"})


class GitResult(Protocol):
    """Describe git command output consumed by repo preflight."""

    @property
    def stdout(self) -> str:
        """Return standard output emitted by the command."""


class GitRunner(Protocol):
    """Run one git command in an explicit working directory."""

    def __call__(self, args: list[str], *, cwd: Path) -> GitResult:
        """Execute one git argument list."""


class TrackedRepoPreflight(Protocol):
    """Inspect and validate one remote cube-pack repository."""

    def inspect_repo(
        self, *, owner: str, repo: str, branch: str
    ) -> TrackedRepoPreflightResult:
        """Inspect one repository without requiring cube content."""

    def require_cubes(
        self, *, owner: str, repo: str, branch: str
    ) -> TrackedRepoPreflightResult:
        """Inspect one repository and require cube content."""



@dataclass(frozen=True)
class HttpJsonResponse:
    """Represent one decoded HTTP JSON response."""

    status: int
    headers: Mapping[str, str]
    payload: Any


@dataclass(frozen=True)
class TrackedRepoPreflightResult:
    """Describe whether a remote GitHub repo appears to contain SugarCubes."""

    owner: str
    repo: str
    branch: str
    contains_cubes: bool
    cube_count: int
    cube_paths: tuple[str, ...]
    truncated: bool = False
    checked_via: str = "github_tree"

    def to_payload(self) -> dict[str, Any]:
        """Return a JSON-safe preflight payload."""

        return {
            "owner": self.owner,
            "repo": self.repo,
            "branch": self.branch,
            "contains_cubes": self.contains_cubes,
            "cube_count": self.cube_count,
            "cube_paths": list(self.cube_paths),
            "truncated": self.truncated,
            "checked_via": self.checked_via,
        }


class TrackedRepoPreflightService:
    """Inspect GitHub repos for cube candidates without tracking them."""

    def __init__(
        self,
        *,
        workspace_root: Path,
        git_runner: GitRunner,
        http_json_loader: Optional[
            Callable[[str, Mapping[str, str], int], HttpJsonResponse]
        ] = None,
        timeout_seconds: int = _HTTP_TIMEOUT_SECONDS,
    ) -> None:
        """Initialize the GitHub preflight service."""

        self.workspace_root = workspace_root.resolve()
        self.git_runner = git_runner
        self.http_json_loader = http_json_loader or load_http_json
        self.timeout_seconds = timeout_seconds

    def inspect_repo(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
    ) -> TrackedRepoPreflightResult:
        """Return remote cube candidate information for one GitHub repo."""

        normalized_owner, normalized_repo = self._normalize_repo_ref(owner, repo)
        normalized_branch = branch.strip() or "main"
        response = self._load_github_tree(
            owner=normalized_owner,
            repo=normalized_repo,
            branch=normalized_branch,
        )
        payload = response.payload
        if not isinstance(payload, Mapping):
            raise BackendError(
                f"Could not inspect GitHub repo '{normalized_owner}/{normalized_repo}'",
                status=502,
                details={
                    "repo": f"{normalized_owner}/{normalized_repo}",
                    "branch": normalized_branch,
                    "reason": "invalid_github_response",
                },
            )
        if payload.get("truncated") is True:
            return self._inspect_with_temporary_git_tree(
                owner=normalized_owner,
                repo=normalized_repo,
                branch=normalized_branch,
            )
        raw_tree = payload.get("tree")
        if not isinstance(raw_tree, list):
            raise BackendError(
                f"Could not inspect GitHub repo '{normalized_owner}/{normalized_repo}'",
                status=502,
                details={
                    "repo": f"{normalized_owner}/{normalized_repo}",
                    "branch": normalized_branch,
                    "reason": "invalid_github_tree",
                },
            )
        candidate_paths = [
            str(entry.get("path") or "")
            for entry in raw_tree
            if isinstance(entry, Mapping)
            and entry.get("type") == "blob"
            and is_cube_candidate_path(str(entry.get("path") or ""))
        ]
        return build_preflight_result(
            owner=normalized_owner,
            repo=normalized_repo,
            branch=normalized_branch,
            paths=candidate_paths,
            checked_via="github_tree",
        )

    def require_cubes(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
    ) -> TrackedRepoPreflightResult:
        """Return preflight results or reject repos that do not contain cubes."""

        result = self.inspect_repo(owner=owner, repo=repo, branch=branch)
        if result.contains_cubes:
            return result
        repo_ref = f"{result.owner}/{result.repo}"
        raise BackendError(
            f"Repository '{repo_ref}' does not contain any .cube files on branch '{result.branch}'.",
            status=422,
            details={"repo": repo_ref, "branch": result.branch, "reason": "no_cubes"},
        )

    def _load_github_tree(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
    ) -> HttpJsonResponse:
        """Load one GitHub recursive tree response with typed errors."""

        url = _GITHUB_TREE_URL.format(owner=owner, repo=repo, branch=branch)
        headers = {
            "Accept": "application/vnd.github+json",
            "User-Agent": "ComfyUI-SugarCubes",
        }
        try:
            response = self.http_json_loader(url, headers, self.timeout_seconds)
        except BackendError:
            raise
        except (OSError, RuntimeError, ValueError, URLError) as exc:
            _logger.warning(
                "SugarCubes: repo preflight network failure for %s/%s branch %s: %s",
                owner,
                repo,
                branch,
                exc,
            )
            raise BackendError(
                f"Could not inspect GitHub repo '{owner}/{repo}'",
                status=503,
                details={
                    "repo": f"{owner}/{repo}",
                    "branch": branch,
                    "reason": "unavailable",
                },
            ) from exc
        if response.status == 200:
            return response
        if response.status == 404:
            raise BackendError(
                f"GitHub repo or branch '{owner}/{repo}@{branch}' was not found",
                status=404,
                details={
                    "repo": f"{owner}/{repo}",
                    "branch": branch,
                    "reason": "not_found",
                },
            )
        if response.status == 403:
            raise BackendError(
                f"GitHub preflight for '{owner}/{repo}' is currently unavailable",
                status=503,
                details={
                    "repo": f"{owner}/{repo}",
                    "branch": branch,
                    "reason": "rate_limited",
                },
            )
        status = 503 if response.status >= 500 else 502
        raise BackendError(
            f"Could not inspect GitHub repo '{owner}/{repo}'",
            status=status,
            details={
                "repo": f"{owner}/{repo}",
                "branch": branch,
                "reason": f"github_status_{response.status}",
            },
        )

    def _inspect_with_temporary_git_tree(
        self,
        *,
        owner: str,
        repo: str,
        branch: str,
    ) -> TrackedRepoPreflightResult:
        """Inspect a remote tree through a temporary no-checkout clone."""

        temp_root = self.workspace_root / "_preflight"
        temp_root.mkdir(parents=True, exist_ok=True)
        temp_root_resolved = temp_root.resolve()
        checkout_path: Optional[Path] = None
        try:
            checkout_path = Path(
                tempfile.mkdtemp(prefix="repo-", dir=str(temp_root_resolved))
            ).resolve()
            try:
                checkout_path.relative_to(temp_root_resolved)
            except ValueError as exc:
                raise BackendError(
                    "Temporary preflight path is invalid", status=500
                ) from exc
            remote_url = f"https://github.com/{owner}/{repo}.git"
            self.git_runner(
                [
                    "clone",
                    "--depth=1",
                    "--filter=blob:none",
                    "--no-checkout",
                    "--branch",
                    branch,
                    remote_url,
                    str(checkout_path),
                ],
                cwd=temp_root_resolved,
            )
            result = self.git_runner(
                ["ls-tree", "-r", "--name-only", "HEAD"],
                cwd=checkout_path,
            )
            paths = [
                line.strip()
                for line in (result.stdout or "").splitlines()
                if is_cube_candidate_path(line.strip())
            ]
            return build_preflight_result(
                owner=owner,
                repo=repo,
                branch=branch,
                paths=paths,
                truncated=True,
                checked_via="temporary_git_tree",
            )
        except BackendError:
            raise
        except (OSError, RuntimeError, ValueError) as exc:
            _logger.warning(
                "SugarCubes: temporary repo preflight failed for %s/%s branch %s: %s",
                owner,
                repo,
                branch,
                exc,
            )
            message = str(exc)
            status = (
                404 if "Remote branch" in message and "not found" in message else 503
            )
            reason = "not_found" if status == 404 else "temporary_git_failed"
            raise BackendError(
                f"Could not inspect GitHub repo '{owner}/{repo}'",
                status=status,
                details={"repo": f"{owner}/{repo}", "branch": branch, "reason": reason},
            ) from exc
        finally:
            if checkout_path is not None:
                try:
                    checkout_path.relative_to(temp_root_resolved)
                    shutil.rmtree(checkout_path, ignore_errors=True)
                except ValueError:
                    _logger.error(
                        "SugarCubes: refused to clean invalid preflight path for %s/%s",
                        owner,
                        repo,
                    )

    def _normalize_repo_ref(self, owner: str, repo: str) -> tuple[str, str]:
        """Normalize GitHub owner/repo or raise a backend error."""

        try:
            return validate_github_repo_ref(owner, repo)
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc


def load_http_json(
    url: str, headers: Mapping[str, str], timeout: int
) -> HttpJsonResponse:
    """Load and decode one JSON HTTP response using the standard library."""

    request = Request(url, headers=dict(headers), method="GET")
    try:
        with urlopen(request, timeout=timeout) as response:
            raw = response.read()
            payload = json.loads(raw.decode("utf-8")) if raw else {}
            return HttpJsonResponse(
                status=response.status,
                headers=dict(response.headers.items()),
                payload=payload,
            )
    except HTTPError as exc:
        raw = exc.read()
        error_payload: Any = {}
        if raw:
            try:
                error_payload = json.loads(raw.decode("utf-8"))
            except ValueError:
                error_payload = {}
        return HttpJsonResponse(
            status=exc.code,
            headers=dict(exc.headers.items()),
            payload=error_payload,
        )


def is_cube_candidate_path(path: str) -> bool:
    """Return whether a repo-relative path is a candidate cube file."""

    cleaned = path.strip()
    if not cleaned or cleaned.startswith("/") or "\\" in cleaned:
        return False
    parts = PurePosixPath(cleaned).parts
    if not parts or any(part in {"", ".", ".."} for part in parts):
        return False
    if parts[0].lower() in _IGNORED_TOP_LEVEL_DIRS:
        return False
    return parts[-1].lower().endswith(".cube")


def build_preflight_result(
    *,
    owner: str,
    repo: str,
    branch: str,
    paths: list[str],
    truncated: bool = False,
    checked_via: str,
) -> TrackedRepoPreflightResult:
    """Build one preflight result with stable path ordering and display limits."""

    unique_paths = tuple(sorted(dict.fromkeys(paths)))
    return TrackedRepoPreflightResult(
        owner=owner,
        repo=repo,
        branch=branch,
        contains_cubes=bool(unique_paths),
        cube_count=len(unique_paths),
        cube_paths=unique_paths[:_RETURNED_PATH_LIMIT],
        truncated=truncated,
        checked_via=checked_via,
    )


def list_local_cube_candidate_paths(base_dir: Path) -> tuple[str, ...]:
    """Return repo-relative local cube candidates using preflight path rules."""

    if not base_dir.exists():
        return tuple()
    resolved_base = base_dir.resolve()
    paths: list[str] = []
    for path in resolved_base.rglob("*"):
        if not path.is_file() or path.suffix.lower() != ".cube":
            continue
        if ".git" in path.parts:
            continue
        try:
            relative = path.resolve().relative_to(resolved_base).as_posix()
        except ValueError:
            continue
        if is_cube_candidate_path(relative):
            paths.append(relative)
    return tuple(sorted(paths))

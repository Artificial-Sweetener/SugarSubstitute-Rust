#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Persist and normalize the tracked-repository manifest."""

from __future__ import annotations

import json
from collections.abc import Iterable
from dataclasses import asdict
from pathlib import Path
from typing import Any

from ...cube_model.cube_identity import (
    CubeIdentityError,
    RESERVED_SOURCE_NAMES,
    validate_github_repo_ref,
)
from ..responses import BackendError
from .tracked_repo_models import (
    DEFAULT_BASE_OWNER,
    DEFAULT_BASE_REPO,
    DEFAULT_BRANCH,
    TrackedRepo,
    is_default_base_repo,
    normalize_branch_name,
    normalize_tracked_repo,
    serialize_tracked_repo,
)

_MANIFEST_DIRNAME = ".sugarcubes"
_MANIFEST_NAME = "tracked_repos.json"


class TrackedRepoManifest:
    """Own tracked-repository storage, paths, and persisted normalization."""

    def __init__(self, extension_root: Path) -> None:
        """Initialize manifest storage for one SugarCubes extension root."""

        self._extension_root = extension_root.resolve()

    @property
    def data_root(self) -> Path:
        """Return the extension-owned data root."""

        return self._extension_root / _MANIFEST_DIRNAME

    @property
    def path(self) -> Path:
        """Return the tracked-repository manifest path."""

        return self.data_root / _MANIFEST_NAME

    @property
    def workspace_root(self) -> Path:
        """Return the managed root for tracked GitHub checkouts."""

        return self.data_root

    @property
    def local_repo_root(self) -> Path:
        """Return the managed local source repo root."""

        return self.data_root / "local"

    def checkout_path(self, owner: str, repo: str) -> Path:
        """Return the canonical managed checkout path for one repository."""

        normalized_owner, normalized_repo = self.normalize_repo_ref(owner, repo)
        return self.workspace_root / normalized_owner / normalized_repo

    def load(self) -> list[TrackedRepo]:
        """Load tracked repository entries from disk."""

        self.path.parent.mkdir(parents=True, exist_ok=True)
        if not self.path.exists():
            default_repo = self.build_default_base_repo()
            self.write([default_repo])
            return [default_repo]
        try:
            payload = json.loads(self.path.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            raise BackendError("Tracked repo manifest is invalid", status=500) from exc
        if not isinstance(payload, dict):
            raise BackendError("Tracked repo manifest is invalid", status=500)
        raw_repos = payload.get("repos")
        if not isinstance(raw_repos, list):
            default_repo = self.build_default_base_repo()
            self.write([default_repo])
            return [default_repo]
        parsed = [
            self._parse_entry(entry) for entry in raw_repos if isinstance(entry, dict)
        ]
        normalized = self._normalize_repos(parsed)
        if normalized != parsed:
            self.write(normalized)
        return normalized

    def write(self, repos: Iterable[TrackedRepo]) -> None:
        """Persist tracked repository entries to disk."""

        self.path.parent.mkdir(parents=True, exist_ok=True)
        normalized = self._normalize_repos(repos)
        payload = {"repos": [serialize_tracked_repo(repo) for repo in normalized]}
        self.path.write_text(
            json.dumps(payload, indent=2) + "\n",
            encoding="utf-8",
        )

    def replace(self, repo: TrackedRepo, **changes: Any) -> TrackedRepo:
        """Rewrite one tracked repository entry in the manifest."""

        replacement = self.normalize(TrackedRepo(**{**asdict(repo), **changes}))
        self.write(
            (
                replacement
                if entry.owner == repo.owner and entry.repo == repo.repo
                else entry
            )
            for entry in self.load()
        )
        return replacement

    def build_default_base_repo(self) -> TrackedRepo:
        """Return the standard default tracked Base-Cubes entry."""

        owner, repo = self.normalize_repo_ref(DEFAULT_BASE_OWNER, DEFAULT_BASE_REPO)
        return TrackedRepo(
            owner=owner,
            repo=repo,
            branch=DEFAULT_BRANCH,
            enabled=True,
            default_base_repo=True,
            auto_update=False,
            local_checkout_path=str(self.checkout_path(owner, repo)),
        )

    def normalize_repo_ref(self, owner: str, repo: str) -> tuple[str, str]:
        """Normalize and validate one GitHub owner/repo reference."""

        owner_candidate = owner.strip() if isinstance(owner, str) else ""
        if owner_candidate.lower() in RESERVED_SOURCE_NAMES:
            raise BackendError(
                f"GitHub owner '{owner_candidate}' is reserved by SugarCubes",
                status=400,
            )
        try:
            return validate_github_repo_ref(owner, repo)
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc

    def normalize(self, repo: TrackedRepo) -> TrackedRepo:
        """Normalize repository invariants and its active checkout path."""

        normalized = normalize_tracked_repo(repo)
        return TrackedRepo(
            **{
                **asdict(normalized),
                "local_checkout_path": self._normalize_checkout_path(
                    owner=normalized.owner,
                    repo=normalized.repo,
                    persisted_path=normalized.local_checkout_path,
                ),
            }
        )

    def _normalize_repos(self, repos: Iterable[TrackedRepo]) -> list[TrackedRepo]:
        """Ensure canonical Base-Cubes exists and owns the base-pack flag."""

        normalized = [self.normalize(repo) for repo in repos]
        if any(is_default_base_repo(repo.owner, repo.repo) for repo in normalized):
            return normalized
        return [self.build_default_base_repo(), *normalized]

    def _parse_entry(self, entry: dict[str, Any]) -> TrackedRepo:
        """Parse one persisted tracked-repository entry."""

        owner, repo = validate_github_repo_ref(
            str(entry.get("owner") or ""),
            str(entry.get("repo") or ""),
        )
        return TrackedRepo(
            owner=owner,
            repo=repo,
            branch=normalize_branch_name(str(entry.get("branch") or "")),
            enabled=bool(entry.get("enabled", True)),
            default_base_repo=is_default_base_repo(owner, repo),
            auto_update=bool(entry.get("auto_update", False)),
            local_checkout_path=str(entry.get("local_checkout_path") or ""),
            last_sync_at=str(entry.get("last_sync_at") or ""),
            last_sync_status=str(entry.get("last_sync_status") or "never"),
            last_sync_error=str(entry.get("last_sync_error") or ""),
            last_checked_at=str(entry.get("last_checked_at") or ""),
            last_check_status=str(entry.get("last_check_status") or "never"),
            last_check_error=str(entry.get("last_check_error") or ""),
            remote_head_sha=str(entry.get("remote_head_sha") or ""),
            local_head_sha=str(entry.get("local_head_sha") or ""),
            update_available=bool(entry.get("update_available", False)),
        )

    def _normalize_checkout_path(
        self,
        *,
        owner: str,
        repo: str,
        persisted_path: str,
    ) -> str:
        """Return the active checkout path for stale or missing manifests."""

        canonical_path = self.checkout_path(owner, repo).resolve()
        if not persisted_path.strip():
            return str(canonical_path)
        persisted = Path(persisted_path).expanduser()
        try:
            resolved_persisted = persisted.resolve()
        except OSError:
            resolved_persisted = persisted.absolute()
        if resolved_persisted == canonical_path:
            return str(canonical_path)
        if not resolved_persisted.exists():
            return str(canonical_path)
        if canonical_path.exists() and _looks_like_managed_checkout_path(
            resolved_persisted,
            owner=owner,
            repo=repo,
        ):
            return str(canonical_path)
        return str(resolved_persisted)


def _looks_like_managed_checkout_path(
    path: Path,
    *,
    owner: str,
    repo: str,
) -> bool:
    """Return whether a path follows SugarCubes' managed checkout layout."""

    parts = path.parts
    return len(parts) >= 4 and (
        parts[-3].lower() == _MANIFEST_DIRNAME
        and parts[-2] == owner
        and parts[-1] == repo
    )

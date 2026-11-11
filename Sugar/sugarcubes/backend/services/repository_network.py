#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Bound libgit2 network operations in an isolated Python process."""

from __future__ import annotations

import json
from pathlib import Path
import shutil
import subprocess
import sys
from typing import Any

from .repository_service import (
    RepositoryOperationError,
    RepositoryReferenceNotFoundError,
)

_NETWORK_TIMEOUT_SECONDS = 180
_WORKER_PATH = Path(__file__).with_name("repository_network_worker.py")


class RepositoryNetworkClient:
    """Run remote libgit2 work with an enforceable wall-clock timeout."""

    def clone(
        self,
        repository_url: str,
        target_path: Path,
        *,
        branch: str | None,
        depth: int,
    ) -> None:
        """Clone one repository and remove newly created partial output on failure."""

        target_existed = target_path.exists()
        arguments = ["clone", repository_url, str(target_path), "--depth", str(depth)]
        if branch:
            arguments.extend(("--branch", branch))
        try:
            self._run(arguments)
        except RepositoryOperationError:
            if not target_existed and target_path.exists():
                shutil.rmtree(target_path)
            raise

    def fetch(
        self,
        repository_path: Path,
        *,
        remote_name: str,
        branch: str | None,
        tags: bool,
    ) -> None:
        """Fetch selected refs into an existing repository."""

        arguments = ["fetch", str(repository_path), "--remote", remote_name]
        if branch:
            arguments.extend(("--branch", branch))
        if tags:
            arguments.append("--tags")
        self._run(arguments)

    def remote_branch_commit_id(self, repository_url: str, branch: str) -> str:
        """Return one remote branch identifier without modifying a checkout."""

        payload = self._run(["remote-head", repository_url, branch])
        commit_id = payload.get("commitId", "")
        return commit_id if isinstance(commit_id, str) else ""

    def _run(self, arguments: list[str]) -> dict[str, Any]:
        """Execute the internal worker and translate its structured result."""

        command = [sys.executable, str(_WORKER_PATH), *arguments]
        creation_flags = int(getattr(subprocess, "CREATE_NO_WINDOW", 0))
        try:
            completed = subprocess.run(  # noqa: S603
                command,
                check=False,
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                timeout=_NETWORK_TIMEOUT_SECONDS,
                creationflags=creation_flags,
            )
        except (OSError, subprocess.TimeoutExpired) as exc:
            raise RepositoryOperationError(
                f"Repository network operation did not complete safely: {exc}"
            ) from exc
        payload = _load_payload(completed.stdout)
        if completed.returncode == 0:
            return payload
        message = payload.get("message")
        reason = message if isinstance(message, str) else completed.stderr.strip()
        if payload.get("kind") == "reference_not_found":
            raise RepositoryReferenceNotFoundError(
                reason or "Remote reference not found"
            )
        raise RepositoryOperationError(reason or "Repository network operation failed")


def _load_payload(output: str) -> dict[str, Any]:
    """Decode one worker response without trusting its shape."""

    try:
        value = json.loads(output)
    except json.JSONDecodeError:
        return {}
    return value if isinstance(value, dict) else {}


__all__ = ["RepositoryNetworkClient"]

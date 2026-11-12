#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Verify the bounded internal libgit2 network process contract."""

from __future__ import annotations

from collections.abc import Sequence
from pathlib import Path
import subprocess
import sys
from typing import Any

import pytest

from sugarcubes.backend.services.repository_network import RepositoryNetworkClient
from sugarcubes.backend.services.repository_service import RepositoryOperationError


def test_clone_timeout_removes_only_new_partial_output(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Bound network work and clean a partial clone without touching prior data."""

    target = tmp_path / "checkout"

    def timeout(
        command: Sequence[str],
        **kwargs: Any,
    ) -> subprocess.CompletedProcess[str]:
        target.mkdir()
        (target / "partial").write_text("incomplete", encoding="utf-8")
        assert Path(command[0]).resolve() == Path(sys.executable).resolve()
        assert kwargs["timeout"] == 180
        raise subprocess.TimeoutExpired(command, kwargs["timeout"])

    monkeypatch.setattr(subprocess, "run", timeout)

    with pytest.raises(RepositoryOperationError, match="did not complete safely"):
        RepositoryNetworkClient().clone(
            "https://github.com/example/repository",
            target,
            branch="main",
            depth=1,
        )

    assert not target.exists()


def test_clone_failure_preserves_preexisting_target(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Never delete a directory that existed before a rejected clone request."""

    target = tmp_path / "checkout"
    target.mkdir()
    marker = target / "user-data"
    marker.write_text("keep", encoding="utf-8")

    def fail(
        command: Sequence[str],
        **kwargs: Any,
    ) -> subprocess.CompletedProcess[str]:
        _ = kwargs
        return subprocess.CompletedProcess(
            command,
            1,
            stdout='{"kind":"failure","message":"clone failed"}',
            stderr="",
        )

    monkeypatch.setattr(subprocess, "run", fail)

    with pytest.raises(RepositoryOperationError, match="clone failed"):
        RepositoryNetworkClient().clone(
            "https://github.com/example/repository",
            target,
            branch="main",
            depth=1,
        )

    assert marker.read_text(encoding="utf-8") == "keep"

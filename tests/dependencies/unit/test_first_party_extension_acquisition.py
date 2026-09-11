#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Prove exact first-party acquisition and archive ownership behavior."""

from __future__ import annotations

import shutil
import subprocess
import zipfile
from collections.abc import Sequence
from pathlib import Path

import pytest

from sugarcubes.backend.services.dependency_acquisition import DependencyAcquirer
from sugarcubes.backend.services.dependency_cli import ComfyCliAdapter, SubprocessRunner
from sugarcubes.backend.services.dependency_first_party_manifest import (
    FirstPartyExtension,
    first_party_extension,
)
from sugarcubes.backend.services.dependency_python_requirements import (
    DependencyPythonRequirementsInstaller,
    PythonRequirementsResult,
)
from sugarcubes.backend.services.dependency_source_archive import (
    TrustedSourceArchiveInstaller,
    extract_single_root_archive,
)


class RecordingRequirementsInstaller(DependencyPythonRequirementsInstaller):
    """Record requirements calls without changing the test Python environment."""

    def __init__(self, *, succeeds: bool = True) -> None:
        """Initialize deterministic installation behavior."""

        self.paths: list[Path] = []
        self._succeeds = succeeds

    def install(self, requirements_path: Path) -> PythonRequirementsResult:
        """Record one request and return the configured result."""

        self.paths.append(requirements_path)
        return PythonRequirementsResult(
            command=("python", "-m", "pip"),
            return_code=0 if self._succeeds else 1,
            stdout="ok" if self._succeeds else "",
            stderr="" if self._succeeds else "pip failed",
        )


def test_manifest_resolves_simple_syrup_as_an_exact_official_release() -> None:
    """Keep first-party identity and archive construction in one manifest."""

    extension = first_party_extension("simplesyrup")

    assert extension is not None
    assert extension.registry_id == "SimpleSyrup"
    assert extension.target_folder_name == "SimpleSyrup"
    assert extension.archive_url("1.7.1").endswith(
        "/Artificial-Sweetener/SimpleSyrup/archive/refs/tags/v1.7.1.zip"
    )


def test_registry_receives_the_exact_required_version_before_fallback(
    tmp_path: Path,
) -> None:
    """Request the cube's semver instead of Registry latest."""

    commands: list[list[str]] = []

    def runner(
        command: Sequence[str], cwd: Path, timeout_seconds: int
    ) -> subprocess.CompletedProcess[str]:
        _ = cwd, timeout_seconds
        commands.append(list(command))
        return subprocess.CompletedProcess(command, 0, stdout="ok", stderr="")

    acquirer = _acquirer(tmp_path, runner=runner)

    result = acquirer.acquire(_install_item())

    assert result["returnCode"] == 0
    assert result["acquisitionSource"] == "registry"
    assert commands[1][-1] == "SimpleSyrup@1.7.1"
    assert not (tmp_path / "custom_nodes" / "SimpleSyrup").exists()


def test_registry_failure_installs_the_verified_official_tag(
    tmp_path: Path,
) -> None:
    """Fall back from unavailable Registry metadata to the exact GitHub tag."""

    archive_path = _release_archive(tmp_path)

    def runner(
        command: Sequence[str], cwd: Path, timeout_seconds: int
    ) -> subprocess.CompletedProcess[str]:
        _ = cwd, timeout_seconds
        is_availability_check = command[-1] == "import comfy_cli"
        return subprocess.CompletedProcess(
            command,
            0 if is_availability_check else 1,
            stdout="",
            stderr="Registry version unavailable",
        )

    requirements = RecordingRequirementsInstaller()
    acquirer = _acquirer(
        tmp_path,
        runner=runner,
        archive_path=archive_path,
        requirements=requirements,
    )

    result = acquirer.acquire(_install_item())

    installed = tmp_path / "custom_nodes" / "SimpleSyrup"
    assert result["returnCode"] == 0
    assert result["acquisitionSource"] == "github_tag"
    assert result["requestedVersion"] == "1.7.1"
    assert result["registryAttempt"]["returnCode"] == 1
    assert (installed / "simple_syrup" / "__init__.py").is_file()
    assert requirements.paths[0].name == "requirements.txt"
    assert "simple_syrup/__init__.py" in (installed / ".tracking").read_text(
        encoding="utf-8"
    )


def test_unknown_extension_never_uses_the_first_party_fallback(tmp_path: Path) -> None:
    """Keep arbitrary workflow node ids outside the trusted source channel."""

    source_calls: list[str] = []

    def runner(
        command: Sequence[str], cwd: Path, timeout_seconds: int
    ) -> subprocess.CompletedProcess[str]:
        _ = cwd, timeout_seconds
        return subprocess.CompletedProcess(command, 1, stdout="", stderr="failed")

    acquirer = _acquirer(tmp_path, runner=runner, source_calls=source_calls)
    item = {**_install_item(), "nodeId": "SomeThirdPartyNode"}

    result = acquirer.acquire(item)

    assert result["returnCode"] == 1
    assert result["operation"] == "comfy_registry_install"
    assert source_calls == []


def test_archive_update_preserves_unowned_files_and_replaces_owned_files(
    tmp_path: Path,
) -> None:
    """Update only Comfy-tracked source while retaining extension-owned data."""

    archive_path = _release_archive(tmp_path)
    target = tmp_path / "custom_nodes" / "SimpleSyrup"
    (target / "simple_syrup").mkdir(parents=True)
    (target / "simple_syrup" / "__init__.py").write_text("old", encoding="utf-8")
    (target / "removed.py").write_text("old", encoding="utf-8")
    (target / "user-data.json").write_text("keep", encoding="utf-8")
    (target / ".tracking").write_text(
        "simple_syrup/__init__.py\nremoved.py",
        encoding="utf-8",
    )
    installer = _source_installer(tmp_path, archive_path=archive_path)

    installer.install(extension=_simple_syrup(), version="1.7.1")

    assert (target / "simple_syrup" / "__init__.py").read_text(
        encoding="utf-8"
    ) == "release"
    assert not (target / "removed.py").exists()
    assert (target / "user-data.json").read_text(encoding="utf-8") == "keep"


def test_archive_install_refuses_unowned_and_git_destinations(tmp_path: Path) -> None:
    """Never replace a manual folder or Git checkout with archive contents."""

    archive_path = _release_archive(tmp_path)
    installer = _source_installer(tmp_path, archive_path=archive_path)
    target = tmp_path / "custom_nodes" / "SimpleSyrup"
    target.mkdir(parents=True)
    (target / "manual.py").write_text("keep", encoding="utf-8")

    with pytest.raises(RuntimeError, match="unowned"):
        installer.install(extension=_simple_syrup(), version="1.7.1")

    (target / ".git").mkdir()
    with pytest.raises(RuntimeError, match="unowned"):
        installer.install(extension=_simple_syrup(), version="1.7.1")
    assert (target / "manual.py").read_text(encoding="utf-8") == "keep"


def test_invalid_archive_identity_and_failed_requirements_leave_no_install(
    tmp_path: Path,
) -> None:
    """Validate source and requirements before mutating custom_nodes."""

    wrong_archive = _release_archive(tmp_path, version="9.9.9")
    installer = _source_installer(tmp_path, archive_path=wrong_archive)

    with pytest.raises(RuntimeError, match="Source identity"):
        installer.install(extension=_simple_syrup(), version="1.7.1")
    assert not (tmp_path / "custom_nodes" / "SimpleSyrup").exists()

    valid_archive = _release_archive(tmp_path, filename="valid.zip")
    failing_requirements = RecordingRequirementsInstaller(succeeds=False)
    installer = _source_installer(
        tmp_path,
        archive_path=valid_archive,
        requirements=failing_requirements,
    )
    with pytest.raises(RuntimeError, match="Could not install"):
        installer.install(extension=_simple_syrup(), version="1.7.1")
    assert not (tmp_path / "custom_nodes" / "SimpleSyrup").exists()


def test_archive_extraction_rejects_path_traversal(tmp_path: Path) -> None:
    """Reject archive members that escape the extraction transaction."""

    archive_path = tmp_path / "unsafe.zip"
    with zipfile.ZipFile(archive_path, "w") as archive:
        archive.writestr("SimpleSyrup-1.7.1/../../escaped.py", "unsafe")

    with pytest.raises(RuntimeError, match="unsafe path"):
        extract_single_root_archive(
            archive_path=archive_path,
            target_path=tmp_path / "extracted",
        )
    assert not (tmp_path / "escaped.py").exists()


def _acquirer(
    tmp_path: Path,
    *,
    runner: SubprocessRunner,
    archive_path: Path | None = None,
    requirements: RecordingRequirementsInstaller | None = None,
    source_calls: list[str] | None = None,
) -> DependencyAcquirer:
    """Build the production acquisition path around deterministic boundaries."""

    def download(url: str, target: Path) -> None:
        if source_calls is not None:
            source_calls.append(url)
        if archive_path is None:
            raise AssertionError("source fallback must not run")
        shutil.copy2(archive_path, target)

    return DependencyAcquirer(
        workspace_path=tmp_path / "ComfyUI",
        cli_adapter=ComfyCliAdapter(runner=runner),
        source_installer=TrustedSourceArchiveInstaller(
            custom_nodes_root=tmp_path / "custom_nodes",
            requirements_installer=requirements or RecordingRequirementsInstaller(),
            downloader=download,
        ),
    )


def _source_installer(
    tmp_path: Path,
    *,
    archive_path: Path,
    requirements: RecordingRequirementsInstaller | None = None,
) -> TrustedSourceArchiveInstaller:
    """Build an archive installer that copies a deterministic fixture."""

    def download(url: str, target: Path) -> None:
        _ = url
        shutil.copy2(archive_path, target)

    return TrustedSourceArchiveInstaller(
        custom_nodes_root=tmp_path / "custom_nodes",
        requirements_installer=requirements or RecordingRequirementsInstaller(),
        downloader=download,
    )


def _release_archive(
    tmp_path: Path,
    *,
    version: str = "1.7.1",
    filename: str = "release.zip",
) -> Path:
    """Create a synthetic official-shaped SimpleSyrup release archive."""

    archive_path = tmp_path / filename
    project = (
        "[project]\n"
        f'name = "SimpleSyrup"\nversion = "{version}"\n'
        '[project.urls]\nRepository = "https://github.com/Artificial-Sweetener/SimpleSyrup"\n'
    )
    with zipfile.ZipFile(archive_path, "w") as archive:
        root = "SimpleSyrup-1.7.1"
        archive.writestr(f"{root}/__init__.py", "release")
        archive.writestr(f"{root}/simple_syrup/__init__.py", "release")
        archive.writestr(f"{root}/pyproject.toml", project)
        archive.writestr(f"{root}/requirements.txt", "")
        archive.writestr(f"{root}/tests/not-installed.py", "ignored")
    return archive_path


def _install_item() -> dict[str, object]:
    """Return one approved missing SimpleSyrup requirement."""

    return {
        "nodeId": "SimpleSyrup",
        "requiredVersion": "1.7.1",
        "requiredVersionKind": "semver",
        "installedEvidence": None,
    }


def _simple_syrup() -> FirstPartyExtension:
    """Return the required SimpleSyrup manifest entry."""

    extension = first_party_extension("SimpleSyrup")
    assert extension is not None
    return extension

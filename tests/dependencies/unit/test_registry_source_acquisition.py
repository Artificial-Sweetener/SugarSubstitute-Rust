#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Prove general Registry-source acquisition and archive ownership behavior."""

from __future__ import annotations

import shutil
import stat
import subprocess
import zipfile
from collections.abc import Sequence
from pathlib import Path

import pytest

from sugarcubes.backend.services.dependency_acquisition import DependencyAcquirer
from sugarcubes.backend.services.dependency_cli import ComfyCliAdapter, SubprocessRunner
from sugarcubes.backend.services.dependency_registry_source import (
    RegistrySource,
    RegistrySourceResolver,
)
from sugarcubes.backend.services.dependency_python_requirements import (
    DependencyPythonRequirementsInstaller,
    PythonRequirementsResult,
)
from sugarcubes.backend.services.dependency_source_archive import (
    TrustedSourceArchiveInstaller,
    extract_single_root_archive,
)
from sugarcubes.backend.services.dependency_source_git import (
    RegistrySourceGitInstaller,
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


def test_registry_resolves_arbitrary_node_source_and_archive_candidates() -> None:
    """Derive source identity from Registry metadata without a node allowlist."""

    extension = RegistrySourceResolver(
        loader=lambda node_id: {
            "id": node_id,
            "repository": "https://github.com/Artificial-Sweetener/SimpleSyrup",
        },
        install_loader=lambda node_id, version: None,
    ).resolve("SimpleSyrup")

    assert extension.node_id == "SimpleSyrup"
    assert extension.target_folder_name == "SimpleSyrup"
    release_urls = extension.archive_urls("1.7.1")
    assert release_urls[0].endswith(
        "/Artificial-Sweetener/SimpleSyrup/archive/refs/tags/v1.7.1.zip"
    )
    assert len(release_urls) == 2
    assert all("HEAD" not in url for url in release_urls)
    assert extension.archive_urls("")[0].endswith("/SimpleSyrup/archive/HEAD.zip")
    assert extension.archive_urls("a" * 40)[0].endswith(
        f"/SimpleSyrup/archive/{'a' * 40}.zip"
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


def test_cli_failure_installs_the_exact_registry_artifact(
    tmp_path: Path,
) -> None:
    """Fall back from Comfy CLI to the exact package published by Registry."""

    archive_path = _release_archive(tmp_path, flat=True)

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
    assert result["acquisitionSource"] == "registry_artifact"
    assert result["requestedVersion"] == "1.7.1"
    assert result["registryAttempt"]["returnCode"] == 1
    assert (installed / "simple_syrup" / "__init__.py").is_file()
    assert requirements.paths[0].name == "requirements.txt"
    assert "simple_syrup/__init__.py" in (installed / ".tracking").read_text(
        encoding="utf-8"
    )


def test_arbitrary_registry_node_uses_authoritative_source_fallback(
    tmp_path: Path,
) -> None:
    """Acquire any cube-required pack from its Registry-declared repository."""

    source_calls: list[str] = []

    def runner(
        command: Sequence[str], cwd: Path, timeout_seconds: int
    ) -> subprocess.CompletedProcess[str]:
        _ = cwd, timeout_seconds
        return subprocess.CompletedProcess(command, 1, stdout="", stderr="failed")

    archive_path = _release_archive(
        tmp_path,
        project_name="SomeThirdPartyNode",
        repository_url="https://github.com/example/SomeThirdPartyNode",
        flat=True,
    )
    acquirer = _acquirer(
        tmp_path,
        runner=runner,
        archive_path=archive_path,
        source_calls=source_calls,
        source_payload={
            "id": "SomeThirdPartyNode",
            "repository": "https://github.com/example/SomeThirdPartyNode",
        },
    )
    item = {**_install_item(), "nodeId": "SomeThirdPartyNode"}

    result = acquirer.acquire(item)

    assert result["returnCode"] == 0
    assert result["operation"] == "registry_source_install"
    assert source_calls[0] == "https://cdn.comfy.org/test/node.zip"
    assert (tmp_path / "custom_nodes" / "SomeThirdPartyNode" / "__init__.py").is_file()


def test_flagged_exact_release_uses_validated_github_tag_instead_of_registry(
    tmp_path: Path,
) -> None:
    """Bypass Registry execution and CDN artifacts for a flagged exact release."""

    commands: list[list[str]] = []
    source_calls: list[str] = []

    def runner(
        command: Sequence[str], cwd: Path, timeout_seconds: int
    ) -> subprocess.CompletedProcess[str]:
        _ = cwd, timeout_seconds
        commands.append(list(command))
        return subprocess.CompletedProcess(command, 0, stdout="ok", stderr="")

    archive_path = _release_archive(
        tmp_path,
        version="3.0.0-beta.10",
        project_name="comfyui-prompt-control",
        repository_url="https://github.com/asagi4/comfyui-prompt-control",
    )
    acquirer = _acquirer(
        tmp_path,
        runner=runner,
        archive_path=archive_path,
        source_calls=source_calls,
        source_payload={
            "id": "comfyui-prompt-control",
            "repository": "https://github.com/asagi4/comfyui-prompt-control",
        },
        registry_status="NodeVersionStatusFlagged",
    )

    result = acquirer.acquire(
        {
            "nodeId": "comfyui-prompt-control",
            "requiredVersion": "3.0.0-beta.10",
            "requiredVersionKind": "semver",
            "requiredVersionPolicy": "exact",
            "installedEvidence": None,
        }
    )

    assert result["returnCode"] == 0
    assert result["acquisitionSource"] == "github_source"
    assert commands == []
    assert source_calls == [
        "https://github.com/asagi4/comfyui-prompt-control/archive/refs/tags/"
        "v3.0.0-beta.10.zip"
    ]


def test_registry_source_rejects_non_github_or_mismatched_metadata() -> None:
    """Do not turn cube ids or arbitrary Registry fields into unsafe downloads."""

    with pytest.raises(ValueError, match="public HTTPS GitHub"):
        RegistrySourceResolver(
            loader=lambda node_id: {
                "id": node_id,
                "repository": "https://example.test/owner/repo",
            },
            install_loader=lambda node_id, version: None,
        ).resolve("example")
    with pytest.raises(ValueError, match="does not match"):
        RegistrySourceResolver(
            loader=lambda node_id: {
                "id": "different",
                "repository": "https://github.com/owner/repo",
            },
            install_loader=lambda node_id, version: None,
        ).resolve("example")


def test_missing_sha_requirement_installs_manager_visible_git_checkout(
    tmp_path: Path,
) -> None:
    """Keep SHA-only fallback as an exact Git checkout Comfy can manage."""

    required_sha = "a" * 40
    commands: list[list[str]] = []

    def git_runner(command: Sequence[str], *, cwd: Path) -> object:
        """Materialize a deterministic checkout for the Git adapter."""

        commands.append(list(command))
        if command[0] == "clone":
            source_path = Path(command[-1])
            (source_path / ".git").mkdir(parents=True)
            (source_path / "__init__.py").write_text("", encoding="utf-8")
            (source_path / "pyproject.toml").write_text(
                "[project]\n"
                'name = "example-node"\n'
                'version = "2.0.0"\n'
                "[project.urls]\n"
                'Repository = "https://github.com/example/example-node"\n',
                encoding="utf-8",
            )
        return subprocess.CompletedProcess(command, 0, stdout="", stderr="")

    result = RegistrySourceGitInstaller(
        custom_nodes_root=tmp_path / "custom_nodes",
        git_runner=git_runner,
        requirements_installer=RecordingRequirementsInstaller(),
    ).install(
        extension=RegistrySource(
            node_id="example-node",
            project_name="example-node",
            repository_url="https://github.com/example/example-node",
            target_folder_name="example-node",
        ),
        git_ref=required_sha,
    )

    assert result.target_path.name == "example-node"
    assert (result.target_path / ".git" / ".cnr-id").read_text(
        encoding="utf-8"
    ) == "example-node"
    assert ["cat-file", "-e", f"{required_sha}^{{commit}}"] in commands
    assert ["checkout", "--detach", required_sha] in commands


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


def test_archive_extraction_materializes_safe_relative_file_aliases(
    tmp_path: Path,
) -> None:
    """Install release archives whose docs use safe relative symlinks."""

    archive_path = tmp_path / "aliases.zip"
    with zipfile.ZipFile(archive_path, "w") as archive:
        archive.writestr("project-1.0.0/docs/source.md", "shared docs")
        alias = zipfile.ZipInfo("project-1.0.0/docs/alias.md")
        alias.create_system = 3
        alias.external_attr = (stat.S_IFLNK | 0o777) << 16
        archive.writestr(alias, "source.md")

    source_root = extract_single_root_archive(
        archive_path=archive_path,
        target_path=tmp_path / "extracted",
    )

    alias_path = source_root / "docs" / "alias.md"
    assert alias_path.read_text(encoding="utf-8") == "shared docs"
    assert alias_path.is_symlink() is False


def test_archive_extraction_rejects_escaping_or_chained_aliases(tmp_path: Path) -> None:
    """Reject aliases that do not resolve to a regular file in the archive."""

    archive_path = tmp_path / "unsafe-aliases.zip"
    with zipfile.ZipFile(archive_path, "w") as archive:
        archive.writestr("project-1.0.0/docs/source.md", "shared docs")
        alias = zipfile.ZipInfo("project-1.0.0/docs/alias.md")
        alias.create_system = 3
        alias.external_attr = (stat.S_IFLNK | 0o777) << 16
        archive.writestr(alias, "../../../outside.md")

    with pytest.raises(RuntimeError, match="unsafe path"):
        extract_single_root_archive(
            archive_path=archive_path,
            target_path=tmp_path / "extracted",
        )
    assert not (tmp_path / "outside.md").exists()


def _acquirer(
    tmp_path: Path,
    *,
    runner: SubprocessRunner,
    archive_path: Path | None = None,
    requirements: RecordingRequirementsInstaller | None = None,
    source_calls: list[str] | None = None,
    source_payload: dict[str, object] | None = None,
    registry_artifact: bool = True,
    registry_status: str = "NodeVersionStatusActive",
) -> DependencyAcquirer:
    """Build the production acquisition path around deterministic boundaries."""

    def download(url: str, target: Path) -> None:
        if source_calls is not None:
            source_calls.append(url)
        if archive_path is None:
            raise AssertionError("source fallback must not run")
        shutil.copy2(archive_path, target)

    def unexpected_git(command: Sequence[str], cwd: Path) -> object:
        """Reject Git fallback in archive-focused tests."""

        _ = command, cwd
        raise AssertionError("Git source fallback must not run")

    return DependencyAcquirer(
        workspace_path=tmp_path / "ComfyUI",
        cli_adapter=ComfyCliAdapter(runner=runner),
        source_resolver=RegistrySourceResolver(
            loader=lambda node_id: (
                source_payload
                or {
                    "id": node_id,
                    "repository": "https://github.com/Artificial-Sweetener/SimpleSyrup",
                }
            ),
            install_loader=lambda node_id, version: (
                {
                    "node_id": node_id,
                    "version": version,
                    "downloadUrl": "https://cdn.comfy.org/test/node.zip",
                    "status": registry_status,
                }
                if registry_artifact
                else None
            ),
        ),
        source_installer=TrustedSourceArchiveInstaller(
            custom_nodes_root=tmp_path / "custom_nodes",
            requirements_installer=requirements or RecordingRequirementsInstaller(),
            downloader=download,
        ),
        git_installer=RegistrySourceGitInstaller(
            custom_nodes_root=tmp_path / "custom_nodes",
            git_runner=unexpected_git,
            requirements_installer=requirements or RecordingRequirementsInstaller(),
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
    project_name: str = "SimpleSyrup",
    repository_url: str = "https://github.com/Artificial-Sweetener/SimpleSyrup",
    flat: bool = False,
) -> Path:
    """Create a synthetic official-shaped SimpleSyrup release archive."""

    archive_path = tmp_path / filename
    project = (
        "[project]\n"
        f'name = "{project_name}"\nversion = "{version}"\n'
        f'[project.urls]\nRepository = "{repository_url}"\n'
    )
    with zipfile.ZipFile(archive_path, "w") as archive:
        prefix = "" if flat else f"{project_name}-{version}/"
        archive.writestr(f"{prefix}__init__.py", "release")
        archive.writestr(f"{prefix}simple_syrup/__init__.py", "release")
        archive.writestr(f"{prefix}pyproject.toml", project)
        archive.writestr(f"{prefix}requirements.txt", "")
        archive.writestr(f"{prefix}tests/not-installed.py", "ignored")
    return archive_path


def _install_item() -> dict[str, object]:
    """Return one approved missing SimpleSyrup requirement."""

    return {
        "nodeId": "SimpleSyrup",
        "requiredVersion": "1.7.1",
        "requiredVersionKind": "semver",
        "installedEvidence": None,
    }


def _simple_syrup() -> RegistrySource:
    """Return Registry-authoritative SimpleSyrup source metadata."""

    return RegistrySource(
        node_id="SimpleSyrup",
        project_name="SimpleSyrup",
        repository_url="https://github.com/Artificial-Sweetener/SimpleSyrup",
        target_folder_name="SimpleSyrup",
    )

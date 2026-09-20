#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Install Registry-authoritative Git sources for commit-only requirements."""

from __future__ import annotations

import tempfile
import tomllib
from dataclasses import dataclass
from pathlib import Path

from .dependency_python_requirements import DependencyPythonRequirementsInstaller
from .dependency_registry_source import RegistrySource
from .dependency_version_types import GitRunner
from .dependency_versions import classify_version


@dataclass(frozen=True, slots=True)
class SourceGitInstallResult:
    """Describe one Comfy-manageable Git source installation."""

    node_id: str
    repository_url: str
    target_path: Path
    git_ref: str


class RegistrySourceGitInstaller:
    """Clone an authoritative repository at an exact required commit."""

    def __init__(
        self,
        *,
        custom_nodes_root: Path,
        git_runner: GitRunner,
        requirements_installer: DependencyPythonRequirementsInstaller,
    ) -> None:
        """Initialize transaction and external-command boundaries."""

        self._custom_nodes_root = custom_nodes_root.resolve()
        self._git_runner = git_runner
        self._requirements_installer = requirements_installer

    def install(
        self,
        *,
        extension: RegistrySource,
        git_ref: str,
    ) -> SourceGitInstallResult:
        """Install one exact commit as the Git checkout Comfy Manager understands."""

        if classify_version(git_ref) != "git_sha":
            raise ValueError("Git source installation requires an exact commit SHA.")
        target_path = (self._custom_nodes_root / extension.target_folder_name).resolve()
        if not _is_relative_to(target_path, self._custom_nodes_root):
            raise RuntimeError("Registry Git target escapes custom_nodes.")
        if target_path.exists():
            raise RuntimeError(
                f"Refusing to replace existing custom node: {target_path}"
            )
        self._custom_nodes_root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(
            prefix="sugarcubes-git-",
            dir=self._custom_nodes_root,
        ) as temporary_directory:
            source_path = Path(temporary_directory) / "source"
            self._run(
                ["clone", "--no-checkout", extension.repository_url, str(source_path)],
                cwd=self._custom_nodes_root,
            )
            self._run(["fetch", "--all", "--tags"], cwd=source_path)
            self._run(["cat-file", "-e", f"{git_ref}^{{commit}}"], cwd=source_path)
            self._run(["checkout", "--detach", git_ref], cwd=source_path)
            _validate_checkout_identity(source_path, extension)
            requirements_path = source_path / extension.requirements_file
            if requirements_path.is_file():
                requirements = self._requirements_installer.install(requirements_path)
                if requirements.return_code != 0:
                    detail = (requirements.stderr or requirements.stdout).strip()[
                        -2000:
                    ]
                    raise RuntimeError(
                        f"Could not install {extension.project_name} requirements: {detail}"
                    )
            (source_path / ".git" / ".cnr-id").write_text(
                extension.node_id,
                encoding="utf-8",
            )
            source_path.replace(target_path)
        return SourceGitInstallResult(
            node_id=extension.node_id,
            repository_url=extension.repository_url,
            target_path=target_path,
            git_ref=git_ref,
        )

    def _run(self, command: list[str], *, cwd: Path) -> None:
        """Run one Git command and preserve its actionable failure output."""

        result = self._git_runner(command, cwd=cwd)
        return_code = int(getattr(result, "returncode", 0) or 0)
        if return_code == 0:
            return
        detail = str(
            getattr(result, "stderr", "") or getattr(result, "stdout", "")
        ).strip()[-2000:]
        raise RuntimeError(f"Git command failed ({return_code}): {detail}")


def _validate_checkout_identity(source_path: Path, extension: RegistrySource) -> None:
    """Prove the checkout declares the Registry node and repository identities."""

    try:
        payload = tomllib.loads(
            (source_path / "pyproject.toml").read_text(encoding="utf-8")
        )
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise RuntimeError(
            "Registry Git source has unreadable project metadata."
        ) from exc
    project = payload.get("project")
    if not isinstance(project, dict):
        raise RuntimeError("Registry Git source has no project metadata.")
    urls = project.get("urls")
    repository = urls.get("Repository") if isinstance(urls, dict) else ""
    name = project.get("name")
    if (
        not isinstance(name, str)
        or name.casefold() != extension.project_name.casefold()
        or not isinstance(repository, str)
        or _normalized_repository(repository)
        != _normalized_repository(extension.repository_url)
    ):
        raise RuntimeError("Registry Git source identity does not match metadata.")
    if not (source_path / "__init__.py").is_file():
        raise RuntimeError("Registry Git source has no __init__.py.")


def _normalized_repository(value: str) -> str:
    """Normalize repository identity for exact comparison."""

    return value.strip().rstrip("/").removesuffix(".git").casefold()


def _is_relative_to(path: Path, parent: Path) -> bool:
    """Return whether one resolved path is contained by another."""

    try:
        path.relative_to(parent)
    except ValueError:
        return False
    return True


__all__ = ["RegistrySourceGitInstaller", "SourceGitInstallResult"]

#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Install Registry-authoritative source archives transactionally."""

from __future__ import annotations

import shutil
import stat
import tempfile
import tomllib
import urllib.request
import zipfile
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from pathlib import Path

from .dependency_registry_source import RegistrySource
from .dependency_python_requirements import DependencyPythonRequirementsInstaller
from .dependency_versions import classify_version

_DOWNLOAD_TIMEOUT_SECONDS = 120
_MAX_ARCHIVE_BYTES = 100 * 1024 * 1024
_MAX_EXTRACTED_BYTES = 256 * 1024 * 1024
ArchiveDownloader = Callable[[str, Path], None]


@dataclass(frozen=True, slots=True)
class SourceArchiveInstallResult:
    """Describe one verified Registry-source installation."""

    node_id: str
    version: str
    repository_url: str
    archive_url: str
    target_path: Path
    tracked_file_count: int


class TrustedSourceArchiveInstaller:
    """Install Registry-authoritative source without replacing unowned content."""

    def __init__(
        self,
        *,
        custom_nodes_root: Path,
        requirements_installer: DependencyPythonRequirementsInstaller,
        downloader: ArchiveDownloader | None = None,
    ) -> None:
        """Initialize trusted source installation boundaries."""

        self._custom_nodes_root = custom_nodes_root.resolve()
        self._requirements_installer = requirements_installer
        self._downloader = downloader or download_archive

    def install(
        self,
        *,
        extension: RegistrySource,
        version: str,
    ) -> SourceArchiveInstallResult:
        """Install verified source while retaining non-owned local data."""

        target_path = self._target_path(extension)
        old_tracked_files = _owned_paths(target_path)
        if target_path.exists() and old_tracked_files is None:
            raise RuntimeError(
                f"Refusing to replace unowned custom-node folder: {target_path}"
            )
        if extension.package_url:
            archive_url, source_path, new_tracked_files, transaction = (
                self._validated_registry_package(extension=extension)
            )
        else:
            archive_url, source_path, transaction = self._validated_source(
                extension=extension,
                version=version,
            )
            new_tracked_files = tracked_source_files(source_path)
        with transaction:
            backup_path = Path(transaction.name) / "previous"
            self._install_requirements(source_path, extension)
            _apply_source_transaction(
                source_path=source_path,
                target_path=target_path,
                old_tracked_files=old_tracked_files or (),
                new_tracked_files=new_tracked_files,
                backup_path=backup_path,
            )
        return SourceArchiveInstallResult(
            node_id=extension.node_id,
            version=extension.package_version or version,
            repository_url=extension.repository_url,
            archive_url=archive_url,
            target_path=target_path,
            tracked_file_count=len(new_tracked_files),
        )

    def _validated_registry_package(
        self,
        *,
        extension: RegistrySource,
    ) -> tuple[
        str,
        Path,
        tuple[Path, ...],
        tempfile.TemporaryDirectory[str],
    ]:
        """Download and validate the exact package published by Comfy Registry."""

        transaction = tempfile.TemporaryDirectory(prefix="sugarcubes-registry-")
        transaction_root = Path(transaction.name)
        try:
            archive_path = transaction_root / "node.zip"
            source_path = transaction_root / "source"
            self._downloader(extension.package_url, archive_path)
            tracked_files = extract_registry_package_archive(
                archive_path=archive_path,
                target_path=source_path,
            )
            validate_source_identity(
                source_path=source_path,
                extension=extension,
                version=extension.package_version,
            )
        except (OSError, RuntimeError, ValueError, zipfile.BadZipFile):
            transaction.cleanup()
            raise
        return extension.package_url, source_path, tracked_files, transaction

    def _validated_source(
        self,
        *,
        extension: RegistrySource,
        version: str,
    ) -> tuple[str, Path, tempfile.TemporaryDirectory[str]]:
        """Download candidates until one proves the requested source identity."""

        failures: list[Exception] = []
        for archive_url in extension.archive_urls(version):
            transaction = tempfile.TemporaryDirectory(prefix="sugarcubes-extension-")
            transaction_root = Path(transaction.name)
            try:
                archive_path = transaction_root / "source.zip"
                extracted_path = transaction_root / "source"
                self._downloader(archive_url, archive_path)
                source_path = extract_single_root_archive(
                    archive_path=archive_path,
                    target_path=extracted_path,
                )
                validate_source_identity(
                    source_path=source_path,
                    extension=extension,
                    version=version,
                )
            except (OSError, RuntimeError, ValueError, zipfile.BadZipFile) as exc:
                failures.append(exc)
                transaction.cleanup()
                continue
            return archive_url, source_path, transaction
        if failures:
            raise RuntimeError(str(failures[-1])) from failures[-1]
        raise RuntimeError("Registry source exposes no usable archive.")

    def _target_path(self, extension: RegistrySource) -> Path:
        """Resolve a Registry-owned folder and reject escaped destinations."""

        target_path = (self._custom_nodes_root / extension.target_folder_name).resolve()
        if not _is_relative_to(target_path, self._custom_nodes_root):
            raise RuntimeError("Registry source target escapes custom_nodes.")
        return target_path

    def _install_requirements(
        self,
        source_path: Path,
        extension: RegistrySource,
    ) -> None:
        """Install a conventional dependency declaration when the source has one."""

        requirements_path = source_path / extension.requirements_file
        if not requirements_path.is_file():
            return
        result = self._requirements_installer.install(requirements_path)
        if result.return_code != 0:
            detail = (result.stderr or result.stdout).strip()[-2000:]
            raise RuntimeError(
                f"Could not install {extension.project_name} requirements: {detail}"
            )


def download_archive(archive_url: str, target_path: Path) -> None:
    """Download a validated Registry repository URL with timeout and size bounds."""

    request = urllib.request.Request(
        archive_url,
        headers={"User-Agent": "SugarCubes"},
    )
    downloaded = 0
    with (
        urllib.request.urlopen(  # noqa: S310 - validated Registry provenance.
            request,
            timeout=_DOWNLOAD_TIMEOUT_SECONDS,
        ) as response,
        target_path.open("wb") as output,
    ):
        while chunk := response.read(1024 * 1024):
            downloaded += len(chunk)
            if downloaded > _MAX_ARCHIVE_BYTES:
                raise RuntimeError("Registry source archive exceeds the size limit.")
            output.write(chunk)


def extract_single_root_archive(*, archive_path: Path, target_path: Path) -> Path:
    """Extract a bounded zip with one root and no links or escaping paths."""

    target_path.mkdir(parents=True, exist_ok=True)
    resolved_target = target_path.resolve()
    extracted_bytes = 0
    roots: set[str] = set()
    with zipfile.ZipFile(archive_path) as archive:
        for member in archive.infolist():
            member_path = Path(member.filename)
            if not member_path.parts:
                continue
            if member_path.is_absolute() or _zip_member_is_symlink(member):
                raise RuntimeError("Registry source archive contains an unsafe path.")
            destination = (target_path / member_path).resolve()
            if not _is_relative_to(destination, resolved_target):
                raise RuntimeError("Registry source archive contains an unsafe path.")
            extracted_bytes += member.file_size
            if extracted_bytes > _MAX_EXTRACTED_BYTES:
                raise RuntimeError("Registry source archive expands beyond the limit.")
            roots.add(member_path.parts[0])
            if member.is_dir():
                destination.mkdir(parents=True, exist_ok=True)
                continue
            destination.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(member) as source, destination.open("wb") as output:
                shutil.copyfileobj(source, output)
    if len(roots) != 1:
        raise RuntimeError("Registry source archive must contain one root folder.")
    return target_path / next(iter(roots))


def extract_registry_package_archive(
    *,
    archive_path: Path,
    target_path: Path,
) -> tuple[Path, ...]:
    """Extract a bounded flat Registry package and return Comfy tracking paths."""

    target_path.mkdir(parents=True, exist_ok=True)
    resolved_target = target_path.resolve()
    extracted_bytes = 0
    tracked_files: list[Path] = []
    with zipfile.ZipFile(archive_path) as archive:
        for member in archive.infolist():
            member_path = Path(member.filename)
            if not member_path.parts:
                continue
            if member_path.is_absolute() or _zip_member_is_symlink(member):
                raise RuntimeError("Registry package contains an unsafe path.")
            destination = (target_path / member_path).resolve()
            if not _is_relative_to(destination, resolved_target):
                raise RuntimeError("Registry package contains an unsafe path.")
            extracted_bytes += member.file_size
            if extracted_bytes > _MAX_EXTRACTED_BYTES:
                raise RuntimeError("Registry package expands beyond the size limit.")
            if member.is_dir():
                destination.mkdir(parents=True, exist_ok=True)
                continue
            destination.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(member) as source, destination.open("wb") as output:
                shutil.copyfileobj(source, output)
            tracked_files.append(member_path)
    if not tracked_files:
        raise RuntimeError("Registry package contains no files.")
    return tuple(tracked_files)


def validate_source_identity(
    *,
    source_path: Path,
    extension: RegistrySource,
    version: str,
) -> None:
    """Prove archive project, version, repository, and sentinels match policy."""

    if not (source_path / "__init__.py").is_file():
        raise RuntimeError(f"{extension.project_name} source has no __init__.py.")
    project, repository_url = _project_identity(source_path / "pyproject.toml")
    name = project.get("name")
    observed_version = project.get("version")
    if (
        not isinstance(name, str)
        or name.casefold() != extension.project_name.casefold()
        or _normalized_repository(repository_url)
        != _normalized_repository(extension.repository_url)
    ):
        raise RuntimeError(
            f"Source identity does not match {extension.node_id}@{version}."
        )
    if classify_version(version) == "semver" and observed_version != version:
        raise RuntimeError(
            f"Source identity does not match {extension.node_id}@{version}."
        )


def tracked_source_files(source_path: Path) -> tuple[Path, ...]:
    """Return release files Comfy ownership metadata should govern."""

    ignored_directories = {
        ".git",
        ".mypy_cache",
        ".pytest_cache",
        ".ruff_cache",
        "__pycache__",
        "artifacts",
        "node_modules",
        "tests",
    }
    return tuple(
        relative_path
        for file_path in sorted(source_path.rglob("*"))
        if file_path.is_file()
        for relative_path in (file_path.relative_to(source_path),)
        if relative_path.name != ".tracking"
        and not any(part in ignored_directories for part in relative_path.parts[:-1])
    )


def _project_identity(
    pyproject_path: Path,
) -> tuple[Mapping[str, object], str]:
    """Read validated project and repository values from release metadata."""

    try:
        payload = tomllib.loads(pyproject_path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise RuntimeError("Registry source pyproject metadata is unreadable.") from exc
    project_value = payload.get("project")
    if not isinstance(project_value, Mapping):
        raise RuntimeError("Registry source pyproject has no project table.")
    urls_value = project_value.get("urls")
    repository_url = (
        urls_value.get("Repository") if isinstance(urls_value, Mapping) else ""
    )
    return project_value, repository_url if isinstance(repository_url, str) else ""


def _apply_source_transaction(
    *,
    source_path: Path,
    target_path: Path,
    old_tracked_files: tuple[Path, ...],
    new_tracked_files: tuple[Path, ...],
    backup_path: Path,
) -> None:
    """Replace owned source files and restore them after any copy failure."""

    _validate_owned_paths(source_path, new_tracked_files)
    _validate_owned_paths(target_path, old_tracked_files)
    target_preexisted = target_path.exists()
    backup_path.mkdir(parents=True)
    for relative_path in old_tracked_files:
        installed_path = target_path / relative_path
        if installed_path.is_file():
            backup_file = backup_path / relative_path
            backup_file.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(installed_path, backup_file)
    previous_tracking = (
        (target_path / ".tracking").read_bytes()
        if (target_path / ".tracking").is_file()
        else None
    )
    try:
        target_path.mkdir(parents=True, exist_ok=True)
        _remove_owned_files(target_path, old_tracked_files)
        for relative_path in new_tracked_files:
            destination = target_path / relative_path
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source_path / relative_path, destination)
        _write_tracking(target_path, new_tracked_files)
    except (OSError, RuntimeError):
        _remove_owned_files(target_path, new_tracked_files)
        for backup_file in backup_path.rglob("*"):
            if backup_file.is_file():
                relative_path = backup_file.relative_to(backup_path)
                destination = target_path / relative_path
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(backup_file, destination)
        tracking_path = target_path / ".tracking"
        if previous_tracking is None:
            tracking_path.unlink(missing_ok=True)
        else:
            tracking_path.write_bytes(previous_tracking)
        if (
            not target_preexisted
            and target_path.exists()
            and not any(target_path.iterdir())
        ):
            target_path.rmdir()
        raise


def _owned_paths(target_path: Path) -> tuple[Path, ...] | None:
    """Read Comfy's tracked-file list or identify an unowned destination."""

    tracking_path = target_path / ".tracking"
    if not tracking_path.is_file():
        return None
    try:
        paths = tuple(
            Path(line.strip())
            for line in tracking_path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        )
    except OSError as exc:
        raise RuntimeError("Comfy ownership metadata is unreadable.") from exc
    _validate_owned_paths(target_path, paths)
    return paths


def _write_tracking(target_path: Path, tracked_files: tuple[Path, ...]) -> None:
    """Atomically write Comfy-compatible source ownership metadata."""

    _validate_owned_paths(target_path, tracked_files)
    temporary_path = target_path / ".tracking.tmp"
    temporary_path.write_text(
        "\n".join(path.as_posix() for path in tracked_files),
        encoding="utf-8",
    )
    temporary_path.replace(target_path / ".tracking")


def _validate_owned_paths(root: Path, paths: tuple[Path, ...]) -> None:
    """Reject absolute and escaping paths before source mutation."""

    resolved_root = root.resolve()
    for relative_path in paths:
        if relative_path.is_absolute() or not _is_relative_to(
            (root / relative_path).resolve(), resolved_root
        ):
            raise RuntimeError(
                f"Comfy ownership metadata contains an unsafe path: {relative_path}"
            )


def _remove_owned_files(root: Path, paths: tuple[Path, ...]) -> None:
    """Remove owned files and resulting empty owned directories."""

    for relative_path in paths:
        installed_path = root / relative_path
        if installed_path.is_file() or installed_path.is_symlink():
            installed_path.unlink()
    parents = sorted(
        {
            parent
            for relative_path in paths
            for parent in (root / relative_path).parents
            if parent != root and _is_relative_to(parent, root)
        },
        key=lambda path: len(path.parts),
        reverse=True,
    )
    for parent in parents:
        if parent.is_dir() and not any(parent.iterdir()):
            parent.rmdir()


def _zip_member_is_symlink(member: zipfile.ZipInfo) -> bool:
    """Return whether Unix archive attributes describe a symbolic link."""

    return stat.S_ISLNK(member.external_attr >> 16)


def _normalized_repository(value: str) -> str:
    """Normalize trusted repository URLs for exact identity comparison."""

    return value.strip().rstrip("/").removesuffix(".git").casefold()


def _is_relative_to(path: Path, parent: Path) -> bool:
    """Return whether one resolved path is contained by another."""

    try:
        path.relative_to(parent)
    except ValueError:
        return False
    return True


__all__ = [
    "SourceArchiveInstallResult",
    "TrustedSourceArchiveInstaller",
    "download_archive",
    "extract_single_root_archive",
    "extract_registry_package_archive",
    "tracked_source_files",
    "validate_source_identity",
]

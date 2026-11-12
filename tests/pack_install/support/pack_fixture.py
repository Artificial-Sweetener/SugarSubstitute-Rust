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
"""Create registry-installed packs and staged releases in owned temp folders."""

from __future__ import annotations

import hashlib
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from pathlib import Path

from sugarcubes.backend.pack_install.archive import extract_registry_archive
from sugarcubes.backend.pack_install.entry_paths import EntryName
from sugarcubes.backend.pack_install.ownership import (
    OwnedPack,
    inspect_pack_folder,
    installed_file_digests,
)
from sugarcubes.backend.pack_install.replacement import FileOperations
from sugarcubes.backend.pack_install.staging import StagedStore, StagingRequest
from sugarcubes.backend.pack_install.startup import updates_root
from tests.pack_install.support.install_vectors import release_archive


def pack_files(version: str, extra: Mapping[str, str] | None = None) -> dict[str, str]:
    """Return the files of a small SugarCubes release."""

    files = {
        "pyproject.toml": f'[project]\nname = "SugarCubes"\nversion = "{version}"\n',
        "__init__.py": f"VERSION = '{version}'\n",
        "sugarcubes/core.py": f"# core {version}\n",
        "web/app.js": f"// app {version}\n",
    }
    files.update(extra or {})
    return files


def install_registry_pack(root: Path, files: Mapping[str, str]) -> Path:
    """Install a pack exactly as ComfyUI Manager does: extract, then record."""

    pack = root / "custom_nodes" / "SugarCubes"
    root.mkdir(parents=True, exist_ok=True)
    archive = release_archive(root / "installed.zip", files)
    extracted = extract_registry_archive(archive, pack)
    (pack / ".tracking").write_text(
        "\n".join(entry.text for entry in extracted.entries), encoding="utf-8"
    )
    archive.unlink()
    return pack


def stage_release(
    root: Path, pack: Path, files: Mapping[str, str], version: str
) -> StagedStore:
    """Stage one release for the installed pack and return the store."""

    owned = inspect_pack_folder(pack)
    assert isinstance(owned, OwnedPack)
    archive = release_archive(root / f"release-{version}.zip", files)
    store = StagedStore(updates_root(pack))
    store.stage(
        StagingRequest(
            archive_path=archive,
            pack_id="SugarCubes",
            version=version,
            from_version=str(owned.identity.version),
            source="registry",
            archive_sha256=hashlib.sha256(archive.read_bytes()).hexdigest(),
            installed_digests=installed_file_digests(owned),
        )
    )
    return store


def snapshot_folder(pack: Path) -> dict[str, bytes]:
    """Return every file of the pack except the private updates area."""

    return {
        path.relative_to(pack).as_posix(): path.read_bytes()
        for path in sorted(pack.rglob("*"))
        if path.is_file()
        and not path.relative_to(pack).as_posix().startswith(".sugarcubes/updates/")
    }


class SimulatedPowerCut(BaseException):
    """Stop a transaction mid-way without running its rollback, like a crash."""


@dataclass
class FaultInjector:
    """Count mutating operations and fail or crash at a chosen one."""

    fail_at: int | None = None
    crash_at: int | None = None
    calls: int = 0

    def operations(self) -> FileOperations:
        """Return file operations that obey this injector."""

        base = FileOperations()

        def guard(name: str) -> None:
            self.calls += 1
            if self.crash_at == self.calls:
                raise SimulatedPowerCut(name)
            if self.fail_at == self.calls:
                raise OSError(f"injected failure in {name}")

        def wrap2(
            function: Callable[[Path, Path], None], name: str
        ) -> Callable[[Path, Path], None]:
            def call(first: Path, second: Path) -> None:
                guard(name)
                function(first, second)

            return call

        def remove(path: Path) -> None:
            guard("remove")
            base.remove(path)

        def write_tracking(folder: Path, entries: tuple[EntryName, ...]) -> None:
            guard("write_tracking")
            base.write_tracking(folder, entries)

        return FileOperations(
            copy=wrap2(base.copy, "copy"),
            remove=remove,
            write_tracking=write_tracking,
        )


__all__ = [
    "FaultInjector",
    "SimulatedPowerCut",
    "install_registry_pack",
    "pack_files",
    "snapshot_folder",
    "stage_release",
]

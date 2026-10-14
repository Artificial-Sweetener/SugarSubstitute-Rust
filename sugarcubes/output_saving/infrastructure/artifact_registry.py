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
"""Persist contained output-file references that Comfy controls can resolve after reload."""

from __future__ import annotations

from dataclasses import dataclass
import logging
from pathlib import Path
import sqlite3
import uuid

MANAGED_SUBFOLDER_PREFIX = "__sugarcubes_managed__/"
_LOGGER = logging.getLogger(__name__)


@dataclass(frozen=True, slots=True)
class ManagedImageReference:
    """Expose a native Comfy image reference while retaining its server-owned identity."""

    artifact_id: str
    filename: str
    type: str
    width: int
    height: int
    companion_filename: str | None = None

    @property
    def subfolder(self) -> str:
        """Keep external output roots out of browser-visible references and query parameters."""
        return f"{MANAGED_SUBFOLDER_PREFIX}{self.artifact_id}"

    def comfy_image(self) -> dict[str, object]:
        """Preserve native preview and download fields with an optional companion reference."""
        image: dict[str, object] = {
            "filename": self.filename,
            "subfolder": self.subfolder,
            "type": self.type,
            "width": self.width,
            "height": self.height,
        }
        if self.companion_filename is not None:
            image["sugarcubes_companion"] = {
                "filename": self.companion_filename,
                "subfolder": self.subfolder,
                "type": self.type,
            }
        return image


class ManagedArtifactRegistry:
    """Own durable opaque file locations independently of queued policy snapshots."""

    def __init__(self, path: Path) -> None:
        """Prepare the artifact namespace at an explicitly configured server state location."""
        self._path = path
        path.parent.mkdir(parents=True, exist_ok=True)
        connection = self._connect()
        try:
            with connection:
                connection.execute(
                    "CREATE TABLE IF NOT EXISTS registry_schema(version INTEGER NOT NULL)"
                )
                connection.execute(
                    "INSERT INTO registry_schema(version) SELECT 1 WHERE NOT EXISTS(SELECT 1 FROM registry_schema)"
                )
                if connection.execute(
                    "SELECT version FROM registry_schema"
                ).fetchall() != [(1,)]:
                    raise ValueError(
                        "Unsupported saved-output artifact registry version."
                    )
                connection.execute(
                    "CREATE TABLE IF NOT EXISTS images(id TEXT PRIMARY KEY,root TEXT NOT NULL,png TEXT NOT NULL,jpeg TEXT,type TEXT NOT NULL,width INTEGER NOT NULL,height INTEGER NOT NULL)"
                )
        finally:
            connection.close()

    def register(
        self,
        *,
        root: Path,
        png: Path,
        jpeg: Path | None,
        saved: bool,
        width: int,
        height: int,
    ) -> ManagedImageReference:
        """Register only committed contained image files without exposing server paths to the client."""
        if not root.is_absolute():
            raise ValueError("Saved image root must be an absolute server path.")
        root = root.resolve()
        png = self._contained(root, png, ".png")
        if jpeg is not None:
            jpeg = self._contained(root, jpeg, ".jpg")
        if width < 1 or height < 1:
            raise ValueError("Saved output dimensions must be positive.")
        identity = str(uuid.uuid4())
        output_type = "output" if saved else "temp"
        connection = self._connect()
        try:
            with connection:
                connection.execute(
                    "INSERT INTO images VALUES(?,?,?,?,?,?,?)",
                    (
                        identity,
                        str(root),
                        str(png),
                        str(jpeg) if jpeg is not None else None,
                        output_type,
                        width,
                        height,
                    ),
                )
        finally:
            connection.close()
        return ManagedImageReference(
            identity,
            png.name,
            output_type,
            width,
            height,
            jpeg.name if jpeg is not None else None,
        )

    def resolve(self, artifact_id: str, filename: str, output_type: str) -> Path | None:
        """Resolve exactly a registered PNG or companion, rechecking current path containment."""
        if not _is_uuid(artifact_id):
            return None
        connection = self._connect()
        try:
            row = connection.execute(
                "SELECT root,png,jpeg,type FROM images WHERE id=?", (artifact_id,)
            ).fetchone()
        finally:
            connection.close()
        if row is None:
            return None
        raw_root: object = row[0]
        raw_type: object = row[3]
        if not isinstance(raw_root, str) or raw_type != output_type:
            return None
        configured_root = Path(raw_root)
        if not configured_root.is_absolute():
            return None
        root = configured_root.resolve()
        for raw_path in row[1:3]:
            if not isinstance(raw_path, str):
                continue
            path = Path(raw_path)
            if not path.is_absolute():
                continue
            if path.name == filename:
                try:
                    return self._contained(root, path, path.suffix)
                except (OSError, ValueError):
                    _LOGGER.debug(
                        "Registered output image is unavailable or no longer contained.",
                        extra={"artifact_id": artifact_id},
                    )
                    return None
        return None

    def _connect(self) -> sqlite3.Connection:
        """Open a bounded local connection whose lifetime is owned by the requesting operation."""
        return sqlite3.connect(self._path, timeout=30)

    @staticmethod
    def _contained(root: Path, path: Path, suffix: str) -> Path:
        """Reject path traversal, symlink escapes, missing files and unexpected media suffixes."""
        resolved = path.resolve()
        if (
            not root.is_absolute()
            or not resolved.is_relative_to(root)
            or not resolved.is_file()
            or suffix not in {".png", ".jpg"}
            or resolved.suffix != suffix
        ):
            raise ValueError(
                "Saved image file is missing or outside its registered output root."
            )
        return resolved


def _is_uuid(value: str) -> bool:
    """Accept only canonical opaque artifact identities before any registry lookup."""
    try:
        return str(uuid.UUID(value)) == value
    except ValueError:
        return False

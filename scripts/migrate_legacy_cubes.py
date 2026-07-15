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
"""Migrate legacy SugarCube files into the current canonical format."""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from sugarcubes.cube_model import (  # noqa: E402
    CubeDocument,
    looks_like_current_cube_payload,
    migrate_legacy_payload,
)


@dataclass(frozen=True)
class MigrationResult:
    """Capture the outcome for one migrated or skipped cube file."""

    path: Path
    status: str
    detail: str = ""


@dataclass(frozen=True)
class MigrationSummary:
    """Capture the overall outcome of one migration run."""

    scanned: int
    migrated: int
    skipped: int
    failed: int
    results: tuple[MigrationResult, ...]




def main(argv: list[str] | None = None) -> int:
    """Run the CLI entry point for the legacy cube migration tool."""

    parser = _build_parser()
    args = parser.parse_args(argv)
    root = Path(args.root).expanduser().resolve()
    summary = migrate_cube_tree(root)
    for result in summary.results:
        detail = f" ({result.detail})" if result.detail else ""
        _write_stdout(f"{result.status}: {result.path}{detail}\n")
    _write_stdout(
        f"scanned={summary.scanned} migrated={summary.migrated} "
        f"skipped={summary.skipped} failed={summary.failed}\n"
    )
    return 0 if summary.failed == 0 else 1


def migrate_cube_tree(root: Path) -> MigrationSummary:
    """Migrate every `.cube` file under the provided root path."""

    cube_files = _list_cube_files(root)
    results = [migrate_cube_file(path) for path in cube_files]
    return MigrationSummary(
        scanned=len(cube_files),
        migrated=sum(1 for result in results if result.status == "migrated"),
        skipped=sum(1 for result in results if result.status == "skipped"),
        failed=sum(1 for result in results if result.status == "failed"),
        results=tuple(results),
    )


def migrate_cube_file(path: Path) -> MigrationResult:
    """Migrate one cube file in place without creating sidecar history files."""

    try:
        payload = _read_json(path)
    except Exception as exc:
        return MigrationResult(path=path, status="failed", detail=str(exc))

    try:
        migrated = _normalize_payload(payload)
    except Exception as exc:
        return MigrationResult(path=path, status="failed", detail=str(exc))
    if migrated is None:
        return MigrationResult(
            path=path, status="skipped", detail="already current format"
        )

    try:
        _replace_json(path, migrated)
    except Exception as exc:
        return MigrationResult(path=path, status="failed", detail=str(exc))
    return MigrationResult(path=path, status="migrated")


def _normalize_payload(payload: dict[str, Any]) -> dict[str, Any] | None:
    """Normalize one legacy or transitional payload into the current format."""

    if looks_like_current_cube_payload(payload):
        normalized = CubeDocument.from_dict(payload).to_dict()
        if normalized == payload:
            return None
        return normalized
    return migrate_legacy_payload(payload).to_dict()


def _build_parser() -> argparse.ArgumentParser:
    """Build the CLI argument parser."""

    parser = argparse.ArgumentParser(
        description="Migrate legacy SugarCube files into the current format."
    )
    parser.add_argument(
        "root",
        nargs="?",
        default=str(ROOT / "cubes"),
        help="Root directory to scan for .cube files",
    )
    return parser


def _list_cube_files(root: Path) -> list[Path]:
    """Return managed cube files while skipping history folders."""

    if not root.exists():
        return []
    cube_files: list[Path] = []
    for path in root.rglob("*.cube"):
        if not path.is_file():
            continue
        try:
            relative = path.relative_to(root)
        except ValueError:
            relative = None
        if relative is not None:
            parts = [part.lower() for part in relative.parts if part]
            if parts and parts[0] in {"old", "backup", "_old", "_history"}:
                continue
        cube_files.append(path.resolve())
    return sorted(cube_files)


def _read_json(path: Path) -> dict[str, Any]:
    """Read and validate one cube JSON payload."""

    with path.open("r", encoding="utf-8") as handle:
        payload = json.load(handle)
    if not isinstance(payload, dict):
        raise ValueError("Cube root must be a JSON object")
    return payload


def _replace_json(path: Path, payload: dict[str, Any]) -> None:
    """Write a migrated payload via a temporary file and atomic replace."""

    temp_path = path.with_suffix(f"{path.suffix}.tmp")
    with temp_path.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, indent=2)
        handle.write("\n")
    temp_path.replace(path)


def _write_stdout(message: str) -> None:
    """Write CLI output without using `print`."""

    sys.stdout.write(message)


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    raise SystemExit(main())

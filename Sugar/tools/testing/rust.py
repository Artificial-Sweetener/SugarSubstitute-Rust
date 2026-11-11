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
"""Resolve selected Rust proofs through explicit Cargo integration-test declarations."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import tomllib

from tools.architecture.scanner import visible_repository_paths


@dataclass(frozen=True)
class RustTestTarget:
    """Bind a selected proof to its declared Cargo package and public test target."""

    manifest: Path
    name: str
    runner: tuple[str, ...] = ("cargo", "test", "--locked")


def _test_runner(payload: object, manifest: Path) -> tuple[str, ...]:
    """Honor an explicit package-owned native environment runner without shell evaluation."""

    current: object = payload
    for key in ("package", "metadata", "sugarcubes"):
        if not isinstance(current, dict):
            return ("cargo", "test", "--locked")
        current = current.get(key)
    if not isinstance(current, dict) or "test-runner" not in current:
        return ("cargo", "test", "--locked")
    runner: object = current["test-runner"]
    if (
        not isinstance(runner, list)
        or not runner
        or not all(isinstance(argument, str) and argument for argument in runner)
    ):
        raise ValueError(
            f"Cargo proof runner requires nonempty argument strings: {manifest}"
        )
    return tuple(str(argument) for argument in runner)


def rust_test_targets(
    root: Path, files: tuple[Path, ...]
) -> tuple[RustTestTarget, ...]:
    """Reject missing, ambiguous or escaping owners before launching any Cargo test."""

    if not files:
        return ()
    root = root.resolve()
    selected = {file.resolve() for file in files}
    visible = visible_repository_paths(root)
    manifests = (
        tuple(root / path for path in visible if Path(path).name == "Cargo.toml")
        if visible is not None
        else tuple(root.rglob("Cargo.toml"))
    )
    owners: dict[Path, list[RustTestTarget]] = {file: [] for file in selected}
    for manifest in sorted(manifests):
        payload: object = tomllib.loads(manifest.read_text(encoding="utf-8"))
        if not isinstance(payload, dict):
            raise ValueError(f"Invalid Cargo manifest: {manifest}")
        runner = _test_runner(payload, manifest)
        tests: object = payload.get("test", [])
        if not isinstance(tests, list):
            raise ValueError(f"Cargo test declarations must be a list: {manifest}")
        for test in tests:
            if not isinstance(test, dict):
                raise ValueError(f"Invalid Cargo test declaration: {manifest}")
            name: object = test.get("name")
            path: object = test.get("path")
            if not isinstance(name, str) or not name or not isinstance(path, str):
                raise ValueError(
                    f"Cargo proofs require explicit names and paths: {manifest}"
                )
            file = (manifest.parent / path).resolve()
            if not file.is_relative_to(root):
                raise ValueError(f"Cargo proof escapes the repository: {manifest}")
            if file in owners:
                owners[file].append(RustTestTarget(manifest, name, runner))
    result: list[RustTestTarget] = []
    for file, candidates in sorted(owners.items()):
        if len(candidates) != 1:
            raise ValueError(f"Rust proof requires exactly one Cargo owner: {file}")
        result.append(candidates[0])
    return tuple(result)

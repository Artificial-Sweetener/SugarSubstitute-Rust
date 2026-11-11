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
"""Prove native proofs cannot disappear behind unknown extensions or ambiguous Cargo owners."""

from pathlib import Path

import pytest

from tools.testing.rust import rust_test_targets


@pytest.mark.parametrize("custom_runner", [False, True])
def test_resolves_explicit_cargo_target(tmp_path: Path, custom_runner: bool) -> None:
    """Select the declared public target while preserving the package toolchain directory."""

    package = tmp_path / "integration"
    package.mkdir()
    proof = tmp_path / "tests" / "native_test.rs"
    proof.parent.mkdir()
    proof.write_text("#[test] fn proof() {}", encoding="utf-8")
    manifest = package / "Cargo.toml"
    configuration = (
        '[package.metadata.sugarcubes]\ntest-runner=["node", "native.ts", "test"]\n'
        if custom_runner
        else ""
    )
    manifest.write_text(
        configuration
        + '[[test]]\nname="native_proof"\npath="../tests/native_test.rs"\n',
        encoding="utf-8",
    )
    targets = rust_test_targets(tmp_path, (proof,))
    assert [(target.manifest, target.name) for target in targets] == [
        (manifest, "native_proof")
    ]
    assert targets[0].runner == (
        ("node", "native.ts", "test")
        if custom_runner
        else ("cargo", "test", "--locked")
    )


@pytest.mark.parametrize("runner", ["[]", '["node", 42]', '[""]', '"cargo test"'])
def test_rejects_invalid_native_runner(tmp_path: Path, runner: str) -> None:
    """Require an argument list rather than a shell string or weakened package setup."""

    proof = tmp_path / "native_test.rs"
    proof.write_text("#[test] fn proof() {}", encoding="utf-8")
    (tmp_path / "Cargo.toml").write_text(
        f"[package.metadata.sugarcubes]\ntest-runner={runner}\n"
        '[[test]]\nname="proof"\npath="native_test.rs"\n',
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="runner requires"):
        rust_test_targets(tmp_path, (proof,))


@pytest.mark.parametrize("mode", ["missing", "ambiguous", "escape"])
def test_rejects_unowned_or_escaping_native_proofs(tmp_path: Path, mode: str) -> None:
    """Fail before execution when a Rust proof lacks one in-repository Cargo owner."""

    proof = tmp_path / "native_test.rs"
    proof.write_text("#[test] fn proof() {}", encoding="utf-8")
    if mode != "missing":
        path = "../escaped_test.rs" if mode == "escape" else "native_test.rs"
        declaration = f'[[test]]\nname="proof"\npath="{path}"\n'
        if mode == "ambiguous":
            declaration += '[[test]]\nname="other"\npath="native_test.rs"\n'
        (tmp_path / "Cargo.toml").write_text(declaration, encoding="utf-8")
    with pytest.raises(ValueError, match="escapes|exactly one"):
        rust_test_targets(tmp_path, (proof,))

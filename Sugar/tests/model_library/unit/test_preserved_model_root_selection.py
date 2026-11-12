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
"""Prove preservation selection reads real source records without changing any bytes."""

from __future__ import annotations

import hashlib
import json
import os
from typing import BinaryIO
from pathlib import Path

import pytest

from sugarcubes_model_roots import (
    MODEL_ROOT_API_VERSION,
    ModelRootSelectionError,
    ModelRootSelectionFailure,
    ModelRootSelectionSource,
    ModelRootStore,
)

SOURCES = tuple(ModelRootSelectionSource)
PRESERVED_RECORD_MAX_BYTES = 256 * 1024


def record_path(store: ModelRootStore, source: ModelRootSelectionSource) -> Path:
    """Name a source fixture using the owner's published current location."""
    return (
        store.config_path
        if source is ModelRootSelectionSource.CURRENT
        else store.config_path.with_name("managed_model_root.json")
    )


def encoded_selection(
    source: ModelRootSelectionSource, value: object, version: int = 1
) -> bytes:
    """Encode each supported historical spelling without invoking a production writer."""
    payload = (
        {"schemaVersion": version, "modelRoot": value}
        if source is ModelRootSelectionSource.CURRENT
        else {"schema_version": version, "model_root": value}
    )
    return json.dumps(payload, ensure_ascii=False).encode("utf-8")


def write_record(
    store: ModelRootStore, source: ModelRootSelectionSource, contents: bytes
) -> Path:
    """Arrange source bytes directly so a hidden production mkdir cannot be masked."""
    path = record_path(store, source)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(contents)
    return path


def snapshot(root: Path) -> dict[str, tuple[str, str]]:
    """Observe all names, directories and exact file hashes in the owned fixture tree."""
    return {
        path.relative_to(root).as_posix(): (
            ("directory", "")
            if path.is_dir()
            else ("file", hashlib.sha256(path.read_bytes()).hexdigest())
        )
        for path in root.rglob("*")
    }


def test_absence_uses_default_without_creating_an_installation(tmp_path: Path) -> None:
    """An absent source remains absent, including its default models directory."""
    base = tmp_path / "absent Comfy"
    store = ModelRootStore(base)
    before = snapshot(tmp_path)
    assert MODEL_ROOT_API_VERSION == 1
    assert store.read_preserved_selection() is None
    assert store.default_root == (base / "models").resolve()
    assert snapshot(tmp_path) == before
    assert not base.exists()


@pytest.mark.parametrize("source", SOURCES)
def test_valid_selection_does_not_create_or_migrate_any_path(
    tmp_path: Path, source: ModelRootSelectionSource, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Both codecs return intent while originals, unrelated files and missing targets stay untouched."""
    store = ModelRootStore(tmp_path / "Comfy")
    selected = tmp_path / "Shared 模型 missing"
    path = write_record(store, source, encoded_selection(source, str(selected)))
    (path.parent / "unrelated.bin").write_bytes(b"\x00unrelated\xff")
    before = snapshot(tmp_path)

    def forbid_save(model_root: Path | None) -> Path | None:
        """Fail if a preservation read delegates to an authoritative mutation."""
        raise AssertionError("A preservation read must not save")

    def forbid_migration() -> None:
        """Fail if the original legacy name could be retired."""
        raise AssertionError("A preservation read must not migrate")

    monkeypatch.setattr(store, "save", forbid_save)
    monkeypatch.setattr(store, "migrate_legacy", forbid_migration)
    assert store.read_preserved_selection() == selected.resolve()
    assert snapshot(tmp_path) == before
    assert not selected.exists()


@pytest.mark.parametrize("legacy", [b"{broken", b"[]", b'{"schema_version":900}'])
def test_valid_current_ignores_corrupt_or_unknown_legacy(
    tmp_path: Path, legacy: bytes
) -> None:
    """Current authority is not blocked by historical bytes that its branch never decodes."""
    store = ModelRootStore(tmp_path / "Comfy")
    selected = tmp_path / "current"
    write_record(
        store,
        ModelRootSelectionSource.CURRENT,
        encoded_selection(SOURCES[0], str(selected)),
    )
    write_record(store, ModelRootSelectionSource.LEGACY, legacy)
    before = snapshot(tmp_path)
    assert store.read_preserved_selection() == selected.resolve()
    assert snapshot(tmp_path) == before


def test_different_current_and_legacy_selections_resolve_to_current(
    tmp_path: Path,
) -> None:
    """Coexisting valid choices follow established precedence instead of rewriting either record."""
    store = ModelRootStore(tmp_path / "Comfy")
    current = tmp_path / "current"
    for source, selected in zip(SOURCES, (current, tmp_path / "legacy"), strict=True):
        write_record(store, source, encoded_selection(source, str(selected)))
    before = snapshot(tmp_path)
    assert store.read_preserved_selection() == current.resolve()
    assert snapshot(tmp_path) == before


@pytest.mark.parametrize("source", SOURCES)
def test_unknown_selected_schema_refuses_without_fallback_or_writes(
    tmp_path: Path, source: ModelRootSelectionSource
) -> None:
    """Unsupported selected data stays recoverable even when an older fallback is valid."""
    store = ModelRootStore(tmp_path / "Comfy")
    path = write_record(
        store, source, encoded_selection(source, str(tmp_path / "future"), 900)
    )
    if source is ModelRootSelectionSource.CURRENT:
        write_record(
            store, SOURCES[1], encoded_selection(SOURCES[1], str(tmp_path / "legacy"))
        )
    before = snapshot(tmp_path)
    with pytest.raises(ModelRootSelectionError) as failure:
        store.read_preserved_selection()
    assert failure.value.reason is ModelRootSelectionFailure.UNSUPPORTED_SCHEMA
    assert failure.value.source is source
    assert failure.value.path == path
    assert isinstance(failure.value.__cause__, ValueError)
    assert snapshot(tmp_path) == before


@pytest.mark.parametrize("source", SOURCES)
@pytest.mark.parametrize("contents", [b"{broken", b"[]", b"\xff"])
def test_corrupt_selected_bytes_preserve_the_original_cause_and_hashes(
    tmp_path: Path, source: ModelRootSelectionSource, contents: bytes
) -> None:
    """Malformed JSON, nonrecords and invalid UTF-8 are typed refusals, not absent selections."""
    store = ModelRootStore(tmp_path / "Comfy")
    write_record(store, source, contents)
    before = snapshot(tmp_path)
    with pytest.raises(ModelRootSelectionError) as failure:
        store.read_preserved_selection()
    assert failure.value.reason is ModelRootSelectionFailure.CORRUPT
    assert failure.value.source is source
    assert isinstance(failure.value.__cause__, ValueError)
    assert snapshot(tmp_path) == before


@pytest.mark.parametrize("source", SOURCES)
def test_excessively_nested_json_is_corrupt_instead_of_a_filesystem_conflict(
    tmp_path: Path, source: ModelRootSelectionSource
) -> None:
    """Preserve the decoder's recursion cause without mistaking it for a path failure."""
    store = ModelRootStore(tmp_path / "Comfy")
    write_record(store, source, b"[" * 10000 + b"0" + b"]" * 10000)
    before = snapshot(tmp_path)
    with pytest.raises(ModelRootSelectionError) as failure:
        store.read_preserved_selection()
    assert failure.value.reason is ModelRootSelectionFailure.CORRUPT
    assert failure.value.source is source
    assert isinstance(failure.value.__cause__, RecursionError)
    assert snapshot(tmp_path) == before


@pytest.mark.parametrize("source", SOURCES)
@pytest.mark.parametrize("value", [None, "", "relative/models", 42])
def test_invalid_root_values_share_the_authoritative_path_codec(
    tmp_path: Path, source: ModelRootSelectionSource, value: object
) -> None:
    """No historical spelling bypasses the existing absolute-directory selection rule."""
    store = ModelRootStore(tmp_path / "Comfy")
    write_record(store, source, encoded_selection(source, value))
    before = snapshot(tmp_path)
    with pytest.raises(ModelRootSelectionError) as failure:
        store.read_preserved_selection()
    assert failure.value.reason is ModelRootSelectionFailure.CORRUPT
    assert failure.value.source is source
    assert failure.value.__cause__ is not None
    assert snapshot(tmp_path) == before


@pytest.mark.parametrize("source", SOURCES)
def test_existing_file_cannot_become_a_model_root(
    tmp_path: Path, source: ModelRootSelectionSource
) -> None:
    """A selected directory/file collision is distinct from malformed JSON."""
    store = ModelRootStore(tmp_path / "Comfy")
    selected = tmp_path / "weights.bin"
    selected.write_bytes(b"authored weights")
    write_record(store, source, encoded_selection(source, str(selected)))
    before = snapshot(tmp_path)
    with pytest.raises(ModelRootSelectionError) as failure:
        store.read_preserved_selection()
    assert failure.value.reason is ModelRootSelectionFailure.CONFLICT
    assert failure.value.source is source
    assert snapshot(tmp_path) == before


@pytest.mark.parametrize("source", SOURCES)
def test_configuration_directory_collision_is_not_absence(
    tmp_path: Path, source: ModelRootSelectionSource
) -> None:
    """Do not fall back when a selected record name is occupied by a directory."""
    store = ModelRootStore(tmp_path / "Comfy")
    record_path(store, source).mkdir(parents=True)
    before = snapshot(tmp_path)
    with pytest.raises(ModelRootSelectionError) as failure:
        store.read_preserved_selection()
    assert failure.value.reason is ModelRootSelectionFailure.CONFLICT
    assert failure.value.source is source
    assert snapshot(tmp_path) == before


def test_configuration_namespace_file_is_a_conflict(tmp_path: Path) -> None:
    """Windows path-not-found spelling must not disguise a colliding namespace file."""
    store = ModelRootStore(tmp_path)
    store.config_path.parent.write_bytes(b"unrelated namespace occupant")
    before = snapshot(tmp_path)
    with pytest.raises(ModelRootSelectionError) as failure:
        store.read_preserved_selection()
    assert failure.value.reason is ModelRootSelectionFailure.CONFLICT
    assert failure.value.source is ModelRootSelectionSource.CURRENT
    assert snapshot(tmp_path) == before


@pytest.mark.parametrize("source", SOURCES)
def test_read_failure_remains_typed_and_retains_its_cause(
    tmp_path: Path, source: ModelRootSelectionSource, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Substitute only a denied IO boundary while the original fixture bytes remain real."""
    store = ModelRootStore(tmp_path / "Comfy")
    path = write_record(
        store, source, encoded_selection(source, str(tmp_path / "models"))
    )
    before = snapshot(tmp_path)
    denied = PermissionError("Fixture denies the selected read")

    def deny_open(candidate: Path, mode: str = "r") -> BinaryIO:
        """Raise the exact external denial only for the selected record."""
        assert candidate == path
        assert mode == "rb"
        raise denied

    with monkeypatch.context() as context:
        context.setattr(Path, "open", deny_open)
        with pytest.raises(ModelRootSelectionError) as failure:
            store.read_preserved_selection()
    assert failure.value.reason is ModelRootSelectionFailure.UNREADABLE
    assert failure.value.source is source
    assert failure.value.__cause__ is denied
    assert snapshot(tmp_path) == before


@pytest.mark.parametrize("source", SOURCES)
def test_record_at_byte_limit_remains_supported_and_untouched(
    tmp_path: Path, source: ModelRootSelectionSource
) -> None:
    """Accept the inclusive byte boundary without creating the selected root."""
    store = ModelRootStore(tmp_path / "Comfy")
    selected = tmp_path / "模型 missing"
    encoded = encoded_selection(source, str(selected))
    contents = encoded + b" " * (PRESERVED_RECORD_MAX_BYTES - len(encoded))
    write_record(store, source, contents)
    before = snapshot(tmp_path)
    assert store.read_preserved_selection() == selected.resolve()
    assert snapshot(tmp_path) == before
    assert not selected.exists()


@pytest.mark.parametrize("source", SOURCES)
@pytest.mark.parametrize("suffix", [b" ", b"\xff"])
def test_oversize_selected_record_is_rejected_before_decoding(
    tmp_path: Path, source: ModelRootSelectionSource, suffix: bytes
) -> None:
    """Bound malformed and valid JSON alike before UTF-8 or JSON allocation."""
    store = ModelRootStore(tmp_path / "Comfy")
    encoded = encoded_selection(source, str(tmp_path / "models"))
    contents = encoded + b" " * (PRESERVED_RECORD_MAX_BYTES - len(encoded)) + suffix
    write_record(store, source, contents)
    before = snapshot(tmp_path)
    with pytest.raises(ModelRootSelectionError) as failure:
        store.read_preserved_selection()
    assert failure.value.reason is ModelRootSelectionFailure.CORRUPT
    assert failure.value.source is source
    assert isinstance(failure.value.__cause__, ValueError)
    assert str(failure.value.__cause__) == (
        "Preserved model-root configuration exceeds 256 KiB."
    )
    assert snapshot(tmp_path) == before


@pytest.mark.parametrize("source", SOURCES)
def test_record_growth_after_inspection_cannot_bypass_the_read_bound(
    tmp_path: Path, source: ModelRootSelectionSource, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Simulate an external append after stat while preserving every resulting source byte."""
    store = ModelRootStore(tmp_path / "Comfy")
    encoded = encoded_selection(source, str(tmp_path / "models"))
    path = write_record(store, source, encoded)
    appended = b" " * (PRESERVED_RECORD_MAX_BYTES + 1 - len(encoded))
    original_stat = Path.stat
    grew = False

    def stat_then_grow(
        candidate: Path, *, follow_symlinks: bool = True
    ) -> os.stat_result:
        """Return the old metadata after an external writer has enlarged the real file."""
        nonlocal grew
        result = original_stat(candidate, follow_symlinks=follow_symlinks)
        if candidate == path and not grew:
            with path.open("ab") as writer:
                writer.write(appended)
            grew = True
        return result

    before = snapshot(tmp_path)
    before[path.relative_to(tmp_path).as_posix()] = (
        "file",
        hashlib.sha256(encoded + appended).hexdigest(),
    )
    with monkeypatch.context() as context:
        context.setattr(Path, "stat", stat_then_grow)
        with pytest.raises(ModelRootSelectionError) as failure:
            store.read_preserved_selection()
    assert grew
    assert failure.value.reason is ModelRootSelectionFailure.CORRUPT
    assert failure.value.source is source
    assert str(failure.value.__cause__) == (
        "Preserved model-root configuration exceeds 256 KiB."
    )
    assert snapshot(tmp_path) == before

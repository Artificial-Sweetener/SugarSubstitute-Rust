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
"""Prove output insertion commits once after native decoding and destination validation."""

from __future__ import annotations

from dataclasses import replace
from pathlib import Path
from threading import Event
import tempfile

import pytest

from integrations.gimp.native.files import NativePngFiles
from integrations.gimp.native.images import describe_image
from integrations.gimp.native.model import (
    NativeOperationCancelled,
    NativeOperationUnsupported,
    NativePng,
    StaleNativeImage,
)
from integrations.gimp.native.outputs import NativeOutputs
from tests.gimp.support.native_outputs import (
    FakeGioApi,
    FakeOutputApi,
    FakeOutputImage,
    FakeOutputLookup,
)

PNG = NativePng(b"admitted PNG fixture", 3, 2)


def fixture(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> tuple[
    FakeOutputImage, FakeOutputApi, NativePngFiles, NativeOutputs, list[Exception]
]:
    """Compose production native owners without replacing their filesystem lifecycle."""
    monkeypatch.setattr(tempfile, "tempdir", str(tmp_path))
    image = FakeOutputImage()
    api = FakeOutputApi(FakeOutputLookup(image))
    errors: list[Exception] = []
    files = NativePngFiles(FakeGioApi(), 64, errors.append)
    return image, api, files, NativeOutputs(api, files), errors


def test_output_is_prepared_off_image_and_commits_exact_shared_coordinates(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Preserve native canvas geometry and intrinsic output pixels without native centering policy."""
    image, api, files, output, errors = fixture(tmp_path, monkeypatch)
    document = describe_image(image)
    result = output.insert(document, PNG, "<literal layer>", -17, 23, lambda: False)
    layer = api.decoded[0]
    assert (result.tattoo, result.left, result.top, result.width, result.height) == (
        123,
        -17,
        23,
        3,
        2,
    )
    assert (layer.name, layer.left, layer.top) == ("<literal layer>", -17, 23)
    assert image.inserted == [layer]
    assert api.loaded == [PNG.data]
    assert describe_image(image) == document
    assert list(tmp_path.iterdir()) == []
    output.close()
    files.close()
    assert not layer.deleted
    assert errors == []


@pytest.mark.parametrize(
    "fault", ["closed", "geometry", "forged-size", "unsupported", "cancelled"]
)
def test_unavailable_destinations_do_not_allocate_decoder_files_or_layers(
    fault: str, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Reject invalid prerequisites before touching the native document."""
    image, api, files, output, _ = fixture(tmp_path, monkeypatch)
    document = describe_image(image)
    expected: type[Exception] = StaleNativeImage
    if fault == "closed":
        image.valid = False
    elif fault == "geometry":
        image.width += 1
    elif fault == "forged-size":
        document = replace(document, width=document.width + 1)
    elif fault == "unsupported":
        image.base_type = 1
        document = describe_image(image)
        expected = NativeOperationUnsupported
    else:
        expected = NativeOperationCancelled
    with pytest.raises(expected):
        output.insert(document, PNG, "Output", 0, 0, lambda: fault == "cancelled")
    assert api.loaded == []
    assert image.inserted == []
    assert list(tmp_path.iterdir()) == []
    output.close()
    files.close()


@pytest.mark.parametrize(
    "fault", ["dimensions", "prepare", "geometry", "cancelled", "insert"]
)
def test_precommit_failure_deletes_the_prepared_layer_without_mutating_the_target(
    fault: str, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Contain failures and concurrent changes at every native preparation boundary."""
    image, api, files, output, _ = fixture(tmp_path, monkeypatch)
    document = describe_image(image)
    cancellation = Event()
    expected: type[Exception] = RuntimeError
    if fault == "dimensions":
        api.width += 1
    elif fault == "prepare":
        api.fail_prepare = True
    elif fault == "geometry":
        api.on_load = lambda: setattr(image, "width", image.width + 1)
        expected = StaleNativeImage
    elif fault == "cancelled":
        api.on_load = cancellation.set
        expected = NativeOperationCancelled
    else:
        image.fail_insert = True
    with pytest.raises(expected):
        output.insert(document, PNG, "Output", 0, 0, cancellation.is_set)
    assert image.inserted == []
    assert api.decoded[0].deleted
    assert list(tmp_path.iterdir()) == []
    output.close()
    files.close()


def test_cancellation_after_atomic_commit_does_not_delete_a_committed_user_layer(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep the mutation boundary authoritative when cancellation arrives too late."""
    image, api, files, output, _ = fixture(tmp_path, monkeypatch)
    cancellation = Event()
    image.on_insert = cancellation.set
    result = output.insert(
        describe_image(image), PNG, "Output", 0, 0, cancellation.is_set
    )
    assert result.tattoo == 123
    assert cancellation.is_set()
    output.close()
    files.close()
    assert len(image.inserted) == 1
    assert not api.decoded[0].deleted


def test_failed_native_cleanup_remains_owned_and_blocks_new_allocations(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Retain cleanup state after a failed delete instead of leaking an unattached layer."""
    image, api, files, output, _ = fixture(tmp_path, monkeypatch)
    api.width += 1
    api.fail_delete = True
    with pytest.raises(RuntimeError, match="release the prepared") as failed:
        output.insert(describe_image(image), PNG, "Output", 0, 0, lambda: False)
    assert failed.value.__context__ is not None
    assert "dimensions" in str(failed.value.__context__)
    with pytest.raises(RuntimeError, match="requires cleanup"):
        output.insert(describe_image(image), PNG, "Output", 0, 0, lambda: False)
    layer = api.decoded[0]
    layer.fail_delete = False
    output.close()
    files.close()
    assert layer.delete_calls == 2
    assert layer.deleted
    assert image.inserted == []


def test_postcommit_file_cleanup_failure_reports_without_losing_the_insert_result(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep completed mutation results authoritative while retaining failed temporary cleanup."""
    image, api, files, output, errors = fixture(tmp_path, monkeypatch)
    unlink = Path.unlink

    def locked(path: Path, missing_ok: bool = False) -> None:
        """Simulate a native decoder holding its input file temporarily."""
        if path.name.startswith("sugarcubes-gimp-"):
            raise PermissionError("Native input file is still locked.")
        unlink(path, missing_ok=missing_ok)

    monkeypatch.setattr(Path, "unlink", locked)
    assert (
        output.insert(describe_image(image), PNG, "Output", 0, 0, lambda: False).tattoo
        == 123
    )
    assert len(errors) == 1
    assert len(list(tmp_path.iterdir())) == 1
    monkeypatch.setattr(Path, "unlink", unlink)
    files.close()
    output.close()
    assert list(tmp_path.iterdir()) == []
    assert not api.decoded[0].deleted

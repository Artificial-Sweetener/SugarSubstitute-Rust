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
"""Prove native decoder files remain bounded and owned through cleanup failures."""

from __future__ import annotations

from pathlib import Path
import tempfile

import pytest

from integrations.gimp.native.files import NativePngFiles
from tests.gimp.support.native_outputs import FakeGioApi


def test_decoder_files_are_closed_before_use_and_removed_on_decoder_failure(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep Windows decoder access valid and release the owned path on failure."""
    monkeypatch.setattr(tempfile, "tempdir", str(tmp_path))
    files = NativePngFiles(FakeGioApi(), 3, lambda error: pytest.fail(str(error)))
    with pytest.raises(RuntimeError, match="decoder"):
        with files.png(b"png") as file:
            path = file.get_path()
            assert path is not None
            assert Path(path).read_bytes() == b"png"
            Path(path).write_bytes(b"PNG")
            raise RuntimeError("decoder failed")
    assert list(tmp_path.iterdir()) == []
    for data in (b"", b"oversized"):
        with pytest.raises(ValueError, match="budget"):
            with files.png(data):
                pytest.fail("Invalid input was materialized.")
    files.close()
    with pytest.raises(RuntimeError, match="closed"):
        with files.png(b"png"):
            pytest.fail("Closed native file owner allocated an input.")


def test_failed_file_deletion_stays_owned_and_bounds_further_allocations(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Attempt cleanup again without allowing failed paths to accumulate without a limit."""
    monkeypatch.setattr(tempfile, "tempdir", str(tmp_path))
    errors: list[Exception] = []
    files = NativePngFiles(FakeGioApi(), 3, errors.append, maximum_live=1)
    unlink = Path.unlink

    def locked(path: Path, missing_ok: bool = False) -> None:
        """Retain only this test's decoder input."""
        raise PermissionError("File locked.")

    monkeypatch.setattr(Path, "unlink", locked)
    with files.png(b"png"):
        pass
    assert len(errors) == 1
    with pytest.raises(RuntimeError, match="cleanup"):
        with files.png(b"png"):
            pytest.fail("The native temporary-file limit was bypassed.")
    with pytest.raises(ExceptionGroup, match="cleanup"):
        files.close()
    monkeypatch.setattr(Path, "unlink", unlink)
    files.close()
    assert list(tmp_path.iterdir()) == []

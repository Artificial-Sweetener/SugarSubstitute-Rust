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
"""Prove geometry admission and mutation against the native SDK boundary."""

from __future__ import annotations

from pathlib import Path

import pytest

from integrations.gimp.native.contract import fields
from integrations.gimp.native.model import NativeOperationCancelled, StaleNativeImage
from tests.gimp.support.native_session import native_session


def test_native_selection_bounds_preserve_document_and_selected_layers(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Return exclusive selection bounds without changing native editing state."""
    fixture = native_session(tmp_path, monkeypatch)
    try:
        document = fixture.call(1, {"type": "context"})["document"]
        selected = list(fixture.image.selected)
        fixture.geometry.Selection.bounds_value = (True, True, 1, 2, 7, 5)
        result = fixture.call(2, {"type": "geometry.context", "document": document})
        assert result == {
            "kind": "geometry",
            "document": document,
            "selection": {"left": 1, "top": 2, "width": 6, "height": 3},
        }
        fixture.geometry.Selection.bounds_value = (True, False, 0, 0, 8, 6)
        assert (
            fixture.call(3, {"type": "geometry.context", "document": document})[
                "selection"
            ]
            is None
        )
        assert fixture.image.selected == selected
        assert fixture.geometry.Image.calls == []
    finally:
        fixture.close()


def test_native_resize_is_one_centered_mutation_and_rejects_the_old_frame(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep pixels and selected layers while replacing only the captured canvas bounds."""
    fixture = native_session(tmp_path, monkeypatch)
    try:
        document = fixture.call(1, {"type": "context"})["document"]
        selected, pixels = list(fixture.image.selected), fixture.image.projection
        command = {
            "type": "image.resize",
            "document": document,
            "width": 13,
            "height": 9,
        }
        result = fixture.call(2, command)
        assert result["kind"] == "resized" and result["committed"] is True
        assert fields(result["document"])["width"] == 13
        assert fixture.geometry.Image.calls == [(13, 9, 2, 1)]
        assert fixture.image.selected == selected and fixture.image.projection == pixels
        with pytest.raises(StaleNativeImage):
            fixture.call(3, command)
        assert len(fixture.geometry.Image.calls) == 1
    finally:
        fixture.close()


def test_native_resize_cancellation_and_sdk_failure_do_not_report_success(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Stop before mutation on cancellation and preserve actionable native failure."""
    fixture = native_session(tmp_path, monkeypatch)
    try:
        document = fixture.call(1, {"type": "context"})["document"]
        command = {
            "type": "image.resize",
            "document": document,
            "width": 4,
            "height": 3,
        }
        with pytest.raises(NativeOperationCancelled):
            fixture.dispatch.execute(2, command, lambda: True)
        assert fixture.geometry.Image.calls == []
        fixture.geometry.Image.fail = True
        with pytest.raises(RuntimeError, match="could not resize"):
            fixture.call(3, command)
        assert fixture.image.width == 8 and fixture.image.height == 6
    finally:
        fixture.close()


def test_native_geometry_uses_canvas_limits_independently_of_png_transfers(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Permit wide canvas bounds and selection positions through the real compiled contract."""
    fixture = native_session(tmp_path, monkeypatch)
    try:
        document = fixture.call(1, {"type": "context"})["document"]
        resized = fixture.call(
            2,
            {"type": "image.resize", "document": document, "width": 50000, "height": 1},
        )
        fixture.geometry.Selection.bounds_value = (True, True, 40000, 0, 45000, 1)
        geometry = fixture.call(
            3, {"type": "geometry.context", "document": resized["document"]}
        )
        assert geometry["selection"] == {
            "left": 40000,
            "top": 0,
            "width": 5000,
            "height": 1,
        }
        assert len(fixture.geometry.Image.calls) == 1
        with pytest.raises(ValueError):
            fixture.call(
                4,
                {
                    "type": "image.resize",
                    "document": resized["document"],
                    "width": 300001,
                    "height": 1,
                },
            )
        assert len(fixture.geometry.Image.calls) == 1
    finally:
        fixture.close()


@pytest.mark.parametrize(
    "bounds", [(False, True, 0, 0, 2, 2), (True, True, 0, 0, 99, 2)]
)
def test_native_geometry_rejects_failed_or_incoherent_selection(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    bounds: tuple[bool, bool, int, int, int, int],
) -> None:
    """Fail closed instead of providing invalid selection dimensions to shared sizing."""
    fixture = native_session(tmp_path, monkeypatch)
    try:
        document = fixture.call(1, {"type": "context"})["document"]
        fixture.geometry.Selection.bounds_value = bounds
        with pytest.raises(RuntimeError):
            fixture.call(2, {"type": "geometry.context", "document": document})
        assert fixture.geometry.Image.calls == []
    finally:
        fixture.close()

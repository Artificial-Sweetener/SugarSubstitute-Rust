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
"""Prove private GIMP primitives preserve shared geometry and native resource ownership."""

from __future__ import annotations

from base64 import b64decode, b64encode
from pathlib import Path
from threading import Event

import pytest

from integrations.gimp.native.contract import fields, text
from integrations.gimp.native.model import NativeOperationCancelled, StaleNativeImage
from tests.gimp.support.native_session import native_session


def test_private_snapshot_wire_captures_frozen_pixels_and_releases_only_owned_images(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Use production codecs and dispatch while mocking only the native SDK."""
    fixture = native_session(tmp_path, monkeypatch)
    try:
        context = fixture.call(1, {"type": "context"})
        assert context["primaryLayer"] == 31
        assert context["rgb"] is True
        snapshot = fixture.call(
            2, {"type": "snapshot.begin", "document": context["document"]}
        )
        fixture.image.projection = b"edited"
        pixels = fixture.call(
            3,
            {
                "type": "snapshot.projection",
                "token": snapshot["token"],
                "visibility": [],
                "width": 8,
                "height": 6,
                "scale": 1,
            },
        )
        assert b64decode(text(pixels["data"])) == bytes([10, 200, 30, 255]) * 48
        assert fixture.dispatch.retained_requests == {2}
        fixture.call(4, {"type": "snapshot.release", "token": snapshot["token"]})
        assert fixture.dispatch.retained_requests == set()
        assert not fixture.image.copies[0].valid
        assert fixture.image.valid
    finally:
        fixture.close()


def test_closed_launch_image_is_not_replaced_by_another_image(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep document binding explicit instead of guessing the active image from inventory order."""
    fixture = native_session(tmp_path, monkeypatch)
    try:
        context = fixture.call(1, {"type": "context"})
        document = fields(context["document"])
        with pytest.raises(StaleNativeImage, match="launch image"):
            fixture.call(
                2, {"type": "snapshot.begin", "document": {**document, "imageId": 99}}
            )
        assert fixture.image.copies == []
        fixture.image.valid = False
        assert fixture.call(3, {"type": "context"}) == {
            "kind": "context",
            "hostId": "fixture-host",
            "document": None,
            "layers": [],
            "primaryLayer": None,
            "rgb": False,
        }
    finally:
        fixture.close()


def test_lost_snapshot_reply_can_be_cancelled_after_native_completion(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Retain request-to-snapshot ownership until cancellation or explicit release succeeds."""
    fixture = native_session(tmp_path, monkeypatch)
    try:
        context = fixture.call(1, {"type": "context"})
        fixture.call(2, {"type": "snapshot.begin", "document": context["document"]})
        fixture.dispatch.cancel(2)
        fixture.dispatch.cancel(2)
        assert not fixture.image.copies[0].valid
        assert fixture.dispatch.retained_requests == set()
    finally:
        fixture.close()


def test_cancellation_during_snapshot_admission_releases_the_duplicate(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Prevent native resources from leaking when the viewer leaves before receiving its token."""
    fixture = native_session(tmp_path, monkeypatch)
    try:
        context = fixture.call(1, {"type": "context"})
        cancelled = Event()
        fixture.image.on_duplicate = lambda image: cancelled.set()
        with pytest.raises(NativeOperationCancelled):
            fixture.dispatch.execute(
                2,
                {"type": "snapshot.begin", "document": context["document"]},
                cancelled.is_set,
            )
        assert not fixture.image.copies[0].valid
        assert fixture.dispatch.retained_requests == set()
    finally:
        fixture.close()


def test_native_insertion_wire_keeps_precomputed_placement_and_literal_name(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Apply prepared bytes without reimplementing the shared centering and placement policy."""
    fixture = native_session(tmp_path, monkeypatch)
    try:
        context = fixture.call(1, {"type": "context"})
        result = fixture.call(
            2,
            {
                "type": "image.insert",
                "document": context["document"],
                "data": b64encode(b"shared-admitted png").decode("ascii"),
                "width": 3,
                "height": 2,
                "name": "<literal layer>",
                "left": -17,
                "top": 23,
            },
        )
        assert result == {
            "kind": "inserted",
            "committed": True,
            "tattoo": 123,
            "left": -17,
            "top": 23,
            "width": 3,
            "height": 2,
        }
        assert fixture.output.decoded[0].name == "<literal layer>"
        assert list(tmp_path.iterdir()) == []
    finally:
        fixture.close()


def test_native_labels_do_not_split_astral_characters(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Return presentation labels that both native and companion validators accept."""
    fixture = native_session(tmp_path, monkeypatch)
    try:
        fixture.image.name = "🧊" * 140
        assert (
            fields(fixture.call(1, {"type": "context"})["document"])["name"]
            == "🧊" * 128
        )
    finally:
        fixture.close()

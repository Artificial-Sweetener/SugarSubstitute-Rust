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
"""Prove native mask framing preserves commit authority and fresh coherent coverage."""

from __future__ import annotations

from pathlib import Path

import pytest

from integrations.gimp.native.contract import fields, integer, text
from tests.gimp.support.native_requests import NativeRequestFixture
from tests.gimp.support.native_session import native_session


def test_native_mask_wire_captures_saved_channels_without_changing_the_selection(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Traverse actual native frame admission, dispatch, channel storage and snapshot release."""
    fixture = native_session(tmp_path, monkeypatch)
    try:
        context = fixture.call(1, {"type": "context"})
        created = fixture.call(
            2,
            {
                "type": "channel.create",
                "document": context["document"],
                "name": "<literal mask>",
                "source": "selection",
                "data": None,
            },
        )
        assert created["kind"] == "channel-created" and created["committed"] is True
        selected = fixture.call(
            3,
            {
                "type": "channel.select",
                "document": context["document"],
                "tattoo": created["tattoo"],
            },
        )
        assert selected["committed"] is True
        frozen = fixture.call(
            4, {"type": "snapshot.begin", "document": context["document"]}
        )
        capture = fixture.call(
            5,
            {
                "type": "snapshot.channel",
                "token": frozen["token"],
                "tattoo": created["tattoo"],
                "width": 8,
                "height": 6,
                "scale": 1,
            },
        )
        assert capture["kind"] == "pixels" and capture["format"] == "coverage8"
        fixture.call(6, {"type": "snapshot.release", "token": frozen["token"]})
        fixture.call(7, {"type": "channel.release", "tattoo": None})
        assert fixture.image.selection.buffer.data == bytes([127]) * 48
        assert not fixture.image.selected_channels
        assert len(fixture.image.inserted_channels) == 1
    finally:
        fixture.close()


def test_transport_cancellation_does_not_deny_a_committed_authored_channel(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Return the native channel identity when cancellation arrives at insertion's commit point."""
    fixture = NativeRequestFixture(native_session(tmp_path, monkeypatch))
    try:
        fixture.receive(1, {"type": "context"})
        fixture.drain()
        fixture.session.image.on_channel_insert = lambda: fixture.cancel(2)
        fixture.receive(
            2,
            {
                "type": "channel.create",
                "document": fixture.result(1)["document"],
                "name": "Region",
                "source": "blank",
                "data": None,
            },
        )
        fixture.drain()
        result = fixture.result(2)
        assert result["kind"] == "channel-created" and result["committed"] is True
        assert len(fixture.session.image.inserted_channels) == 1
        assert integer(result["tattoo"]) > 0
        assert text(fields(fixture.result(1)["document"])["name"])
    finally:
        fixture.stop()

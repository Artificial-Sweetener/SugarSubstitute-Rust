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
"""Prove native IPC admission never calls GIMP off-thread or replays a mutation."""

from __future__ import annotations

from base64 import b64encode
from pathlib import Path
from threading import Thread

import pytest

from integrations.gimp.native.contract import fields
from integrations.gimp.native.image_api import NativeImage
from tests.gimp.support.native_requests import NativeRequestFixture
from tests.gimp.support.native_session import native_session
from tests.support.no_progress import join_thread


def test_reader_admission_schedules_all_sdk_work_on_the_explicit_main_loop(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Receive on a background thread while keeping native metadata access on the loop."""
    fixture = NativeRequestFixture(native_session(tmp_path, monkeypatch))
    try:
        reader = Thread(target=fixture.receive, args=(1, {"type": "context"}))
        reader.start()
        join_thread(reader)
        assert not reader.is_alive()
        assert fixture.session.image.width_reads == 0
        assert len(fixture.callbacks) == 1
        fixture.drain()
        assert fixture.result(1)["kind"] == "context"
        assert fixture.session.image.width_reads > 0
        assert fixture.session.errors == []
    finally:
        fixture.stop()


def test_cancelled_queued_capture_does_not_allocate_a_native_snapshot(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Expose cancellation before its queued callback can touch the SDK."""
    fixture = NativeRequestFixture(native_session(tmp_path, monkeypatch))
    try:
        fixture.receive(1, {"type": "context"})
        fixture.drain()
        fixture.receive(
            2, {"type": "snapshot.begin", "document": fixture.result(1)["document"]}
        )
        fixture.cancel(2)
        fixture.drain()
        assert fixture.result(2)["status"] == "cancelled"
        assert fixture.session.image.copies == []
    finally:
        fixture.stop()


def test_cancelled_snapshot_reply_keeps_late_cleanup_addressable(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Cancel an already-created batch when its response did not reach the viewer."""
    fixture = NativeRequestFixture(native_session(tmp_path, monkeypatch))
    try:
        fixture.receive(1, {"type": "context"})
        fixture.drain()
        fixture.receive(
            2, {"type": "snapshot.begin", "document": fixture.result(1)["document"]}
        )
        fixture.drain()
        assert fixture.result(2)["kind"] == "snapshot"
        fixture.cancel(2)
        fixture.cancel(2)
        assert len(fixture.callbacks) == 1
        fixture.drain()
        assert not fixture.session.image.copies[0].valid
        assert fixture.session.dispatch.retained_requests == set()
    finally:
        fixture.stop()


@pytest.mark.parametrize("when", ["precommit", "committed"])
def test_native_cancel_obeys_the_single_insertion_commit_point(
    when: str, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Release decoded layers before insertion and preserve successful commits after insertion."""
    fixture = NativeRequestFixture(native_session(tmp_path, monkeypatch))
    try:
        fixture.receive(1, {"type": "context"})
        fixture.drain()
        if when == "precommit":
            fixture.session.output.on_load = lambda: fixture.cancel(2)
        else:
            fixture.session.image.on_insert = lambda: fixture.cancel(2)
        fixture.receive(
            2,
            {
                "type": "image.insert",
                "document": fixture.result(1)["document"],
                "data": b64encode(b"admitted PNG").decode("ascii"),
                "width": 3,
                "height": 2,
                "name": "Result",
                "left": -1,
                "top": 2,
            },
        )
        fixture.drain()
        assert fixture.result(2)["kind"] == (
            "failure" if when == "precommit" else "inserted"
        )
        assert len(fixture.session.image.inserted) == (0 if when == "precommit" else 1)
        assert fixture.session.output.decoded[0].deleted == (when == "precommit")
        assert list(tmp_path.iterdir()) == []
    finally:
        fixture.stop()


def test_request_admission_is_bounded_and_does_not_reenter_the_native_loop(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Refuse excess queued work while retaining one scheduled callback for admitted operations."""
    fixture = NativeRequestFixture(native_session(tmp_path, monkeypatch))
    try:
        fixture.receive(1, {"type": "context"})
        fixture.receive(2, {"type": "context"})
        fixture.receive(3, {"type": "context"})
        assert fixture.result(3)["kind"] == "failure"
        assert len(fixture.callbacks) == 1
        fixture.drain()
        assert fixture.result(1)["kind"] == "context"
        assert fixture.result(2)["kind"] == "context"
        fixture.receive(4, {"type": "context"})
        fixture.drain()
        assert fixture.result(4)["kind"] == "context"
    finally:
        fixture.stop()


def test_native_request_identities_cannot_be_replayed_or_reordered(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Reject forgotten identities rather than reexecuting a mutation after transport loss."""
    fixture = NativeRequestFixture(native_session(tmp_path, monkeypatch))
    try:
        fixture.receive(2, {"type": "context"})
        fixture.drain()
        for request_id in (1, 2):
            with pytest.raises(ValueError, match="identity"):
                fixture.receive(request_id, {"type": "context"})
        assert len(fixture.frames) == 1
    finally:
        fixture.stop()


def test_native_shutdown_cancels_pending_work_and_releases_completed_snapshots(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Release offscreen resources without sending replies into a retired viewer lifetime."""
    fixture = NativeRequestFixture(native_session(tmp_path, monkeypatch))
    fixture.receive(1, {"type": "context"})
    fixture.drain()
    fixture.receive(
        2, {"type": "snapshot.begin", "document": fixture.result(1)["document"]}
    )
    fixture.drain()
    fixture.receive(3, {"type": "context"})
    fixture.stop()
    fixture.stop()
    assert fixture.closed == [True]
    assert len(fixture.frames) == 2
    assert not fixture.session.image.copies[0].valid
    assert fixture.session.image.valid


def test_native_failures_are_reported_without_exposing_internal_details(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep exception evidence in diagnostics and stable bounded error text on the private pipe."""
    fixture = NativeRequestFixture(native_session(tmp_path, monkeypatch))
    try:
        fixture.receive(1, {"type": "context"})
        fixture.drain()
        document = fields(fixture.result(1)["document"])
        fixture.receive(
            2, {"type": "snapshot.begin", "document": {**document, "width": 99}}
        )
        fixture.drain()
        assert fixture.result(2)["status"] == "stale"
        fixture.receive(
            3,
            {
                "type": "image.insert",
                "document": document,
                "data": "%%%",
                "width": 3,
                "height": 2,
                "name": "Result",
                "left": 0,
                "top": 0,
            },
        )
        fixture.drain()
        assert fixture.result(3) == {
            "kind": "failure",
            "status": "error",
            "message": "The GIMP native operation failed.",
        }
        assert len(fixture.session.errors) == 1
        assert fixture.session.output.decoded == []
    finally:
        fixture.stop()


@pytest.mark.parametrize("when", ["queued", "committed"])
def test_canvas_resize_cancellation_preserves_the_native_commit_boundary(
    when: str, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Cancel queued sizing without a mutation and retain a committed resize without replay."""
    fixture = NativeRequestFixture(native_session(tmp_path, monkeypatch))
    try:
        fixture.receive(1, {"type": "context"})
        fixture.drain()
        geometry = fixture.session.geometry.Image
        resize = geometry.resize

        def commit_then_cancel(
            image: NativeImage, width: int, height: int, left: int, top: int
        ) -> bool:
            """Deliver transport cancellation immediately after the external SDK mutation."""
            success = resize(image, width, height, left, top)
            fixture.cancel(2)
            return success

        if when == "committed":
            monkeypatch.setattr(geometry, "resize", commit_then_cancel)
        command = {
            "type": "image.resize",
            "document": fixture.result(1)["document"],
            "width": 13,
            "height": 9,
        }
        fixture.receive(2, command)
        if when == "queued":
            fixture.cancel(2)
        fixture.drain()
        if when == "queued":
            assert fixture.result(2)["status"] == "cancelled"
            assert geometry.calls == []
            assert (fixture.session.image.width, fixture.session.image.height) == (8, 6)
        else:
            assert fixture.result(2)["kind"] == "resized"
            assert fixture.result(2)["committed"] is True
            assert geometry.calls == [(13, 9, 2, 1)]
            assert (fixture.session.image.width, fixture.session.image.height) == (
                13,
                9,
            )
        with pytest.raises(ValueError, match="identity"):
            fixture.receive(2, command)
        assert len(geometry.calls) == (0 if when == "queued" else 1)
        assert fixture.session.errors == []
    finally:
        fixture.stop()

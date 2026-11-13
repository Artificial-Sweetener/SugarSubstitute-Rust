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
"""Exercise actual anonymous pipes while keeping native frame IO isolated from SDK work."""

from __future__ import annotations

from contextlib import ExitStack
import os
from threading import Event

import pytest

from integrations.gimp.native.streams import NativePipe
from tests.support.no_progress import wait_for_event


def test_native_pipe_reads_and_writes_complete_frames_and_retires_on_eof() -> None:
    """Use operating-system pipes to prove bounded framing and normal peer shutdown."""
    with ExitStack() as stack:
        inbound_read, inbound_write = os.pipe()
        outbound_read, outbound_write = os.pipe()
        incoming = stack.enter_context(os.fdopen(inbound_read, "rb", buffering=0))
        peer_write = stack.enter_context(os.fdopen(inbound_write, "wb", buffering=0))
        peer_read = stack.enter_context(os.fdopen(outbound_read, "rb", buffering=0))
        outgoing = stack.enter_context(os.fdopen(outbound_write, "wb", buffering=0))
        frames: list[bytes] = []
        errors: list[Exception] = []
        received, closed = Event(), Event()

        def receive(frame: bytes) -> None:
            """Record only byte transport, without calling any native image service."""
            frames.append(frame)
            received.set()

        pipe = NativePipe(incoming, outgoing, 256, receive, closed.set, errors.append)
        pipe.send(b'{"reply":1}\n')
        pipe.start()
        try:
            peer_write.write(b'{"request":1}\n')
            wait_for_event(received)
            assert frames == [b'{"request":1}\n']
            assert peer_read.readline(256) == b'{"reply":1}\n'
            peer_write.close()
            wait_for_event(closed)
            pipe.join()
            assert errors == []
        finally:
            peer_write.close()
            pipe.stop()
            pipe.join()


@pytest.mark.parametrize("frame", [b"x" * 257 + b"\n", b"incomplete"])
def test_invalid_peer_frames_close_the_channel_before_dispatch(frame: bytes) -> None:
    """Reject long prefixes and truncated frames without parsing their content."""
    with ExitStack() as stack:
        inbound_read, inbound_write = os.pipe()
        outbound_read, outbound_write = os.pipe()
        incoming = stack.enter_context(os.fdopen(inbound_read, "rb", buffering=0))
        peer_write = stack.enter_context(os.fdopen(inbound_write, "wb", buffering=0))
        stack.enter_context(os.fdopen(outbound_read, "rb", buffering=0))
        outgoing = stack.enter_context(os.fdopen(outbound_write, "wb", buffering=0))
        frames: list[bytes] = []
        errors: list[Exception] = []
        closed = Event()
        pipe = NativePipe(
            incoming, outgoing, 256, frames.append, closed.set, errors.append
        )
        pipe.start()
        peer_write.write(frame)
        peer_write.close()
        wait_for_event(closed)
        pipe.join()
        assert frames == []
        assert len(errors) == 1
        assert isinstance(errors[0], ValueError)


def test_response_backpressure_is_bounded_before_starting_a_writer() -> None:
    """Refuse excess retained frames instead of blocking a native callback."""
    with ExitStack() as stack:
        inbound_read, inbound_write = os.pipe()
        outbound_read, outbound_write = os.pipe()
        incoming = stack.enter_context(os.fdopen(inbound_read, "rb", buffering=0))
        stack.enter_context(os.fdopen(inbound_write, "wb", buffering=0))
        stack.enter_context(os.fdopen(outbound_read, "rb", buffering=0))
        outgoing = stack.enter_context(os.fdopen(outbound_write, "wb", buffering=0))
        errors: list[Exception] = []
        pipe = NativePipe(
            incoming, outgoing, 8, lambda frame: None, lambda: None, errors.append
        )
        pipe.send(b"1234567\n")
        with pytest.raises(RuntimeError, match="busy"):
            pipe.send(b"x\n")
        with pytest.raises(ValueError, match="frame"):
            pipe.send(b"incomplete")
        pipe.stop()
        with pytest.raises(RuntimeError, match="closed"):
            pipe.send(b"x\n")

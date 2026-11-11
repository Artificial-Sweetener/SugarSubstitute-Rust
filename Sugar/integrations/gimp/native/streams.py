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
"""Move bounded private frames without performing SDK calls or blocking GIMP's main loop."""

from __future__ import annotations

from collections import deque
from collections.abc import Callable
from threading import Condition, Thread
from typing import Protocol


class NativeInput(Protocol):
    """Borrow a viewer's readable binary pipe."""

    def readline(self, size: int = -1, /) -> bytes:
        """Read a bounded frame prefix."""
        ...


class NativeOutput(Protocol):
    """Borrow a viewer's writable binary pipe."""

    def write(self, data: bytes, /) -> int | None:
        """Write response bytes while allowing an explicit short write."""
        ...

    def flush(self) -> None:
        """Publish completed frames to the viewer."""
        ...


class NativePipe:
    """Own off-thread frame IO; the process owner closes borrowed handles after child exit."""

    def __init__(
        self,
        incoming: NativeInput,
        outgoing: NativeOutput,
        maximum_frame: int,
        receive: Callable[[bytes], None],
        disconnected: Callable[[], None],
        report: Callable[[Exception], None],
        maximum_queued: int = 8,
    ) -> None:
        """Bound queued response bytes as well as frame count before accepting native capture results."""
        if maximum_frame < 1 or maximum_queued < 1:
            raise ValueError("Native pipe limits must be positive.")
        self._incoming = incoming
        self._outgoing = outgoing
        self._maximum_frame = maximum_frame
        self._maximum_queued = maximum_queued
        self._receive = receive
        self._disconnected = disconnected
        self._report = report
        self._condition = Condition()
        self._queue: deque[bytes] = deque()
        self._charged = 0
        self._closed = False
        self._started = False
        self._reader = Thread(
            target=self._read, name="SugarCubes GIMP reader", daemon=True
        )
        self._writer = Thread(
            target=self._write, name="SugarCubes GIMP writer", daemon=True
        )

    def start(self) -> None:
        """Start one pair of workers for this owned viewer lifetime."""
        with self._condition:
            if self._started or self._closed:
                raise RuntimeError("The native pipe has already started or closed.")
            self._started = True
        self._writer.start()
        self._reader.start()

    def send(self, frame: bytes) -> None:
        """Queue a complete response without waiting for the viewer to drain its input pipe."""
        if not frame.endswith(b"\n") or len(frame) > self._maximum_frame:
            raise ValueError("Invalid native response frame.")
        with self._condition:
            if self._closed:
                raise RuntimeError("The native pipe has closed.")
            if (
                len(self._queue) >= self._maximum_queued
                or self._charged + len(frame) > self._maximum_frame
            ):
                raise RuntimeError("The native response pipe is busy.")
            self._queue.append(frame)
            self._charged += len(frame)
            self._condition.notify()

    def stop(self) -> None:
        """Retire queued output without closing a handle that another thread may be writing."""
        self._end(None)

    def join(self, timeout: float = 2) -> None:
        """Join after the process owner has ended the viewer and unblocked both borrowed pipes."""
        if not self._started:
            return
        for worker in (self._reader, self._writer):
            worker.join(timeout=timeout)
        if self._reader.is_alive() or self._writer.is_alive():
            raise RuntimeError(
                "The GIMP viewer pipe workers did not finish after child exit."
            )

    def _read(self) -> None:
        """Admit bounded newline frames and turn EOF into native lifetime cancellation."""
        try:
            while True:
                with self._condition:
                    if self._closed:
                        return
                frame = self._incoming.readline(self._maximum_frame + 1)
                if not frame:
                    self._end(None)
                    return
                if len(frame) > self._maximum_frame or not frame.endswith(b"\n"):
                    raise ValueError(
                        "The viewer sent an oversized or incomplete native frame."
                    )
                self._receive(frame)
        except Exception as error:
            self._end(error)

    def _write(self) -> None:
        """Keep pipe backpressure off the native event loop and account for a frame until written."""
        try:
            while True:
                with self._condition:
                    self._condition.wait_for(lambda: self._closed or bool(self._queue))
                    if self._closed:
                        return
                    frame = self._queue.popleft()
                try:
                    written = 0
                    while written < len(frame):
                        count = self._outgoing.write(frame[written:])
                        if count is None or count < 1:
                            raise OSError(
                                "The viewer pipe stopped accepting response bytes."
                            )
                        written += count
                    self._outgoing.flush()
                finally:
                    with self._condition:
                        self._charged -= len(frame)
        except Exception as error:
            self._end(error)

    def _end(self, error: Exception | None) -> None:
        """Report an unexpected pipe fault once and wake both native lifetime and writer owners."""
        with self._condition:
            if self._closed:
                return
            self._closed = True
            self._charged -= sum(map(len, self._queue))
            self._queue.clear()
            self._condition.notify_all()
        try:
            if error is not None:
                self._report(error)
        finally:
            self._disconnected()

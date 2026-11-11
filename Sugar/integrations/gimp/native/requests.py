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
"""Schedule private requests on GIMP's main loop while receiving cancellation off-thread."""

from __future__ import annotations

from collections import deque
from collections.abc import Callable
from dataclasses import dataclass, field
from threading import Event, Lock
from typing import Literal

from .contract import NativeContract, fields, integer
from .dispatch import NativeDispatch
from .model import (
    NativeOperationCancelled,
    NativeOperationUnsupported,
    StaleNativeImage,
)


@dataclass(slots=True)
class _Request:
    """Keep cancellation live until native snapshots have actually left their owner."""

    size: int
    cancelled: Event = field(default_factory=Event)
    completed: bool = False


@dataclass(frozen=True, slots=True)
class _Work:
    """Carry validated work into the native event-loop thread."""

    request_id: int
    command: dict[str, object] | None


class NativeRequests:
    """Own bounded admission, request identities, cancellation and main-loop dispatch."""

    def __init__(
        self,
        contract: NativeContract,
        dispatch: NativeDispatch,
        schedule: Callable[[Callable[[], None]], None],
        send: Callable[[bytes], None],
        report: Callable[[Exception], None],
        closed: Callable[[], None],
        maximum_pending: int | None = None,
    ) -> None:
        """Inject viewer transport and the native main-loop scheduler; never call GI on a reader thread."""
        admitted_pending = (
            contract.limits.pending_requests
            if maximum_pending is None
            else maximum_pending
        )
        if admitted_pending < 1:
            raise ValueError("Native request admission must be positive.")
        self._contract = contract
        self._dispatch = dispatch
        self._schedule = schedule
        self._send = send
        self._report = report
        self._on_closed = closed
        self._maximum_pending = admitted_pending
        self._lock = Lock()
        self._entries: dict[int, _Request] = {}
        self._queue: deque[_Work] = deque()
        self._highest = 0
        self._charged = 0
        self._pending = 0
        self._scheduled = False
        self._closed = False
        self._cleaned = False

    def receive(self, frame: bytes) -> None:
        """Admit a pipe frame off-thread and make cancellation visible during native decoding."""
        value = self._contract.decode(frame)
        kind, request_id = value["type"], integer(value["id"])
        if kind not in ("request", "cancel"):
            raise ValueError(
                "The native parent accepts requests and cancellation only."
            )
        busy = False
        with self._lock:
            if self._closed:
                return
            if kind == "cancel":
                entry = self._entries.get(request_id)
                if entry is None or entry.cancelled.is_set():
                    return
                entry.cancelled.set()
                self._queue.append(_Work(request_id, None))
            else:
                if request_id <= self._highest:
                    raise ValueError(
                        "The native request identity was reused or reordered."
                    )
                self._highest = request_id
                busy = (
                    self._pending >= self._maximum_pending
                    or self._charged + len(frame) > self._contract.limits.frame_bytes
                )
                if not busy:
                    self._entries[request_id] = _Request(len(frame))
                    self._pending += 1
                    self._charged += len(frame)
                    self._queue.append(_Work(request_id, fields(value["command"])))
        if busy:
            self._reply(
                request_id, _failure("error", "The GIMP native bridge is busy.")
            )
        else:
            self._wake()

    def close(self) -> None:
        """Cancel pending work immediately and perform native cleanup on the owning loop."""
        with self._lock:
            if self._closed:
                return
            self._closed = True
            for entry in self._entries.values():
                entry.cancelled.set()
            self._queue.clear()
        self._wake()

    def _wake(self) -> None:
        """Coalesce callbacks so nested native event processing cannot reenter an SDK operation."""
        with self._lock:
            if self._scheduled or self._cleaned:
                return
            self._scheduled = True
        try:
            self._schedule(self._drain)
        except Exception as error:
            self._report(error)
            raise

    def _drain(self) -> None:
        """Run exactly one native operation before yielding back to GIMP's event loop."""
        with self._lock:
            closing = self._closed
            work = self._queue.popleft() if self._queue and not closing else None
        try:
            if closing:
                self._cleanup()
            elif work is not None:
                if work.command is None:
                    self._dispatch.cancel(work.request_id)
                else:
                    self._execute(work)
                retained = self._dispatch.retained_requests
                with self._lock:
                    self._entries = {
                        key: item
                        for key, item in self._entries.items()
                        if not item.completed or key in retained
                    }
        except Exception as error:
            self._report(error)
            self.close()
        finally:
            with self._lock:
                self._scheduled = False
                more = bool(self._queue) or (self._closed and not self._cleaned)
            if more:
                self._wake()

    def _execute(self, work: _Work) -> None:
        """Keep successful atomic insertion replies authoritative even when cancellation arrives later."""
        with self._lock:
            entry = self._entries[work.request_id]
        try:
            if work.command is None:
                raise ValueError("A native command is required.")
            result = self._dispatch.execute(
                work.request_id, work.command, entry.cancelled.is_set
            )
            if entry.cancelled.is_set() and result.get("committed") is not True:
                self._dispatch.cancel(work.request_id)
                result = _failure("cancelled", "GIMP native request cancelled.")
            self._contract.validate(result, "NativeResult")
        except NativeOperationCancelled:
            result = _failure("cancelled", "GIMP native request cancelled.")
        except StaleNativeImage:
            result = _failure(
                "stale", "The GIMP document or capture is no longer available."
            )
        except NativeOperationUnsupported:
            result = _failure(
                "unsupported",
                "This GIMP document does not support the requested operation.",
            )
        except Exception as error:
            self._report(error)
            # A failed response must not strand a just-created native snapshot.
            self._dispatch.cancel(work.request_id)
            result = _failure("error", "The GIMP native operation failed.")
        finally:
            with self._lock:
                self._charged -= entry.size
                entry.size = 0
                entry.completed = True
                self._pending -= 1
        self._reply(work.request_id, result)

    def _reply(self, request_id: int, result: dict[str, object]) -> None:
        """Publish validated bytes without retaining payloads or claiming transport retry ownership."""
        encoded = self._contract.encode(
            {"type": "reply", "id": request_id, "result": result}
        )
        with self._lock:
            if self._closed:
                return
        self._send(encoded)

    def _cleanup(self) -> None:
        """Attempt all native releases once and always notify the owning plug-in loop."""
        try:
            self._dispatch.close()
        except Exception as error:
            self._report(error)
        finally:
            with self._lock:
                self._entries.clear()
                self._cleaned = True
            self._on_closed()


def _failure(
    status: Literal["cancelled", "stale", "unsupported", "error"], message: str
) -> dict[str, object]:
    """Return stable native failure values while diagnostics retain the exception context."""
    return {"kind": "failure", "status": status, "message": message}

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
"""Drive the production native request owner with an explicit deterministic event loop."""

from __future__ import annotations

from collections import deque
from collections.abc import Callable
from dataclasses import dataclass, field

from integrations.gimp.native.contract import fields
from integrations.gimp.native.requests import NativeRequests
from .native_session import NativeSessionFixture


@dataclass
class NativeRequestFixture:
    """Replace only the external event scheduler and byte pipe."""

    session: NativeSessionFixture
    callbacks: deque[Callable[[], None]] = field(default_factory=deque)
    frames: list[bytes] = field(default_factory=list)
    closed: list[bool] = field(default_factory=list)
    requests: NativeRequests = field(init=False)

    def __post_init__(self) -> None:
        """Bind actual request admission and native services to the fixture loop."""
        self.requests = NativeRequests(
            self.session.contract,
            self.session.dispatch,
            self.callbacks.append,
            self.frames.append,
            self.session.errors.append,
            lambda: self.closed.append(True),
            maximum_pending=2,
        )

    def receive(self, request_id: int, command: dict[str, object]) -> None:
        """Send an actual validated frame without touching the native SDK synchronously."""
        self.requests.receive(
            self.session.contract.encode(
                {"type": "request", "id": request_id, "command": command}
            )
        )

    def cancel(self, request_id: int) -> None:
        """Deliver cancellation on the same pipe boundary used by the viewer."""
        self.requests.receive(
            self.session.contract.encode({"type": "cancel", "id": request_id})
        )

    def drain(self) -> None:
        """Run scheduled native callbacks without a GUI or polling timer."""
        while self.callbacks:
            self.callbacks.popleft()()

    def result(self, request_id: int) -> dict[str, object]:
        """Decode the reply rather than inspecting private request-owner state."""
        for frame in self.frames:
            value = self.session.contract.decode(frame)
            if value["id"] == request_id:
                return fields(value["result"])
        raise AssertionError("The fixture has not received this native reply.")

    def stop(self) -> None:
        """Close the production owner and release external filesystem resources."""
        self.requests.close()
        self.drain()
        self.session.files.close()

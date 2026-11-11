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
"""Describe the installed GLib event loop used by the modeless native plug-in process."""

from __future__ import annotations

from collections.abc import Callable
from typing import Protocol


class NativeLoop(Protocol):
    """Own one plug-in event-loop lifetime."""

    def run(self) -> None:
        """Service native callbacks until the companion ends."""
        ...

    def quit(self) -> None:
        """End this loop without stopping GIMP's own application loop."""
        ...


class NativeLoopFactory(Protocol):
    """Construct a loop without importing dynamic GI classes into lifecycle code."""

    def new(self, context: object | None, running: bool) -> NativeLoop:
        """Create the plug-in's explicit main loop."""
        ...


class NativeLoopApi(Protocol):
    """Schedule SDK work onto the installed GLib thread."""

    MainLoop: NativeLoopFactory

    def idle_add(self, callback: Callable[[], bool]) -> int:
        """Schedule one callback safely from a pipe worker thread."""
        ...

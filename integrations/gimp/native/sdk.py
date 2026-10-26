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
"""Load installed GIMP namespaces at the native adapter boundary."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from importlib import import_module
from typing import Protocol, cast

from .image_api import GimpImageApi
from .channel_api import GimpChannelApi
from .file_api import GioFileApi
from .output_api import GimpOutputApi
from .loop_api import NativeLoopApi


class RectangleFactory(Protocol):
    """Construct native GEGL region values."""

    def new(self, x: int, y: int, width: int, height: int) -> object:
        """Create one region in scaled buffer coordinates."""
        ...


class AbyssModes(Protocol):
    """Expose the native out-of-bounds sampling mode."""

    CLAMP: int


class ColorFactory(Protocol):
    """Create the native channel editor's standard overlay color."""

    def new(self, value: str) -> object:
        """Translate a literal native color at the GEGL boundary."""
        ...


class GeglApi(Protocol):
    """Expose only GEGL operations used by native capture."""

    Rectangle: RectangleFactory
    AbyssPolicy: AbyssModes
    Color: ColorFactory

    def init(self, argv: list[str] | None) -> list[str] | None:
        """Initialize native buffer access for this plug-in process."""
        ...


@dataclass(frozen=True, slots=True)
class NativeSdk:
    """Retain installed native collaborators independently of the viewer."""

    gimp: GimpImageApi
    gegl: GeglApi
    output: GimpOutputApi
    gio: GioFileApi
    channels: GimpChannelApi


def load_sdk() -> NativeSdk:
    """Load installed packages without modifying import paths or importing a source checkout."""
    gi = import_module("gi")
    require: object = getattr(gi, "require_version", None)
    if not callable(require):
        raise RuntimeError(
            "The installed GIMP Python runtime does not provide GI version selection."
        )
    # GI creates its modules dynamically and supplies no static SDK declarations.
    require_version = cast(Callable[[str, str], None], require)
    require_version("Gimp", "3.0")
    require_version("Gegl", "0.4")
    require_version("Gio", "2.0")
    gimp_namespace = import_module("gi.repository.Gimp")
    gimp = cast(GimpImageApi, gimp_namespace)
    gegl = cast(GeglApi, import_module("gi.repository.Gegl"))
    gegl.init(None)
    return NativeSdk(
        gimp,
        gegl,
        cast(GimpOutputApi, gimp_namespace),
        cast(GioFileApi, import_module("gi.repository.Gio")),
        cast(GimpChannelApi, gimp_namespace),
    )


def load_event_loop() -> NativeLoopApi:
    """Load GLib after the installed SDK has selected its GI versions."""
    return cast(NativeLoopApi, import_module("gi.repository.GLib"))


def host_process_id() -> int:
    """Read GIMP's public host identity after SDK initialization, rather than the plug-in's PID."""
    function: object = getattr(import_module("gi.repository.Gimp"), "getpid", None)
    if not callable(function):
        raise RuntimeError(
            "The installed GIMP SDK does not expose its host process identity."
        )
    value: object = function()
    if type(value) is not int or value < 1:
        raise RuntimeError("GIMP returned an invalid host process identity.")
    return value


def host_window_handle() -> int | None:
    """Read the invoking display's public HWND without guessing the foreground window.

    GIMP returns an opaque native handle as bytes. A headless host supplies no
    display; malformed handles fail before a companion process is admitted.
    """
    function: object = getattr(
        import_module("gi.repository.Gimp"), "progress_get_window_handle", None
    )
    if not callable(function):
        raise RuntimeError(
            "The installed GIMP SDK does not expose display window association."
        )
    value: object = function()
    if value is None:
        return None
    get_data: object = getattr(value, "get_data", None)
    data: object = get_data() if callable(get_data) else None
    if data == b"":
        get_size: object = getattr(value, "get_size", None)
        size: object = get_size() if callable(get_size) else None
        # The installed GI binding boxes GIMP's nullable headless handle as empty GBytes.
        if type(size) is int and size == 0:
            return None
    if not isinstance(data, bytes) or len(data) != 8:
        raise RuntimeError("GIMP returned an invalid display window handle.")
    handle = int.from_bytes(data, "little")
    if not handle:
        raise RuntimeError("GIMP returned an empty display window handle.")
    return handle

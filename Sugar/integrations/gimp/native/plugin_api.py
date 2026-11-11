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
"""Describe the public libgimp registration surface independently of dynamic GI modules."""

from __future__ import annotations

from collections.abc import Callable
from typing import Protocol

from .image_api import NativeImage


class PluginProcedure(Protocol):
    """Register one image-scoped native menu action and its result."""

    def set_image_types(self, types: str) -> None:
        """Declare the image modes admitted by the action."""
        ...

    def set_menu_label(self, label: str) -> None:
        """Name the native action."""
        ...

    def set_documentation(self, summary: str, description: str, help_id: str) -> None:
        """Describe the action through GIMP's own procedure metadata."""
        ...

    def set_attribution(self, authors: str, copyright: str, date: str) -> None:
        """Identify the first-party implementation."""
        ...

    def add_menu_path(self, path: str) -> None:
        """Contribute to the host's menu rather than constructing another one."""
        ...

    def new_return_values(self, status: int, error: object) -> object:
        """Return an SDK-owned procedure result."""
        ...


class NativePlugin(Protocol):
    """Enable public wire processing while the modeless companion runs."""

    def persistent_enable(self) -> None:
        """Allow GIMP messages to be serviced by the plug-in's GLib loop."""
        ...


RunImage = Callable[
    [PluginProcedure, object, NativeImage, object, object, object], object
]


class ImageProcedureFactory(Protocol):
    """Construct an image procedure using the installed SDK callback convention."""

    def new(
        self,
        plugin: NativePlugin,
        name: str,
        kind: int,
        run: RunImage,
        data: object | None,
    ) -> PluginProcedure:
        """Register an explicit launch image instead of guessing the active document."""
        ...


class ProcedureKinds(Protocol):
    """Expose supported plug-in process registration."""

    PLUGIN: int


class ProcedureStatus(Protocol):
    """Return GIMP's native success and failure categories."""

    SUCCESS: int
    CALLING_ERROR: int
    EXECUTION_ERROR: int


class RunModes(Protocol):
    """Expose the native interactive invocation category."""

    INTERACTIVE: int


class PluginApi(Protocol):
    """Keep dynamic GObject registration isolated from viewer and canvas policy."""

    PlugIn: type[NativePlugin]
    ImageProcedure: ImageProcedureFactory
    PDBProcType: ProcedureKinds
    PDBStatusType: ProcedureStatus
    RunMode: RunModes

    def main(self, plugin_type: object, argv: list[str]) -> int:
        """Service GIMP's standard plug-in process protocol."""
        ...


class ErrorFactory(Protocol):
    """Construct GI-compatible error objects, including the required empty success value."""

    def __call__(self) -> object:
        """Create the empty error required by the installed GI binding."""
        ...

    def new_literal(self, domain: int, message: str, code: int) -> object:
        """Return an actionable host-facing launch failure."""
        ...


class PluginErrors(Protocol):
    """Use GLib's native error representation at the host boundary."""

    Error: ErrorFactory

    def quark_from_string(self, domain: str) -> int:
        """Identify the native plug-in error domain."""
        ...

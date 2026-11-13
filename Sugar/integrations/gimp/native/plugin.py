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
"""Register SugarCubes as a public image procedure and own its native menu lifetime."""

from __future__ import annotations

from collections.abc import Callable, Sequence
from importlib import import_module
import logging
from pathlib import Path
import sys
from typing import cast

from .image_api import NativeImage
from .plugin_api import NativePlugin, PluginApi, PluginErrors, PluginProcedure
from .sdk import load_event_loop, load_sdk
from .viewer import run_native_viewer

PROCEDURE = "plug-in-sugarcubes"
_logger = logging.getLogger(__name__)


def create_plugin(
    api: PluginApi, errors: PluginErrors, launch: Callable[[int], None]
) -> type[NativePlugin]:
    """Build the GI subclass at its dynamic boundary while keeping launch behavior injectable."""

    def query(plugin: NativePlugin) -> list[str]:
        """Advertise exactly the native menu procedure owned by this package."""
        return [PROCEDURE]

    def i18n(plugin: NativePlugin, name: str) -> tuple[bool, None, None]:
        """Use the package's literal labels without borrowing another plug-in's translation domain."""
        return False, None, None

    def create(plugin: NativePlugin, name: str) -> PluginProcedure | None:
        """Contribute one standard image action to GIMP's Windows menu."""
        if name != PROCEDURE:
            return None

        def run(
            procedure: PluginProcedure,
            mode: object,
            image: NativeImage,
            drawables: object,
            config: object,
            data: object,
        ) -> object:
            """Run modeless IO against only the image supplied by GIMP's invocation."""
            if mode != api.RunMode.INTERACTIVE:
                return procedure.new_return_values(
                    api.PDBStatusType.CALLING_ERROR,
                    errors.Error.new_literal(
                        errors.quark_from_string("sugarcubes-gimp"),
                        "Open SugarCubes from GIMP's Windows menu.",
                        1,
                    ),
                )
            try:
                if not image.is_valid():
                    raise RuntimeError("The GIMP launch image has closed.")
                plugin.persistent_enable()
                launch(image.get_id())
            except Exception as error:
                _logger.exception("SugarCubes GIMP companion launch failed")
                return procedure.new_return_values(
                    api.PDBStatusType.EXECUTION_ERROR,
                    errors.Error.new_literal(
                        errors.quark_from_string("sugarcubes-gimp"),
                        str(error),
                        1,
                    ),
                )
            return procedure.new_return_values(
                api.PDBStatusType.SUCCESS, errors.Error()
            )

        procedure = api.ImageProcedure.new(
            plugin, name, api.PDBProcType.PLUGIN, run, None
        )
        procedure.set_image_types("*")
        procedure.set_menu_label("SugarCubes…")
        procedure.set_documentation(
            "Open SugarCubes for this image",
            "Open the SugarCubes companion and connect this GIMP image to ComfyUI.",
            PROCEDURE,
        )
        procedure.set_attribution(
            "Artificial Sweetener and contributors",
            "Artificial Sweetener and contributors",
            "2026",
        )
        procedure.add_menu_path("<Image>/Windows")
        return procedure

    # GI supplies a GObject metaclass at runtime rather than statically declared Python classes.
    return type(
        "SugarCubesGimpPlugin",
        (api.PlugIn,),
        {
            "__module__": __name__,
            "do_query_procedures": query,
            "do_create_procedure": create,
            "do_set_i18n": i18n,
        },
    )


def main(directory: Path, argv: Sequence[str]) -> int:
    """Enter GIMP's own Python plug-in protocol using only package-local companion resources."""
    if sys.platform == "win32" and sys.maxsize > 2**32:
        sdk = load_sdk()
        api = cast(PluginApi, import_module("gi.repository.Gimp"))
        errors = cast(PluginErrors, import_module("gi.repository.GLib"))
        executable = directory / "companion" / "sugarcubes-gimp-companion.exe"

        def launch(image_id: int) -> None:
            """Bind the packaged viewer to the explicit image for this native invocation."""
            run_native_viewer(image_id, executable, sdk, load_event_loop())

        plugin = create_plugin(api, errors, launch)
        native_type: object = getattr(plugin, "__gtype__", None)
        if native_type is None:
            raise RuntimeError("GIMP did not register the SugarCubes plug-in type.")
        return api.main(native_type, list(argv))
    raise RuntimeError("This SugarCubes companion package requires 64-bit Windows.")

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
"""Open already validated still images in the installed Photoshop through its COM server."""

from collections.abc import Sequence
from dataclasses import dataclass
import importlib
import importlib.util
import os
from pathlib import Path
import sys
from typing import Protocol

MAX_PHOTOSHOP_LAYERS = 256
MAX_PHOTOSHOP_NAME_LENGTH = 120

# ExtendScript receives the document name, then path/name pairs, as `arguments`, so no path or
# name is ever spliced into script text. Each image becomes one named layer, the last on top.
_OPEN_AS_LAYERS = """
var units = app.preferences.rulerUnits;
app.preferences.rulerUnits = Units.PIXELS;
try {
  var target = null;
  var placeholder = null;
  for (var index = 1; index + 1 < arguments.length; index += 2) {
    var source = app.open(new File(arguments[index]));
    var width = source.width.as("px");
    var height = source.height.as("px");
    if (target === null) {
      target = app.documents.add(width, height, source.resolution, arguments[0],
        NewDocumentMode.RGB, DocumentFill.TRANSPARENT);
      placeholder = target.activeLayer;
      app.activeDocument = source;
    } else if (width > target.width.as("px") || height > target.height.as("px")) {
      app.activeDocument = target;
      target.resizeCanvas(Math.max(width, target.width.as("px")),
        Math.max(height, target.height.as("px")), AnchorPosition.TOPLEFT);
      app.activeDocument = source;
    }
    var layer = source.activeLayer.duplicate(target, ElementPlacement.PLACEATBEGINNING);
    source.close(SaveOptions.DONOTSAVECHANGES);
    app.activeDocument = target;
    layer.name = arguments[index + 1];
  }
  if (placeholder !== null) placeholder.remove();
} finally {
  app.preferences.rulerUnits = units;
}
"""


@dataclass(frozen=True, slots=True)
class PhotoshopLayer:
    """One existing still image and the layer name it takes in the new document."""

    path: Path
    name: str


class PhotoshopAutomation(Protocol):
    """Run one fixed script inside Photoshop with literal string arguments."""

    @property
    def available(self) -> bool:
        """Report whether Photoshop's automation server is registered and reachable."""

    def run(self, script: str, arguments: Sequence[str]) -> None:
        """Run the script, raising OSError when Photoshop refuses or fails it."""


class PhotoshopImageOpener(Protocol):
    """Keep the Photoshop boundary outside image resolution and request validation."""

    @property
    def available(self) -> bool:
        """Report whether an installed Photoshop can open images on this desktop."""

    def open(self, document: str, layers: Sequence[PhotoshopLayer]) -> None:
        """Open the images as layers of one new document named ``document``."""


class ComPhotoshopAutomation:
    """Reach Photoshop's registered COM server through pywin32 when the interpreter has it."""

    @property
    def available(self) -> bool:
        """Require pywin32 and a registered `Photoshop.Application` class without starting Photoshop."""
        if sys.platform != "win32" or importlib.util.find_spec("win32com") is None:
            return False
        import winreg

        try:
            with winreg.OpenKey(
                winreg.HKEY_CLASSES_ROOT, r"Photoshop.Application\CLSID"
            ):
                return True
        except OSError:
            return False

    def run(self, script: str, arguments: Sequence[str]) -> None:
        """Dispatch on this worker thread's own COM apartment and translate COM failures."""
        # pywin32 is untyped; these modules are reached only at this adapter boundary.
        pythoncom = importlib.import_module("pythoncom")
        client = importlib.import_module("win32com.client")
        com_error: type[BaseException] = importlib.import_module("pywintypes").com_error
        pythoncom.CoInitialize()
        try:
            application = client.Dispatch("Photoshop.Application")
            # 1 is PsJavaScriptExecutionMode.psNeverShowDebugger.
            application.DoJavaScript(script, list(arguments), 1)
        except com_error as error:
            raise OSError("Photoshop could not open these images.") from error
        finally:
            pythoncom.CoUninitialize()


class WindowsPhotoshopOpener:
    """Validate the layer set before handing it to Photoshop as literal script arguments."""

    def __init__(self, automation: PhotoshopAutomation | None = None) -> None:
        """Use the COM server unless a test supplies another automation boundary."""
        self._automation = automation or ComPhotoshopAutomation()

    @property
    def available(self) -> bool:
        """Disable Photoshop for service sessions, where no desktop can show it."""
        return (
            os.name == "nt"
            and os.environ.get("SESSIONNAME", "").casefold() != "services"
            and self._automation.available
        )

    def open(self, document: str, layers: Sequence[PhotoshopLayer]) -> None:
        """Refuse missing files, relative paths, empty or oversized sets and unprintable names."""
        if not self.available:
            raise ValueError("Photoshop is unavailable on this computer.")
        if not layers or len(layers) > MAX_PHOTOSHOP_LAYERS:
            raise ValueError("Choose between one and 256 images to open in Photoshop.")
        arguments = [_name(document)]
        for layer in layers:
            if not layer.path.is_absolute() or not layer.path.is_file():
                raise ValueError("This output image is unavailable.")
            arguments.extend((str(layer.path), _name(layer.name)))
        self._automation.run(_OPEN_AS_LAYERS, arguments)


def _name(value: str) -> str:
    """Keep printable characters of a bounded document or layer name."""
    printable = "".join(character for character in value if character.isprintable())
    name = printable.strip()[:MAX_PHOTOSHOP_NAME_LENGTH].strip()
    if not name:
        raise ValueError("Photoshop documents and layers need a name.")
    return name

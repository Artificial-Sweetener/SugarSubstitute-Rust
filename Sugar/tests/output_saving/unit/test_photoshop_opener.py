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
"""Verify the Photoshop boundary passes literal arguments and refuses unusable layer sets."""

from collections.abc import Sequence
from dataclasses import dataclass, field
from pathlib import Path

import pytest

from sugarcubes.output_saving.infrastructure.photoshop_opener import (
    PhotoshopLayer,
    WindowsPhotoshopOpener,
)


@dataclass
class Automation:
    """Record script runs in place of Photoshop's COM server."""

    available: bool = True
    runs: list[tuple[str, list[str]]] = field(default_factory=list)

    def run(self, script: str, arguments: Sequence[str]) -> None:
        """Record one fixed script and its arguments."""
        self.runs.append((script, list(arguments)))


class DesktopOpener(WindowsPhotoshopOpener):
    """Run the proof independently of the operating-system desktop session."""

    @property
    def available(self) -> bool:
        """Follow only the substituted automation server."""
        return self._automation.available


def test_layers_reach_the_fixed_script_as_arguments_never_as_script_text(
    tmp_path: Path,
) -> None:
    """Keep paths and names out of the script while cleaning names of control characters."""
    automation = Automation()
    first, second = tmp_path / "a'; alert(1); '.png", tmp_path / "b.png"
    for path in (first, second):
        path.write_bytes(b"pixels")
    DesktopOpener(automation).open(
        " Save\x00 1 ",
        [PhotoshopLayer(first, "Save\n 1"), PhotoshopLayer(second, "Save 2")],
    )
    [(script, arguments)] = automation.runs
    assert arguments == ["Save 1", str(first), "Save 1", str(second), "Save 2"]
    assert str(first) not in script and "Save 1" not in script
    assert "arguments[0]" in script and "DONOTSAVECHANGES" in script


def test_refuses_unavailable_photoshop_missing_files_and_unusable_sets(
    tmp_path: Path,
) -> None:
    """Fail before reaching Photoshop when the request cannot be honoured exactly."""
    image = tmp_path / "image.png"
    image.write_bytes(b"pixels")
    automation = Automation()
    opener = DesktopOpener(automation)
    for document, layers in [
        ("Doc", []),
        ("Doc", [PhotoshopLayer(Path("relative.png"), "Layer")]),
        ("Doc", [PhotoshopLayer(tmp_path / "missing.png", "Layer")]),
        ("Doc", [PhotoshopLayer(image, "\x07")]),
        ("   ", [PhotoshopLayer(image, "Layer")]),
        ("Doc", [PhotoshopLayer(image, "Layer")] * 257),
    ]:
        with pytest.raises(ValueError):
            opener.open(document, layers)
    with pytest.raises(ValueError):
        DesktopOpener(Automation(available=False)).open(
            "Doc", [PhotoshopLayer(image, "Layer")]
        )
    assert automation.runs == []

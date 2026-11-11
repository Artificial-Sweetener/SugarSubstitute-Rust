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
"""Safeguard shared custom-element constructor identity across canvas and host bundles."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
IMPORT = re.compile(r"\b(?:from|import)\s*[\"']([^\"']+)[\"']")


def test_canvas_uses_the_shared_menu_and_single_fluent_registration_owner() -> None:
    """Reject bundled copies whose constructors disagree with already registered Fluent controls."""
    runtime = ROOT / "web/comfyui/ui"
    pending = [runtime / "output_canvas/presentation/vue/CanvasOverlayRuntime.js"]
    visited: set[Path] = set()
    registration_owners: set[Path] = set()
    menu_owners: set[Path] = set()
    floating_owners: set[Path] = set()
    while pending:
        module = pending.pop().resolve()
        if module in visited:
            continue
        visited.add(module)
        source = module.read_text("utf-8")
        if re.search(
            r"\bclass ContextMenu\b|\b(?:var|let|const) _?ContextMenu\d*\s*=\s*class\b",
            source,
        ):
            menu_owners.add(module)
        if re.search(
            r"\bclass FloatingSurface\b|\b(?:var|let|const) _?FloatingSurface\d*\s*=\s*class\b",
            source,
        ):
            floating_owners.add(module)
        if "fluent-text-input" in source and "customElements.get" in source:
            registration_owners.add(module)
        for specifier in IMPORT.findall(source):
            if specifier.startswith("."):
                dependency = (module.parent / specifier).resolve()
                assert dependency.is_relative_to(ROOT / "web")
                pending.append(dependency)
    assert (runtime / "menus/ContextMenu.js").resolve() in visited
    assert menu_owners == {(runtime / "menus/ContextMenu.js").resolve()}
    assert floating_owners == {(runtime / "menus/FloatingSurface.js").resolve()}
    assert registration_owners == {
        (runtime / "surface/appearance/FluentThemeRuntime.js").resolve()
    }

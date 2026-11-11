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
"""Own formatting-independent SugarScript workflow identity."""

from __future__ import annotations

import hashlib

from .renderer import render_sugarscript
from .syntax import SugarScriptDocument, SugarStatement


def compute_semantic_hash(statements: tuple[SugarStatement, ...]) -> str:
    """Hash canonical statement meaning for stable workflow and instance identity."""

    canonical_source = render_sugarscript(SugarScriptDocument(statements)).source
    return hashlib.sha256(canonical_source.encode("utf-8")).hexdigest()

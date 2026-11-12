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
"""Own the Comfy Registry install format of a node pack and its safe replacement.

This package uses only the standard library and relative imports, because the
prestartup script loads it by file location, under a private module name, before
ComfyUI imports SugarCubes itself.
"""

from __future__ import annotations

import logging
from pathlib import Path

from .startup import run_startup_update

_logger = logging.getLogger("sugarcubes.pack_install")


def apply_pending_update(pack_folder: Path) -> None:
    """Run the startup update step without ever stopping ComfyUI from starting."""

    try:
        run_startup_update(pack_folder)
    except Exception:
        # Prestartup must never block ComfyUI; the journal keeps the pack recoverable.
        _logger.exception(
            "SugarCubes update event=startup_step_failed action=continue_with_installed"
        )


__all__ = ["apply_pending_update"]

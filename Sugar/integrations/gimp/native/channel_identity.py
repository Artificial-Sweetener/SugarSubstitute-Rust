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
"""Qualify authored coverage by its persistent native marker rather than its editable name."""

from __future__ import annotations

from .image_api import NativeChannelSource

MASK_MARKER = "sugarcubes-mask"
MASK_VERSION = b"1"


def is_authored_channel(channel: NativeChannelSource) -> bool:
    """Refuse unrelated channels even if a stale reference happens to name their native identity."""
    marker = channel.get_parasite(MASK_MARKER)
    return marker is not None and list(marker.get_data()) == list(MASK_VERSION)

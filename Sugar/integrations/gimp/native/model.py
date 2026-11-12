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
"""Carry typed native metadata and pixels across the editor adapter's internal boundary."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal


class StaleNativeImage(RuntimeError):
    """Reject a closed image or geometry that differs from the captured destination."""


class NativeOperationCancelled(RuntimeError):
    """Stop native preparation before its atomic mutation point."""


class NativeOperationUnsupported(RuntimeError):
    """Refuse a native destination that cannot preserve the admitted operation."""


@dataclass(frozen=True, slots=True)
class NativePng:
    """Carry PNG bytes and dimensions already admitted by the shared image policy."""

    data: bytes
    width: int
    height: int


@dataclass(frozen=True, slots=True)
class NativeInsertedLayer:
    """Report the exact native layer identity and shared placement committed to a document."""

    tattoo: int
    left: int
    top: int
    width: int
    height: int


@dataclass(frozen=True, slots=True)
class NativeDocument:
    """Identify a native canvas independently of content edits and UI focus."""

    image_id: int
    revision: str
    width: int
    height: int
    name: str


@dataclass(frozen=True, slots=True)
class NativeLayerInfo:
    """Describe native layer identities without applying source interpretation policy."""

    tattoo: int
    parent_tattoo: int | None
    name: str
    depth: int
    visible: bool
    has_mask: bool


@dataclass(frozen=True, slots=True)
class NativePixels:
    """Retain exact straight sRGB bytes or selection coverage read by the native SDK."""

    width: int
    height: int
    format: Literal["rgba8", "coverage8"]
    data: bytes

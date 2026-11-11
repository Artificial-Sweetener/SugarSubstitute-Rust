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
"""Describe prepared-image ownership independently of model presentation and storage."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

from .models import ThumbnailReference

ThumbnailSourceOrigin = Literal[
    "civitai", "openmodeldb", "user", "local", "bundled", "legacy"
]


@dataclass(frozen=True, slots=True)
class ThumbnailOwnership:
    """Retain persisted image ownership even while a prepared file is unavailable."""

    origin: ThumbnailSourceOrigin
    thumbnails: tuple[ThumbnailReference, ...]


@dataclass(frozen=True, slots=True)
class ThumbnailSourceSummary:
    """Count available source images and distinct physical prepared assets."""

    source_count: int
    variant_count: int
    byte_size: int


def thumbnail_source_origin(value: object) -> ThumbnailSourceOrigin:
    """Reject unknown origins before they address a cache ownership namespace."""
    if not isinstance(value, str):
        raise ValueError("Unknown thumbnail source origin.")
    if value == "civitai":
        return "civitai"
    if value == "openmodeldb":
        return "openmodeldb"
    if value == "user":
        return "user"
    if value == "local":
        return "local"
    if value == "bundled":
        return "bundled"
    if value == "legacy":
        return "legacy"
    raise ValueError("Unknown thumbnail source origin.")

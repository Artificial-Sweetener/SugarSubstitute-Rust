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
"""Own the reference CivitAI thumbnail-content policy independently of transport and rendering."""

from __future__ import annotations

from enum import StrEnum


class CivitaiThumbnailSafety(StrEnum):
    """Retain all user-selectable reference policies with stable persistence identities."""

    SFW_ONLY = "sfw_only"
    ALLOW_SOFT = "allow_soft"
    ALLOW_ALL = "allow_all"
    DISABLED = "disabled"


def allows_civitai_image(
    policy: CivitaiThumbnailSafety,
    *,
    image_type: str | None,
    nsfw: bool | None,
    level: str | int | None,
) -> bool:
    """Apply configured content restrictions equally to image and video thumbnail sources."""
    if policy is CivitaiThumbnailSafety.DISABLED:
        return False
    if image_type is not None and image_type.casefold() not in ("image", "video"):
        return False
    if policy is CivitaiThumbnailSafety.ALLOW_ALL:
        return True
    normalized = level.strip().casefold() if isinstance(level, str) else level
    if nsfw is not True and normalized in (
        None,
        0,
        1,
        "",
        "0",
        "1",
        "none",
        "false",
        "safe",
        "sfw",
    ):
        return True
    return (
        policy is CivitaiThumbnailSafety.ALLOW_SOFT
        and nsfw is not True
        and normalized in (2, "2", "soft", "racy")
    )

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
"""Normalize provider images once for metadata, release history, and discovery previews."""

from __future__ import annotations

from dataclasses import dataclass
import re
from urllib.parse import urlsplit

from ..domain.thumbnail_safety import CivitaiThumbnailSafety, allows_civitai_image


@dataclass(frozen=True, slots=True)
class CivitaiThumbnail:
    """Keep optional provider identity separate from a reviewed image URL."""

    image_id: str | None
    url: str


def valid_preview_url(url: str) -> bool:
    """Restrict image requests to the reviewed HTTPS image CDN."""
    try:
        parsed = urlsplit(url)
        return (
            parsed.scheme == "https"
            and parsed.hostname == "image.civitai.com"
            and parsed.port in (None, 443)
            and not parsed.username
            and not parsed.password
            and not parsed.fragment
        )
    except ValueError:
        return False


def valid_preview_redirect(url: str) -> bool:
    """Admit only reviewed public blob redirects after a valid preview request."""
    if valid_preview_url(url):
        return True
    try:
        parsed = urlsplit(url)
        return (
            parsed.scheme == "https"
            and parsed.hostname == "blobs-b2.civitai.com"
            and parsed.port in (None, 443)
            and not parsed.username
            and not parsed.password
            and not parsed.fragment
            and re.fullmatch(r"/file/blobs-managed-public/[A-Za-z0-9_-]+", parsed.path)
            is not None
        )
    except ValueError:
        return False


def select_civitai_thumbnail(
    images: object,
    policy: CivitaiThumbnailSafety = CivitaiThumbnailSafety.SFW_ONLY,
    *,
    discovery_portrait: bool = False,
) -> CivitaiThumbnail | None:
    """Keep provider order, requiring identified large portraits only for discovery cards."""
    if not isinstance(images, list):
        return None
    for image in images:
        if not isinstance(image, dict):
            continue
        url = image.get("url")
        if not isinstance(url, str) or not valid_preview_url(url):
            continue
        image_type, nsfw, level = (
            image.get("type"),
            image.get("nsfw"),
            image.get("nsfwLevel"),
        )
        if (
            image_type is not None
            and not isinstance(image_type, str)
            or nsfw is not None
            and not isinstance(nsfw, bool)
            or level is not None
            and (not isinstance(level, (str, int)) or isinstance(level, bool))
        ):
            continue
        if not allows_civitai_image(
            policy, image_type=image_type, nsfw=nsfw, level=level
        ):
            continue
        identity = image.get("id")
        image_id = (
            str(identity)
            if isinstance(identity, int)
            and not isinstance(identity, bool)
            and 0 < identity < 10**16
            else None
        )
        if discovery_portrait:
            width, height = image.get("width"), image.get("height")
            if (
                image_id is None
                or not isinstance(width, int)
                or isinstance(width, bool)
                or not isinstance(height, int)
                or isinstance(height, bool)
                or width < 640
                or height < 800
                or height <= width
            ):
                continue
            url = re.sub(r"/width=\d+/", "/width=512/", url, count=1)
            url = url.replace("/original=true/", "/width=512/", 1)
        return CivitaiThumbnail(image_id, url)
    return None

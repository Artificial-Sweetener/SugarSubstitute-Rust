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
"""Validate the complete OpenModelDB catalog at its untrusted JSON boundary."""

from __future__ import annotations

import re
from urllib.parse import urlsplit

from ..domain.openmodeldb_catalog import (
    OpenModelDbCatalog,
    OpenModelDbModel,
    OpenModelDbResource,
)


def approved_openmodeldb_preview(url: str) -> bool:
    """Accept only reviewed HTTPS image origins without credentials or custom ports."""
    try:
        parsed = urlsplit(url)
        return (
            parsed.scheme == "https"
            and parsed.hostname in ("openmodeldb.info", "i.slow.pics")
            and parsed.port in (None, 443)
            and not parsed.username
            and not parsed.password
            and not parsed.fragment
        )
    except ValueError:
        return False


def parse_openmodeldb_catalog(payload: object) -> OpenModelDbCatalog:
    """Retain reference catalog fields while isolating malformed models and resources."""
    if not isinstance(payload, dict):
        raise ValueError("OpenModelDB catalog must be an object.")
    models: list[OpenModelDbModel] = []
    for model_id, value in payload.items():
        if not isinstance(model_id, str) or not re.fullmatch(
            r"[A-Za-z0-9_-]{1,200}", model_id
        ):
            continue
        if not isinstance(value, dict):
            continue
        title = _text(value.get("name"), 1000)
        resources = _resources(value.get("resources"))
        if not title or not resources:
            continue
        models.append(
            OpenModelDbModel(
                model_id=model_id,
                name=title,
                author=_text(value.get("author"), 1000) or None,
                license_name=_text(value.get("license"), 1000) or None,
                architecture=_text(value.get("architecture"), 200) or None,
                scale=_positive_int(value.get("scale")),
                tags=_strings(value.get("tags")),
                description=_text(value.get("description"), 100_000) or None,
                resources=resources,
                thumbnail_url=_preview(value.get("thumbnail")),
            )
        )
    if not models:
        raise ValueError("OpenModelDB catalog contains no validated model resources.")
    return OpenModelDbCatalog(tuple(models))


def _resources(payload: object) -> tuple[OpenModelDbResource, ...]:
    """Require exact hashes, byte counts, and HTTPS resources before any catalog use."""
    if not isinstance(payload, list):
        return ()
    resources: list[OpenModelDbResource] = []
    for value in payload:
        if not isinstance(value, dict):
            continue
        resource_type = _text(value.get("type"), 100).casefold()
        size = _positive_int(value.get("size"))
        digest = value.get("sha256")
        urls = tuple(url for url in _strings(value.get("urls")) if _safe_https(url))
        if (
            resource_type not in ("pth", "safetensors")
            or size is None
            or not isinstance(digest, str)
            or not re.fullmatch(r"[a-fA-F0-9]{64}", digest)
            or not urls
        ):
            continue
        resources.append(OpenModelDbResource(resource_type, size, digest.lower(), urls))
    return tuple(resources)


def _safe_https(value: str) -> bool:
    """Validate catalog links independently of the stricter acquisition host policy."""
    try:
        parsed = urlsplit(value)
        _ = parsed.port
        return (
            parsed.scheme == "https"
            and parsed.hostname is not None
            and not parsed.username
            and not parsed.password
        )
    except ValueError:
        return False


def _positive_int(value: object) -> int | None:
    """Reject booleans and nonintegral sizes or scales at the provider boundary."""
    return (
        value
        if isinstance(value, int) and not isinstance(value, bool) and value > 0
        else None
    )


def _strings(value: object) -> tuple[str, ...]:
    """Preserve ordered bounded provider strings without parsing markup."""
    if not isinstance(value, list):
        return ()
    return tuple(text for item in value if (text := _text(item, 4096)))


def _text(value: object, limit: int) -> str:
    """Bound provider text while preserving literal labels rather than parsing markup."""
    return value.strip()[:limit] if isinstance(value, str) else ""


def _preview(value: object) -> str | None:
    """Prefer the published super-resolution image, matching Substitute's preview policy."""
    if not isinstance(value, dict):
        return None
    url = _text(value.get("SR"), 4096) or _text(value.get("url"), 4096)
    if url.startswith("/") and not url.startswith("//"):
        url = "https://openmodeldb.info" + url
    return url if _safe_https(url) else None

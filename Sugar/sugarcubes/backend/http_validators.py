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

"""Own HTTP content validators for immutable-by-content backend assets."""

from __future__ import annotations

import hashlib
from dataclasses import dataclass


@dataclass(frozen=True)
class ValidatedAsset:
    """Carry asset bytes with the strong entity tag derived from exactly those bytes."""

    body: bytes
    etag: str


def validated_asset(body: bytes) -> ValidatedAsset:
    """Derive a strong entity tag so unchanged content revalidates without a transfer."""

    return ValidatedAsset(
        body=body, etag=f'"sha256-{hashlib.sha256(body).hexdigest()}"'
    )


def if_none_match_satisfied(header: str | None, etag: str) -> bool:
    """Return whether an If-None-Match header names the current entity.

    RFC 9110 compares If-None-Match weakly, so a ``W/`` prefix still identifies the same
    content; ``*`` matches any current representation.
    """

    if not header:
        return False
    for candidate in header.split(","):
        tag = candidate.strip()
        if tag == "*":
            return True
        if tag.startswith("W/"):
            tag = tag[2:]
        if tag == etag:
            return True
    return False

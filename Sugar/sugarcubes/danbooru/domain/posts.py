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
"""Danbooru posts as the prompt editor's paste import reads them.

Which pasted links name a post and the prompt text a post gives belong to the prompt editor's
services; the backend only finds the post and reports Danbooru's own tag fields.
"""

from __future__ import annotations

from dataclasses import dataclass
import re

_MD5 = re.compile(r"[0-9a-fA-F]{32}")
_TAG_FIELDS = (
    "tag_string_general",
    "tag_string_artist",
    "tag_string_copyright",
    "tag_string_character",
    "tag_string_meta",
)


@dataclass(frozen=True, slots=True)
class DanbooruPost:
    """A post's id, image md5 and tags by category, as Danbooru's tag strings."""

    post_id: int
    md5: str | None
    tags: tuple[tuple[str, str], ...]

    def to_record(self) -> dict[str, object]:
        """The post in Danbooru's own field names, as the paste import validates them."""
        return {"id": self.post_id, "md5": self.md5, **dict(self.tags)}


def parse_post(payload: object) -> DanbooruPost:
    """Validate one post object from Danbooru's JSON; raise ValueError when it is not one."""
    if not isinstance(payload, dict):
        raise ValueError("A Danbooru post is not an object.")
    record: dict[str, object] = {str(key): value for key, value in payload.items()}
    post_id = record.get("id")
    if not isinstance(post_id, int) or isinstance(post_id, bool) or post_id <= 0:
        raise ValueError("A Danbooru post has no id.")
    tags: list[tuple[str, str]] = []
    for field in _TAG_FIELDS:
        value = record.get(field, "")
        if not isinstance(value, str):
            raise ValueError(f"Danbooru post field {field} is not text.")
        tags.append((field, value))
    md5 = record.get("md5")
    return DanbooruPost(
        post_id=post_id,
        md5=md5.lower() if isinstance(md5, str) and _MD5.fullmatch(md5) else None,
        tags=tuple(tags),
    )

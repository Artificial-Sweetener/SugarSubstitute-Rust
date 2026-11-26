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
"""Prove how a Danbooru post is read for the prompt editor's paste import."""

from __future__ import annotations

import pytest

from sugarcubes.danbooru.domain.posts import parse_post


def test_a_post_keeps_its_tags_by_category_and_a_valid_md5_only() -> None:
    """The record handed to the editor's paste import is Danbooru's own field set."""
    post = parse_post(
        {
            "id": 7,
            "md5": "0123456789ABCDEF0123456789abcdef",
            "rating": "g",
            "tag_string_general": "1girl long_hair",
            "tag_string_meta": "highres",
        }
    )
    assert post.to_record() == {
        "id": 7,
        "md5": "0123456789abcdef0123456789abcdef",
        "tag_string_general": "1girl long_hair",
        "tag_string_artist": "",
        "tag_string_copyright": "",
        "tag_string_character": "",
        "tag_string_meta": "highres",
    }
    assert parse_post({"id": 8, "md5": "short"}).md5 is None


def test_a_post_without_an_id_or_with_damaged_tags_is_refused() -> None:
    """A reply of the wrong shape is never read as a post."""
    payloads: list[object] = [
        [],
        {"tag_string_general": "a"},
        {"id": 0},
        {"id": True},
        {"id": 1, "tag_string_general": 3},
    ]
    for payload in payloads:
        with pytest.raises(ValueError):
            parse_post(payload)

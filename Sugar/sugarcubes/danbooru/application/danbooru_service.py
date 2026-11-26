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
"""Find the Danbooru post a pasted link names.

Pasted links are looked up directly every time, as they were in SugarSubstitute: a paste is rare
and its post may have been retagged since.
"""

from __future__ import annotations

from typing import Protocol

from ..domain.lookup import Lookup, LookupStatus
from ..domain.posts import DanbooruPost


class DanbooruApi(Protocol):
    """The questions the service asks Danbooru."""

    async def post(self, post_id: int) -> Lookup[DanbooruPost]:
        """One post by its id."""
        ...

    async def post_by_md5(self, md5: str) -> Lookup[DanbooruPost]:
        """The post whose image has this md5."""
        ...


class DanbooruService:
    """The prompt editor's one Danbooru owner."""

    def __init__(self, api: DanbooruApi) -> None:
        """Bind Danbooru."""
        self._api = api

    async def pasted_post(
        self, post_id: int | None, md5: str | None
    ) -> Lookup[DanbooruPost]:
        """The post a pasted link names, by id or image md5."""
        if post_id is not None:
            return await self._api.post(post_id)
        if md5 is not None:
            return await self._api.post_by_md5(md5)
        return Lookup(LookupStatus.NOT_FOUND)

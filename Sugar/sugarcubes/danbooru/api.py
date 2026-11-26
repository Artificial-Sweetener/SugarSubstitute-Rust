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
"""The prompt editor's Danbooru route: a thin translation of the Danbooru service."""

from __future__ import annotations

import re

from aiohttp import web

from .application.danbooru_service import DanbooruService

_MD5 = re.compile(r"[0-9a-f]{32}")


def _positive(text: str) -> int | None:
    """A post id, or None when it is not a positive whole number."""
    return int(text) if text.isdigit() and 0 < len(text) <= 12 else None


def register_danbooru_routes(
    routes: web.RouteTableDef, service: DanbooruService
) -> None:
    """Add the Danbooru route to the host's route table."""

    async def pasted_post(request: web.Request) -> web.Response:
        """The post a pasted link names, by `id` or `md5`, as the editor's paste import reads it."""
        md5 = request.query.get("md5", "").lower()
        answer = await service.pasted_post(
            _positive(request.query.get("id", "")),
            md5 if _MD5.fullmatch(md5) else None,
        )
        return web.json_response(
            {
                "status": answer.status.value.replace("_", "-"),
                "post": answer.value.to_record() if answer.value else None,
            },
            headers={"Cache-Control": "no-store"},
        )

    routes.get("/sugarcubes/v1/danbooru/pasted-post")(pasted_post)

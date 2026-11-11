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
"""Honor native Comfy image channel and preview-format requests for registered output files."""

from __future__ import annotations

import asyncio
from io import BytesIO
from pathlib import Path

from aiohttp import web
from PIL import Image


class ManagedImageViews:
    """Bound requested image conversions while serving canonical files without re-encoding."""

    def __init__(self) -> None:
        """Limit concurrent CPU previews without retaining decoded images between requests."""
        self._conversions = asyncio.Semaphore(2)

    async def response(self, request: web.Request, path: Path) -> web.StreamResponse:
        """Preserve native view semantics and keep CPU conversion off the server event loop."""
        preview = request.query.get("preview")
        channel = request.query.get("channel", "")
        if preview is None and channel not in {"rgb", "a"}:
            return web.FileResponse(path)
        async with self._conversions:
            task = asyncio.create_task(
                asyncio.to_thread(_encode_view, path, preview, channel)
            )
            try:
                data, mime_type = await asyncio.shield(task)
            except asyncio.CancelledError:
                await task
                raise
        return web.Response(body=data, content_type=mime_type)


def _encode_view(path: Path, preview: str | None, channel: str) -> tuple[bytes, str]:
    """Apply native RGB, alpha and lossy-thumbnail rules without modifying saved originals."""
    with Image.open(path) as source:
        image: Image.Image = source
        image_format = "PNG"
        quality = 90
        if preview is not None:
            values = preview.split(";")
            image_format = (
                "JPEG" if values[0] == "jpeg" and "a" not in channel else "WEBP"
            )
            if values[-1].isdigit():
                quality = max(1, min(100, int(values[-1])))
            if image_format == "JPEG" or channel == "rgb":
                image = source.convert("RGB")
        elif channel == "rgb":
            image = source.convert("RGB")
        else:
            alpha = (
                source.getchannel("A")
                if source.mode == "RGBA"
                else Image.new("L", source.size, 255)
            )
            image = Image.new("RGBA", source.size)
            image.putalpha(alpha)
        buffer = BytesIO()
        image.save(buffer, format=image_format, quality=quality)
        mime_type = {"PNG": "image/png", "JPEG": "image/jpeg", "WEBP": "image/webp"}[
            image_format
        ]
        return buffer.getvalue(), mime_type

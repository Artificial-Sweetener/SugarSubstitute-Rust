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
"""Expose bounded on-demand image transfers and guarded local desktop reveal requests."""

import asyncio
import logging
import subprocess
from urllib.parse import quote
from collections.abc import Callable, Mapping

from aiohttp import web
from PIL import UnidentifiedImageError, Image

from ..application.media_service import OutputMediaService
from ..domain.media import ImageTransferFormat, ImageTransferPayload
from .local_file_access import local_file_request
from .transfer_references import decode_output_reference, decode_transfer_reference

_LOGGER = logging.getLogger(__name__)


def register_media_routes(
    routes: web.RouteTableDef, service: OutputMediaService
) -> None:
    """Keep HTTP validation and one-at-a-time codec scheduling outside the shared media use cases."""
    codec = asyncio.Semaphore(1)

    async def profile(request: web.Request) -> web.Response:
        """Publish copy preference and a conservative request-qualified reveal capability."""
        format, available = service.profile()
        return web.json_response(
            {
                "success": True,
                "data": {
                    "preferred_copy_format": format.value,
                    "reveal": available
                    and local_file_request(request, require_origin=False),
                },
            },
            headers={"Cache-Control": "no-store"},
        )

    async def image(request: web.Request) -> web.Response:
        """Return an exact or transient rendition without storing another output or blocking the event loop."""
        try:
            if request.method == "GET":
                reference = decode_transfer_reference(dict(request.query))
                raw_format: object = request.query.get("format", "preferred")
            else:
                value: object = await request.json()
                if not isinstance(value, Mapping):
                    raise ValueError("An image transfer request is required.")
                reference = decode_transfer_reference(value.get("file"))
                raw_format = value.get("format", "preferred")
            if not isinstance(raw_format, str):
                raise ValueError("The image transfer format is invalid.")
            format = ImageTransferFormat(raw_format)
            payload = await _run(codec, lambda: service.prepare(reference, format))
            filename = quote(payload.filename, safe="")
            return web.Response(
                body=payload.data,
                content_type=payload.mime_type,
                headers={
                    "Content-Disposition": f"attachment; filename*=UTF-8''{filename}",
                    "X-SugarCubes-Filename": filename,
                    "Cache-Control": "no-store",
                },
            )
        except (UnidentifiedImageError, Image.DecompressionBombError):
            _LOGGER.exception("The requested output is not a readable supported image.")
            return _error(
                ValueError("This output is not a readable supported image."), 400
            )
        except (ValueError, TypeError) as error:
            return _error(error, 400)
        except OSError:
            _LOGGER.exception("An output image rendition could not be prepared.")
            return _error(OSError("The output image could not be prepared."), 500)

    async def preview(request: web.Request) -> web.Response:
        """Serve a small cacheable preview; the URL's revision names one immutable output."""
        try:
            reference = decode_transfer_reference(dict(request.query))
            raw_size = request.query.get("size", "")
            if not raw_size.isdigit():
                raise ValueError("The output preview size is invalid.")
            payload = await _run(
                codec, lambda: service.preview(reference, int(raw_size))
            )
            return web.Response(
                body=payload.data,
                content_type=payload.mime_type,
                headers={"Cache-Control": "private, max-age=86400"},
            )
        except (UnidentifiedImageError, Image.DecompressionBombError):
            _LOGGER.exception("The requested output is not a readable supported image.")
            return _error(
                ValueError("This output is not a readable supported image."), 400
            )
        except (ValueError, TypeError) as error:
            return _error(error, 400)
        except OSError:
            _LOGGER.exception("An output image preview could not be prepared.")
            return _error(OSError("The output preview could not be prepared."), 500)

    async def reveal(request: web.Request) -> web.Response:
        """Require a same-origin loopback POST before asking the desktop adapter to reveal a file."""
        if not local_file_request(request, require_origin=True):
            return _error(
                ValueError(
                    "File reveal is only available for a local same-origin connection."
                ),
                403,
            )
        try:
            value: object = await request.json()
            reference = decode_output_reference(value)
            await asyncio.to_thread(service.reveal, reference)
            return web.json_response({"success": True})
        except (ValueError, TypeError) as error:
            return _error(error, 400)
        except (OSError, subprocess.TimeoutExpired):
            _LOGGER.exception(
                "The file manager could not reveal the requested output image."
            )
            return _error(
                OSError("The image could not be revealed in its folder."), 500
            )

    routes.get("/sugarcubes/output-saving/media-profile")(profile)
    routes.get("/sugarcubes/output-saving/image")(image)
    routes.get("/sugarcubes/output-saving/preview")(preview)
    routes.post("/sugarcubes/output-saving/image")(image)
    routes.post("/sugarcubes/output-saving/reveal")(reveal)


async def _run(
    semaphore: asyncio.Semaphore,
    encode: Callable[[], ImageTransferPayload],
) -> ImageTransferPayload:
    """Keep the codec slot until canceled worker threads finish, preventing overlapping decode allocations."""
    async with semaphore:
        worker = asyncio.create_task(asyncio.to_thread(encode))
        try:
            return await asyncio.shield(worker)
        except asyncio.CancelledError:
            await asyncio.shield(worker)
            raise


def _error(error: Exception, status: int) -> web.Response:
    """Preserve literal failures for the existing canvas error and toast owners."""
    return web.json_response(
        {"success": False, "error": {"message": str(error)}}, status=status
    )

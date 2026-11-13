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
"""Expose same-origin bounded file receipts and exact re-resolution through the artifact service."""

import asyncio
import json
import logging

from aiohttp import web

from ..application.artifact_service import OutputArtifactService
from ..domain.artifact import ArtifactUnavailable
from .artifact_codec import encode_receipt, expected_request, receipt_request
from .artifact_workers import ArtifactWorkers
from .local_file_access import local_file_request

_LOGGER = logging.getLogger(__name__)
_MAX_REQUEST_BYTES = 16384


def register_artifact_routes(
    routes: web.RouteTableDef, service: OutputArtifactService, workers: ArtifactWorkers
) -> None:
    """Register thin typed operations; lifecycle teardown is attached by the host composition."""

    async def inspect(request: web.Request) -> web.Response:
        """Validate external JSON before admitting any filesystem or hashing operation."""
        if not local_file_request(request, require_origin=True):
            return web.json_response(
                {
                    "available": False,
                    "code": "invalid_reference",
                    "message": "Output inspection requires the current local frontend origin.",
                },
                status=403,
            )
        try:
            if (
                request.content_length is None
                or not 1 <= request.content_length <= _MAX_REQUEST_BYTES
                or request.content_type != "application/json"
            ):
                raise ArtifactUnavailable(
                    "invalid_reference",
                    "The output inspection request exceeds its bounds.",
                )
            body = await request.content.readexactly(request.content_length)
            value: object = json.loads(body)
            if request.path.endswith("artifact-receipt"):
                file = receipt_request(value)
                receipt = await workers.run(
                    lambda budget: service.receipt(file, budget)
                )
            else:
                expected = expected_request(value)
                receipt = await workers.run(
                    lambda budget: service.resolve(expected, budget)
                )
            return web.json_response(
                encode_receipt(receipt), headers={"Cache-Control": "no-store"}
            )
        except ArtifactUnavailable as error:
            return web.json_response(
                {"available": False, "code": error.code, "message": str(error)},
                headers={"Cache-Control": "no-store"},
            )
        except (
            ValueError,
            TypeError,
            UnicodeError,
            RecursionError,
            asyncio.IncompleteReadError,
        ):
            return web.json_response(
                {
                    "available": False,
                    "code": "invalid_reference",
                    "message": "The output inspection request is invalid.",
                },
                status=400,
            )
        except OSError:
            _LOGGER.exception("Output artifact inspection failed.")
            return web.json_response(
                {
                    "available": False,
                    "code": "inaccessible",
                    "message": "The output artifact could not be inspected.",
                },
                status=500,
            )

    routes.post("/sugarcubes/output-saving/artifact-receipt")(inspect)
    routes.post("/sugarcubes/output-saving/artifact-resolve")(inspect)

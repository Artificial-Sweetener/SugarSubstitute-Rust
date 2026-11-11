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
"""Translate complete cart requests into one canonical Cube stack plan."""

from __future__ import annotations

from collections.abc import Mapping
from math import isfinite
from typing import Any

from ..cube_model import CubeDocument, CubeSchemaError
from ..execution import CubeExecutionError
from ..workflow import CanonicalWorkflowError
from ..workflow_mutation_errors import CubeGraphMutationError
from ..workflow_stack_mutation import (
    CubeStackEntry,
    CubeStackMutationService,
    ExistingCubeStackEntry,
    NewCubeStackEntry,
)
from .composition import BackendServices
from .responses import BackendError, json_success
from .route_types import RouteHandler
from .workflow_analysis_routes import workflow_analysis_response
from .workflow_mutation_boundary import (
    read_workflow_mutation_body,
    required_mutation_text,
    workflow_mutation_error,
)


def build_workflow_stack_handler(services: BackendServices) -> RouteHandler:
    """Compose one plan owner using the existing canonical graph services."""

    planner = CubeStackMutationService(
        services.workflow_analysis, services.workflow_mutation
    )

    async def apply_stack(request: Any) -> Any:
        """Return a complete plan while the browser retains native commit ownership."""

        try:
            body = await read_workflow_mutation_body(request)
            result = planner.apply(body["workflow"], _entries(body.get("entries")))
            return json_success(workflow_analysis_response(result))
        except (
            BackendError,
            CanonicalWorkflowError,
            CubeExecutionError,
            CubeGraphMutationError,
        ) as error:
            return workflow_mutation_error(error)

    return apply_stack


def _entries(value: object) -> tuple[CubeStackEntry, ...]:
    """Convert dynamic references and portable documents to typed cart entries."""

    if not isinstance(value, list):
        raise BackendError("Cube cart entries must be an array", status=400)
    entries: list[CubeStackEntry] = []
    for entry in value:
        if not isinstance(entry, Mapping):
            raise BackendError("Cube cart entry must be an object", status=400)
        instance_id = required_mutation_text(entry.get("instance_id"), "instance_id")
        if entry.get("source") == "existing":
            entries.append(ExistingCubeStackEntry(instance_id))
        elif entry.get("source") == "new":
            document = entry.get("document")
            if not isinstance(document, Mapping):
                raise BackendError("Cube cart document must be an object", status=400)
            try:
                parsed = CubeDocument.from_dict(document)
            except (CubeSchemaError, TypeError, ValueError) as error:
                raise BackendError(
                    f"Cube document is invalid: {error}", status=422
                ) from error
            entries.append(
                NewCubeStackEntry(
                    instance_id,
                    required_mutation_text(entry.get("alias"), "alias"),
                    parsed,
                    _size(entry.get("size")),
                )
            )
        else:
            raise BackendError(
                "Cube cart entry source must be existing or new", status=400
            )
    return tuple(entries)


def _size(value: object) -> tuple[float, float]:
    """Require an exact finite native frame pair before it reaches geometry policy."""

    if not isinstance(value, list) or len(value) != 2:
        raise BackendError("Cube cart size must contain two finite numbers", status=400)
    width, height = value
    if (
        isinstance(width, bool)
        or not isinstance(width, int | float)
        or isinstance(height, bool)
        or not isinstance(height, int | float)
        or not isfinite(width)
        or not isfinite(height)
    ):
        raise BackendError("Cube cart size must contain two finite numbers", status=400)
    return float(width), float(height)

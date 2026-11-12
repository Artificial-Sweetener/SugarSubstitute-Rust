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
"""Validate shared workflow mutation envelopes and preserve structured HTTP failures."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from ..execution import CubeExecutionError
from ..workflow import CanonicalWorkflowError
from .responses import BackendError, json_error_from_exception
from .validation import parse_json_body


async def read_workflow_mutation_body(request: Any) -> Mapping[str, Any]:
    """Validate the shared workflow-mutation request envelope."""

    body = await parse_json_body(request)
    if body.get("schema_version") != 1:
        raise BackendError("Unsupported workflow mutation schema", status=400)
    if "workflow" not in body:
        raise BackendError("'workflow' field is required", status=400)
    return body


def mutation_instance_ids(value: object) -> tuple[str, ...]:
    """Require one non-empty string array at the HTTP boundary."""

    if not isinstance(value, list) or not all(
        isinstance(item, str) and item.strip() for item in value
    ):
        raise BackendError("Workflow mutation instance ids must be strings", status=400)
    return tuple(item.strip() for item in value)


def required_mutation_text(value: object, field: str) -> str:
    """Require one non-empty mutation field."""

    if not isinstance(value, str) or not value.strip():
        raise BackendError(f"Cube {field} must be a non-empty string", status=400)
    return value.strip()


def workflow_mutation_error(error: Exception) -> Any:
    """Preserve structured workflow errors across every mutation route."""

    if isinstance(error, BackendError):
        return json_error_from_exception(error)
    if isinstance(error, CanonicalWorkflowError):
        return json_error_from_exception(
            BackendError(
                str(error),
                status=422,
                extra={"code": error.code, "phase": "workflow", "path": error.path},
            )
        )
    if isinstance(error, CubeExecutionError):
        return json_error_from_exception(
            BackendError(
                str(error),
                status=422,
                extra={"code": error.code, "phase": error.phase},
            )
        )
    return json_error_from_exception(BackendError(str(error), status=422))

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
"""Adapt SugarScript workflow authoring to a bounded JSON endpoint."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any
from ..authoring.composed_workflow_service import exact_integer_fields
from ..authoring.workflow_sugarscript_projection import (
    WorkflowScriptProjectionError,
    project_workflow_script,
)
from ..cube_model import CubeSchemaError
from ..language.substitute_writer import write_substitute_script
from .composition import BackendServices
from .responses import BackendError, json_error_from_exception, json_success
from .route_types import RouteHandler
from .validation import parse_json_body
from .workflow_analysis_routes import workflow_analysis_response
from .workflow_plan_responses import serialize_sugarscript_result

_MAX_SOURCE_BYTES = 1_000_000


def _document(payload: Mapping[str, object]) -> Mapping[str, object]:
    """Read the Cube document one prepared import carries."""

    document = payload.get("document")
    return document if isinstance(document, Mapping) else {}


@dataclass(frozen=True)
class SugarScriptRouteHandlers:
    """Collect the additive SugarScript route family."""

    compile_sugarscript: RouteHandler
    render_sugarscript: RouteHandler


def build_sugarscript_route_handlers(
    services: BackendServices,
) -> SugarScriptRouteHandlers:
    """Build a thin authoring endpoint over the application service."""

    async def compile_sugarscript(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            source = body.get("source")
            if not isinstance(source, str):
                raise BackendError("'source' field is required", status=400)
            if len(source.encode("utf-8")) > _MAX_SOURCE_BYTES:
                raise BackendError("SugarScript source exceeds 1 MB", status=413)
            # A stored script has Windows line endings; its values hold plain newlines.
            result = services.sugarscript_authoring.compile(
                source.replace("\r\n", "\n").replace("\r", "\n")
            )
            payload = serialize_sugarscript_result(result)
            workflow = payload.get("workflow")
            payload["analysis"] = (
                workflow_analysis_response(services.workflow_analysis.analyze(workflow))
                if result.is_valid and workflow is not None
                else None
            )
            if isinstance(payload["analysis"], dict):
                payload["workflow"] = payload["analysis"]["workflow"]
            # A page holds numbers as doubles; it is told the digits of each it would round.
            payload["exact_integers"] = [
                {
                    "instance_id": field.instance_id,
                    "node_symbol": field.node_symbol,
                    "input_name": field.input_name,
                    "digits": field.digits,
                }
                for instance in (result.plan.instances if result.plan else ())
                for field in exact_integer_fields(
                    instance.instance_id, _document(instance.payload)
                )
            ]
            return json_success(
                payload,
                status=200 if result.is_valid else 422,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def render_sugarscript(request: Any) -> Any:
        """Write the project script of one native workflow, without storing it."""

        try:
            body = await parse_json_body(request)
            workflow = body.get("workflow")
            if not isinstance(workflow, dict):
                raise BackendError("'workflow' field is required", status=400)
            try:
                script = project_workflow_script(
                    services.workflow_analysis.analyze(workflow)
                )
                source = write_substitute_script(script)
            except (WorkflowScriptProjectionError, CubeSchemaError) as error:
                raise BackendError(str(error), status=422) from error
            return json_success({"source": source})
        except BackendError as error:
            return json_error_from_exception(error)

    return SugarScriptRouteHandlers(
        compile_sugarscript=compile_sugarscript,
        render_sugarscript=render_sugarscript,
    )

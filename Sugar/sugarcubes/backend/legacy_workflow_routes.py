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
#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Adapt exact-version legacy workflow reconciliation to JSON."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
import json
from typing import Any

from ..authoring import ComposedWorkflowImportError, LegacyWorkflowImportError
from .composition import BackendServices
from .responses import BackendError, json_error_from_exception, json_success
from .route_types import RouteHandler
from .validation import parse_json_body
from .workflow_plan_responses import serialize_native_workflow_plan


@dataclass(frozen=True)
class LegacyWorkflowRouteHandlers:
    """Collect the legacy workflow authoring route family."""

    compile_legacy_workflow: RouteHandler


def build_legacy_workflow_route_handlers(
    services: BackendServices,
) -> LegacyWorkflowRouteHandlers:
    """Build a thin endpoint over exact-version workflow reconciliation."""

    async def compile_legacy_workflow(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            workflow = _read_workflow(body)
            exact_integers: list[dict[str, str]] = []
            try:
                composed = services.composed_workflow_authoring
                if composed.recognizes(workflow):
                    rebuilt = composed.compile(workflow)
                    plan = rebuilt.plan
                    exact_integers = [
                        {
                            "instance_id": field.instance_id,
                            "node_symbol": field.node_symbol,
                            "input_name": field.input_name,
                            "digits": field.digits,
                        }
                        for field in rebuilt.exact_integers
                    ]
                else:
                    plan = services.legacy_workflow_authoring.compile(workflow)
            except (ComposedWorkflowImportError, LegacyWorkflowImportError) as error:
                raise BackendError(str(error), status=422) from error
            return json_success(
                {
                    "valid": True,
                    "plan": serialize_native_workflow_plan(plan),
                    "diagnostics": [],
                    "exact_integers": exact_integers,
                },
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    return LegacyWorkflowRouteHandlers(compile_legacy_workflow=compile_legacy_workflow)


def _read_workflow(body: Mapping[str, Any]) -> Mapping[str, object]:
    """Read the workflow from its saved text when the page sends it, else as an object.

    A page parses numbers as doubles, so an object it sends has already lost
    the low digits of a 64-bit seed. The saved text is parsed here instead and
    keeps every whole number exact.
    """

    text = body.get("workflow_text")
    if isinstance(text, str):
        try:
            workflow = json.loads(text)
        except ValueError as error:
            raise BackendError(
                "'workflow_text' is not valid JSON", status=400
            ) from error
    else:
        workflow = body.get("workflow")
    if not isinstance(workflow, Mapping):
        raise BackendError("'workflow' object is required", status=400)
    return workflow

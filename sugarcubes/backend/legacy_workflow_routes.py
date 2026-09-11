#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Adapt exact-version legacy workflow reconciliation to JSON."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any

from ..authoring import LegacyWorkflowImportError
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
            workflow = body.get("workflow")
            if not isinstance(workflow, Mapping):
                raise BackendError("'workflow' object is required", status=400)
            try:
                plan = services.legacy_workflow_authoring.compile(workflow)
            except LegacyWorkflowImportError as error:
                raise BackendError(str(error), status=422) from error
            return json_success(
                {
                    "valid": True,
                    "plan": serialize_native_workflow_plan(plan),
                    "diagnostics": [],
                },
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    return LegacyWorkflowRouteHandlers(compile_legacy_workflow=compile_legacy_workflow)

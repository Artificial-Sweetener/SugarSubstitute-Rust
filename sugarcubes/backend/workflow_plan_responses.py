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
"""Serialize native workflow authoring plans for every source adapter."""

from __future__ import annotations

from typing import Any

from ..authoring import (
    NativeWorkflowAuthoringResult,
    NativeWorkflowImportPlan,
    project_native_workflow_plan,
)
from ..language.source import SugarScriptDiagnostic


def serialize_native_workflow_plan(plan: NativeWorkflowImportPlan) -> dict[str, Any]:
    """Serialize one host-neutral native workflow mutation plan."""

    return {
        "semantic_hash": plan.semantic_hash,
        "instances": [
            {
                "instance_id": instance.instance_id,
                "alias": instance.alias,
                "bypassed": instance.bypassed,
                "payload": instance.payload,
            }
            for instance in plan.instances
        ],
        "connections": [
            {
                "source_instance_id": connection.source_instance_id,
                "source_binding": connection.source_binding,
                "target_instance_id": connection.target_instance_id,
                "target_binding": connection.target_binding,
            }
            for connection in plan.connections
        ],
    }


def serialize_sugarscript_result(
    result: NativeWorkflowAuthoringResult,
) -> dict[str, Any]:
    """Serialize one language result without leaking source text."""

    return {
        "valid": result.is_valid,
        "plan": (
            serialize_native_workflow_plan(result.plan)
            if result.plan is not None
            else None
        ),
        "workflow": (
            project_native_workflow_plan(result.plan)
            if result.plan is not None
            else None
        ),
        "diagnostics": [serialize_diagnostic(item) for item in result.diagnostics],
    }


def serialize_diagnostic(diagnostic: SugarScriptDiagnostic) -> dict[str, object]:
    """Serialize one stable diagnostic and its half-open source span."""

    return {
        "code": diagnostic.code,
        "severity": diagnostic.severity.value,
        "message": diagnostic.message,
        "span": {
            "start": {
                "offset": diagnostic.span.start.offset,
                "line": diagnostic.span.start.line,
                "column": diagnostic.span.start.column,
            },
            "end": {
                "offset": diagnostic.span.end.offset,
                "line": diagnostic.span.end.line,
                "column": diagnostic.span.end.column,
            },
        },
    }

#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Verify the exact-version legacy workflow compile endpoint."""

from __future__ import annotations

import asyncio
from collections.abc import Mapping
from dataclasses import replace
from pathlib import Path
from typing import cast

from sugarcubes.authoring import (
    LegacyWorkflowAuthoringService,
    LegacyWorkflowImportError,
    NativeCubeImport,
    NativeWorkflowImportPlan,
)
from sugarcubes.backend.routes import build_route_handlers
from tests.backend_api.support.backend_fixtures import (
    FakeRequest,
    decode_json_response,
)
from tests.backend_api.support.typing_support import BackendServicesFactory


class _SuccessfulCompiler:
    """Return one deterministic reconciled plan."""

    def compile(self, workflow: Mapping[str, object]) -> NativeWorkflowImportPlan:
        """Prove the route passes the workflow object without transforming it."""

        assert workflow["version"] == 0.4
        return NativeWorkflowImportPlan(
            "a" * 64,
            (
                NativeCubeImport(
                    "first",
                    "First",
                    False,
                    {"document": {"cube_id": "owner/repo/demo.cube"}},
                ),
            ),
            (),
        )


class _FailingCompiler:
    """Reject a workflow whose historical schema is unavailable."""

    def compile(self, _workflow: Mapping[str, object]) -> NativeWorkflowImportPlan:
        """Expose one actionable fail-closed diagnostic."""

        raise LegacyWorkflowImportError("exact version 1.0.0 is unavailable")


def test_legacy_workflow_route_returns_native_plan(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Serialize the same native plan shape used by SugarScript."""

    services = replace(
        backend_services_factory(tmp_path),
        legacy_workflow_authoring=cast(
            LegacyWorkflowAuthoringService,
            _SuccessfulCompiler(),
        ),
    )

    response = asyncio.run(
        build_route_handlers(services).compile_legacy_workflow(
            FakeRequest(body={"workflow": {"version": 0.4}})
        )
    )

    assert response.status == 200
    body = _mapping(decode_json_response(response))
    plan = _mapping(body["plan"])
    instances = plan["instances"]
    assert isinstance(instances, list)
    assert _mapping(instances[0])["alias"] == "First"


def test_legacy_workflow_route_rejects_ambiguous_or_missing_payloads(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Return bounded client errors without ever providing a partial plan."""

    services = replace(
        backend_services_factory(tmp_path),
        legacy_workflow_authoring=cast(
            LegacyWorkflowAuthoringService,
            _FailingCompiler(),
        ),
    )
    handler = build_route_handlers(services).compile_legacy_workflow

    invalid = asyncio.run(handler(FakeRequest(body={"workflow": []})))
    ambiguous = asyncio.run(handler(FakeRequest(body={"workflow": {"version": 0.4}})))

    assert invalid.status == 400
    assert ambiguous.status == 422
    assert "exact version" in str(_mapping(decode_json_response(ambiguous))["error"])


def _mapping(value: object) -> Mapping[str, object]:
    """Narrow one decoded response object."""

    assert isinstance(value, Mapping)
    return value

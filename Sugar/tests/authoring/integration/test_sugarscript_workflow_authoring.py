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
"""Verify SugarScript plans cross the normal native Cube import boundary."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Mapping, Protocol

from sugarcubes.authoring import SugarScriptWorkflowAuthoringService
from sugarcubes.cube_model import CubeDocument
from sugarcubes.language import SugarScriptLanguageService


@dataclass(frozen=True)
class _Resolver:
    """Resolve one immutable validated fixture Cube."""

    document: CubeDocument

    def resolve(self, cube_id: str, version_pin: str | None) -> CubeDocument:
        """Resolve the fixture identity and enforce its version."""

        if cube_id != self.document.cube_id:
            raise ValueError(f"Cube '{cube_id}' is unavailable.")
        if version_pin is not None and version_pin != self.document.version:
            raise ValueError(f"Version '{version_pin}' is unavailable.")
        return self.document


@dataclass
class _Preparer:
    """Record complete compiler documents before returning native payloads."""

    seen: list[CubeDocument] = field(default_factory=list)

    def prepare(self, document: CubeDocument) -> Mapping[str, object]:
        """Return a minimal host-neutral prepared import or inject failure."""

        self.seen.append(document)
        return {"cube": {"cube_id": document.cube_id, "version": document.version}}


class _PreparerPort(Protocol):
    """Describe the structural application port accepted by the use case."""

    def prepare(self, document: CubeDocument) -> Mapping[str, object]:
        """Prepare one compiler-materialized Cube."""


def test_authoring_service_returns_complete_plan_without_host_mutation() -> None:
    """Prepare every instance and retain warnings without treating them as errors."""

    document = _document()
    preparer = _Preparer()
    service = _service(document, preparer)

    result = service.compile(
        f'# seed_control {{"alias":\nuse "{document.cube_id}" as Cube repeat 2\n'
    )

    assert result.is_valid
    assert result.plan is not None
    assert [instance.alias for instance in result.plan.instances] == ["Cube1", "Cube2"]
    assert len(preparer.seen) == 2
    assert [diagnostic.severity.value for diagnostic in result.diagnostics] == [
        "warning"
    ]


def test_authoring_service_rejects_entire_plan_when_one_import_cannot_prepare() -> None:
    """Prevent partial host plans when any compiled Cube fails normal preparation."""

    document = _document()

    class _FailingPreparer:
        """Reject the second preparation after recording the attempt."""

        def __init__(self) -> None:
            self.calls = 0

        def prepare(self, compiled: CubeDocument) -> Mapping[str, object]:
            """Inject a bounded deterministic failure on the second call."""

            self.calls += 1
            if self.calls == 2:
                raise ValueError("injected preparation failure")
            return {"document": compiled.to_dict()}

    failing = _FailingPreparer()
    failed = _service(document, failing).compile(
        f'use "{document.cube_id}" as First\nuse "{document.cube_id}" as Second\n'
    )

    assert failed.plan is None
    assert failing.calls == 2
    assert [diagnostic.code for diagnostic in failed.diagnostics] == [
        "sugarscript.prepare.invalid_cube"
    ]
    assert failed.diagnostics[0].span.start.line == 2


def _service(
    document: CubeDocument,
    preparer: _PreparerPort,
) -> SugarScriptWorkflowAuthoringService:
    """Compose the application use case around test ports."""

    return SugarScriptWorkflowAuthoringService(
        language=SugarScriptLanguageService(),
        resolver=_Resolver(document),
        preparer=preparer,
    )


def _document() -> CubeDocument:
    """Build one minimal canonical Cube accepted by the normal importer."""

    return CubeDocument.from_dict(
        {
            "cube_id": "local/tests/authoring.cube",
            "version": "1.0.0",
            "description": "Authoring fixture",
            "metadata": {},
            "implementation": {
                "nodes": {
                    "value": {
                        "class_type": "PrimitiveInt",
                        "label": "Value",
                        "inputs": {"value": 1},
                    }
                },
                "inputs": {},
                "outputs": {},
                "layout": {},
                "definitions": {},
                "subgraphs": [],
            },
            "surface": {"default_flavor_id": "default", "controls": []},
            "flavors": {
                "authored": [{"id": "default", "name": "Default", "values": {}}]
            },
        }
    )

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
"""Verify SugarScript authors the same prepared native Cube payloads as files."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass

from sugarcubes.authoring import SugarScriptWorkflowAuthoringService
from sugarcubes.backend.services import NativeCubeImportPreparerAdapter
from sugarcubes.cube_model import CubeDocument
from sugarcubes.importer import load_cube_document, prepare_import
from sugarcubes.language import SugarScriptLanguageService


@dataclass(frozen=True)
class _Resolver:
    """Return one fixed validated document through the language port."""

    document: CubeDocument

    def resolve(self, cube_id: str, version_pin: str | None) -> CubeDocument:
        """Resolve the fixture only when identity and version match."""

        if cube_id != self.document.cube_id:
            raise ValueError(f"Unknown Cube '{cube_id}'.")
        if version_pin not in {None, self.document.version}:
            raise ValueError(f"Unknown version '{version_pin}'.")
        return self.document


def test_service_prepares_complete_native_imports_without_mutating_a_graph() -> None:
    """Prepare instances and links atomically through the canonical importer."""

    document = _document()
    service = SugarScriptWorkflowAuthoringService(
        language=SugarScriptLanguageService(),
        resolver=_Resolver(document),
        preparer=NativeCubeImportPreparerAdapter(
            load_document=lambda value: load_cube_document(value),
            prepare_import=lambda loaded: prepare_import(loaded),
        ),
    )
    source = (
        f'use "{document.cube_id}" as Left repeat 2\n'
        "set Left1.sampler.steps = 18\n"
        "connect Left1.output.image to Left2.input.image\n"
    )

    result = service.compile(source)

    assert result.plan is not None
    assert result.diagnostics == ()
    assert [instance.alias for instance in result.plan.instances] == ["Left1", "Left2"]
    assert len(result.plan.connections) == 1
    first_payload = result.plan.instances[0].payload
    document_payload = _mapping(first_payload["document"])
    implementation = _mapping(document_payload["implementation"])
    nodes = _mapping(implementation["nodes"])
    sampler = _mapping(nodes["sampler"])
    assert _mapping(sampler["inputs"])["steps"] == 18
    assert _mapping(first_payload["cube"])["cube_id"] == document.cube_id
    boundaries = _mapping(first_payload["boundaries"])
    boundary_inputs = boundaries["inputs"]
    boundary_outputs = boundaries["outputs"]
    assert isinstance(boundary_inputs, list)
    assert isinstance(boundary_outputs, list)
    assert _mapping(boundary_inputs[0])["name"] == "input.image"
    assert _mapping(boundary_outputs[0])["name"] == "output.image"


def test_preparation_failure_returns_a_located_diagnostic_and_no_partial_plan() -> None:
    """Keep host mutation unavailable when any Cube cannot enter native construction."""

    document = _document()

    class _FailingPreparer:
        """Reject every document at the importer boundary."""

        def prepare(self, _document: CubeDocument) -> dict[str, object]:
            """Simulate a validated importer failure."""

            raise ValueError("unsafe embedded definition")

    service = SugarScriptWorkflowAuthoringService(
        language=SugarScriptLanguageService(),
        resolver=_Resolver(document),
        preparer=_FailingPreparer(),
    )

    result = service.compile(f'use "{document.cube_id}" as Broken\n')

    assert result.plan is None
    assert result.diagnostics[0].code == "sugarscript.prepare.invalid_cube"
    assert result.diagnostics[0].span.start.line == 1


def _document() -> CubeDocument:
    """Build one current-format Cube with native public boundaries."""

    cube_id = "artificial-sweetener/base-cubes/contract.cube"
    return CubeDocument.from_dict(
        {
            "cube_id": cube_id,
            "version": "1.0.0",
            "metadata": {"default_alias": "contract"},
            "implementation": {
                "nodes": {
                    "sampler": {
                        "class_type": "KSampler",
                        "inputs": {"image": None, "steps": 10},
                    }
                },
                "inputs": {
                    "input.image": {
                        "kind": "input",
                        "targets": [["sampler", "image"]],
                    }
                },
                "outputs": {"output.image": ["sampler", 0]},
                "layout": {"origin": [0, 0], "ds": {}, "nodes": {}, "markers": {}},
                "definitions": {},
                "subgraphs": [],
            },
            "surface": {
                "default_flavor_id": "default",
                "controls": [
                    {
                        "control_id": "sampler.steps",
                        "symbol": "sampler",
                        "input_name": "steps",
                        "label": "Steps",
                        "class_type": "KSampler",
                        "value_type": "number",
                    }
                ],
            },
            "flavors": {
                "authored": [{"id": "default", "name": "Default", "values": {}}]
            },
        }
    )


def _mapping(value: object) -> Mapping[str, object]:
    """Narrow one serialized JSON object for strict contract assertions."""

    assert isinstance(value, Mapping)
    assert all(isinstance(key, str) for key in value)
    return value

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
"""Verify a native Cube workflow is written back as the project script that built it."""

from __future__ import annotations

from collections.abc import Mapping
from copy import deepcopy
from typing import cast

import pytest

from sugarcubes.authoring import SugarScriptWorkflowAuthoringService
from sugarcubes.authoring.native_workflow_projection import (
    project_native_workflow_plan,
)
from sugarcubes.authoring.workflow_sugarscript_projection import (
    WorkflowScriptProjectionError,
    project_workflow_script,
)
from sugarcubes.backend.services import NativeCubeImportPreparerAdapter
from sugarcubes.importer import load_cube_document, prepare_import
from sugarcubes.language import SugarScriptLanguageService
from sugarcubes.language.substitute_writer import write_substitute_script
from sugarcubes.workflow_analysis import CubeGraphAnalysisService

from .test_sugarscript_native_workflow_authoring import _Resolver, _document


def _workflow(source: str) -> dict[str, object]:
    """Build the native workflow one script compiles to."""

    document = _document()
    service = SugarScriptWorkflowAuthoringService(
        language=SugarScriptLanguageService(),
        resolver=_Resolver(document),
        preparer=NativeCubeImportPreparerAdapter(
            load_document=lambda value: load_cube_document(value),
            prepare_import=lambda loaded: prepare_import(loaded),
        ),
    )
    result = service.compile(source)
    assert result.diagnostics == ()
    assert result.plan is not None
    return project_native_workflow_plan(result.plan)


def _written(workflow: Mapping[str, object]) -> str:
    """Write the project script of one workflow."""

    return write_substitute_script(
        project_workflow_script(CubeGraphAnalysisService().analyze(workflow))
    )


def _source() -> str:
    """Return a script in persisted form: two Cubes, one bypassed, values and a connection."""

    document = _document()
    use = f'use "{document.cube_id}"@{document.version}'
    return (
        f"{use} as Left\n"
        f"# bypass {use} as Right\n"
        "\n"
        "set Left.sampler.Steps = 18\n"
        "\n"
        "# bypass set Right.sampler.Steps = 7\n"
        "\n"
        "connect Left.output.image to Right.input.image\n"
    )


def test_a_workflow_is_written_back_as_the_script_that_built_it() -> None:
    """Keep Cubes, order, pins, bypass, exposed values and connections through a round trip."""

    assert _written(_workflow(_source())) == _source()


def test_only_exposed_fields_are_written_and_each_under_the_name_a_person_sees() -> (
    None
):
    """Persist a Cube's surface, never its internal graph values."""

    document = _document()
    script = project_workflow_script(
        CubeGraphAnalysisService().analyze(_workflow(_source()))
    )
    exposed = {
        (control.symbol, control.input_name): control.label
        for control in document.surface.controls
    }

    written = {
        (node.key, field.input_key): field.input_label
        for node in script.cubes[0].nodes
        for field in node.fields
    }
    assert written == exposed
    assert [cube.alias for cube in script.cubes] == ["Left", "Right"]
    assert [cube.bypassed for cube in script.cubes] == [False, True]
    assert all(cube.version_pin == document.version for cube in script.cubes)


def test_a_cube_whose_outputs_are_not_saved_says_so_in_the_script() -> None:
    """Carry the workflow-local output policy of one Cube."""

    workflow = deepcopy(_workflow(_source()))
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    cast(dict[str, object], nodes[1]["properties"])["sugarcubes_save_outputs"] = False

    assert '# cube_output_persistence {"alias":"Right","saved":false}' in _written(
        workflow
    )


def test_two_cubes_with_one_name_cannot_be_written_as_a_script() -> None:
    """Refuse a workflow whose Cubes a script could not tell apart."""

    workflow = deepcopy(_workflow(_source()))
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    for node in nodes:
        properties = cast(dict[str, object], node["properties"])
        cast(dict[str, object], properties["sugarcubes_cube"])[
            "instance_alias"
        ] = "Same"

    with pytest.raises(WorkflowScriptProjectionError, match="named 'Same'"):
        _written(workflow)

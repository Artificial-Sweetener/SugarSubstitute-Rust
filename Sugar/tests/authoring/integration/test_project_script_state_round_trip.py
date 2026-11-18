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
"""Verify a project script's editor state survives being opened and saved again.

A script says more than values: one value for every Cube, a node that follows another Cube's
node, a seed held fixed, a Cube whose outputs are not saved. Each of these is opened into a
workflow and written back, from the workflow as compiled and from the workflow as the page
keeps it.
"""

from __future__ import annotations

import json
from collections.abc import Mapping
from copy import deepcopy
from typing import cast

from sugarcubes.authoring import SugarScriptWorkflowAuthoringService
from sugarcubes.authoring.native_workflow_projection import (
    project_native_workflow_plan,
)
from sugarcubes.authoring.workflow_sugarscript_projection import (
    project_workflow_script,
)
from sugarcubes.backend.services import NativeCubeImportPreparerAdapter
from sugarcubes.cube_model import CubeDocument
from sugarcubes.importer import load_cube_document, prepare_import
from sugarcubes.language import SugarScriptLanguageService
from sugarcubes.language.substitute_writer import write_substitute_script
from sugarcubes.workflow_analysis import CubeGraphAnalysisService

_CUBE_ID = "artificial-sweetener/base-cubes/state.cube"
_USE = f'use "{_CUBE_ID}"@1.0.0'
_WHOLE_SEED = 12345678901234567890


class _Resolver:
    """Resolve the one Cube these scripts use."""

    def resolve(self, cube_id: str, version_pin: str | None) -> CubeDocument:
        """Return the Cube for its id."""

        assert cube_id == _CUBE_ID
        assert version_pin == "1.0.0"
        return _document()


def _document() -> CubeDocument:
    """Build a Cube with a prompt, a sampler, and one sampler value its face does not show."""

    def control(symbol: str, name: str, class_type: str) -> dict[str, object]:
        return {
            "control_id": f"{symbol}.{name}",
            "symbol": symbol,
            "input_name": name,
            "label": name,
            "class_type": class_type,
            "value_type": "string" if name in {"value", "sampler_name"} else "number",
        }

    return CubeDocument.from_dict(
        {
            "cube_id": _CUBE_ID,
            "version": "1.0.0",
            "metadata": {"default_alias": "state"},
            "implementation": {
                "nodes": {
                    "prompt": {
                        "class_type": "PrimitiveStringMultiline",
                        "label": "positive prompt",
                        "inputs": {"value": ""},
                    },
                    "sampler": {
                        "class_type": "KSampler",
                        "inputs": {
                            "image": None,
                            "seed": 0,
                            "sampler_name": "euler",
                            "steps": 10,
                            "cfg": 7.0,
                        },
                    },
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
                    control("prompt", "value", "PrimitiveStringMultiline"),
                    control("sampler", "seed", "KSampler"),
                    control("sampler", "sampler_name", "KSampler"),
                    control("sampler", "steps", "KSampler"),
                ],
            },
            "flavors": {
                "authored": [{"id": "default", "name": "Default", "values": {}}]
            },
        }
    )


def _workflow(source: str) -> dict[str, object]:
    """Build the native workflow one script compiles to."""

    service = SugarScriptWorkflowAuthoringService(
        language=SugarScriptLanguageService(),
        resolver=_Resolver(),
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


def _inputs(workflow: Mapping[str, object], cube: int, node: str) -> dict[str, object]:
    """Read the values one node of one Cube holds in a compiled workflow."""

    definitions = cast(dict[str, object], workflow["definitions"])
    subgraph = cast(list[dict[str, object]], definitions["subgraphs"])[cube]
    document = cast(dict[str, object], subgraph["extra"])["sugarcubes_document"]
    implementation = cast(dict[str, object], document)["implementation"]
    nodes = cast(dict[str, object], implementation)["nodes"]
    held = cast(dict[str, object], cast(dict[str, object], nodes)[node])["inputs"]
    return cast(dict[str, object], held)


def _instance_ids(workflow: Mapping[str, object]) -> list[str]:
    """Read the Cube instance identities of a compiled workflow, in stack order."""

    ids: list[str] = []
    for node in cast(list[dict[str, object]], workflow["nodes"]):
        properties = cast(dict[str, object], node["properties"])
        cube = cast(dict[str, object], properties["sugarcubes_cube"])
        ids.append(str(cube["instance_id"]))
    return ids


def _script(*sections: str) -> str:
    """Join script sections the way a project script separates them."""

    return "\n\n".join(sections) + "\n"


_CUBES = f"{_USE} as Left\n{_USE} as Right"
_SHARED = f'set *.*.sampler_name = "dpmpp_2m"\nset *.*.seed = {_WHOLE_SEED}'
_LEFT = 'set Left."positive prompt".value = "left text"\nset Left.sampler.steps = 18'
_RIGHT = 'set Right."positive prompt".value = "right text"\nset Right.sampler.steps = 7'


def test_a_value_given_to_every_cube_is_written_once_and_never_per_cube() -> None:
    """Keep workflow-wide overrides as overrides, with a seed beyond a browser's range exact."""

    source = _script(
        _CUBES,
        f'set *.*.sampler_name = "dpmpp_2m"\nset *.*.seed = {_WHOLE_SEED}',
        _LEFT,
        _RIGHT,
    )

    workflow = _workflow(source)

    assert _inputs(workflow, 0, "sampler")["seed"] == _WHOLE_SEED
    assert _inputs(workflow, 1, "sampler")["sampler_name"] == "dpmpp_2m"
    assert _written(workflow) == source


def test_a_node_following_another_keeps_its_own_value_in_the_script() -> None:
    """Run the followed value, and write back the value the follower holds for itself."""

    source = _script(
        _CUBES,
        _SHARED,
        _LEFT,
        'set Right."positive prompt" = Left."positive prompt"\n' + _RIGHT,
    )

    workflow = _workflow(source)

    assert _inputs(workflow, 1, "prompt")["value"] == "left text"
    assert _written(workflow) == source


def test_a_value_the_cube_does_not_show_is_written_back_when_the_script_set_it() -> (
    None
):
    """Keep an assignment to a field outside the Cube's face."""

    source = _script(
        _CUBES,
        _SHARED,
        'set Left."positive prompt".value = "left text"\n'
        "set Left.sampler.cfg = 3.5\n"
        "set Left.sampler.steps = 18",
        _RIGHT,
    )

    assert _written(_workflow(source)) == source


def test_state_no_value_expresses_is_carried_through() -> None:
    """Keep unsaved outputs, partial overrides, override choices and fixed seeds."""

    source = _script(
        _CUBES,
        '# cube_output_persistence {"alias":"Right","saved":false}',
        f"set *.*.seed = {_WHOLE_SEED}",
        '# global_override_value {"key":"sampler_name","mode":"global","value":"euler"}',
        '# global_override_selection {"key":"steps","selected":true}',
        '# seed_control {"alias":"Left","field":"steps","mode":"fixed","node":"sampler"}',
        '# global_override_seed_control {"key":"seed","mode":"fixed"}',
        'set Left."positive prompt".value = "left text"\n'
        'set Left.sampler.sampler_name = "euler"\n'
        "set Left.sampler.steps = 18",
        'set Right."positive prompt".value = "right text"\n'
        'set Right.sampler.sampler_name = "euler"\n'
        "set Right.sampler.steps = 7",
    )

    assert _written(_workflow(source)) == source


def test_the_pages_own_overrides_and_links_decide_once_it_has_edited_the_workflow() -> (
    None
):
    """Write what the page keeps: a switched-off override, a new link, a follower's value."""

    source = _script(
        _CUBES,
        f'set *.*.sampler_name = "dpmpp_2m"\nset *.*.seed = {_WHOLE_SEED}',
        _LEFT,
        _RIGHT,
    )
    workflow = deepcopy(_workflow(source))
    left, right = _instance_ids(workflow)

    def node(instance: str) -> str:
        return json.dumps(["cube", instance, "prompt"], separators=(",", ":"))

    cast(dict[str, object], workflow["extra"])["sugarcubes_relations"] = {
        "schema": 1,
        "overrides": {
            "seed": {
                "enabled": True,
                "value": 1.2345678901234567e19,
                "seedMode": "fixed",
            },
            "sampler_name": {
                "enabled": False,
                "value": "dpmpp_2m",
                "seedMode": "fixed",
            },
        },
        "fieldLinks": {},
        "nodeLinks": {node(left): None, node(right): node(left)},
        "locals": {
            json.dumps([node(right), "value"], separators=(",", ":")): "kept text"
        },
        "localSeedModes": {},
        "localNodeModes": {},
    }

    assert _written(workflow) == _script(
        _CUBES,
        f"set *.*.seed = {_WHOLE_SEED}",
        '# global_override_seed_control {"key":"seed","mode":"fixed"}',
        'set Left."positive prompt".value = "left text"\n'
        'set Left.sampler.sampler_name = "dpmpp_2m"\n'
        "set Left.sampler.steps = 18",
        'set Right."positive prompt" = Left."positive prompt"\n'
        'set Right."positive prompt".value = "kept text"\n'
        'set Right.sampler.sampler_name = "dpmpp_2m"\n'
        "set Right.sampler.steps = 7",
    )


def test_cubes_of_a_script_are_a_stack_each_fed_by_the_cube_above() -> None:
    """Connect what the order implies, and leave it unsaid when written back."""

    source = _script(_CUBES, _SHARED, _LEFT, _RIGHT)

    workflow = _workflow(source)

    left, right = _instance_ids(workflow)
    links = cast(list[list[object]], workflow["links"])
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    ids = {
        str(
            cast(
                dict[str, object],
                cast(dict[str, object], node["properties"])["sugarcubes_cube"],
            )["instance_id"]
        ): node["id"]
        for node in nodes
    }
    assert [(link[1], link[3]) for link in links] == [(ids[left], ids[right])]
    assert "connect " not in _written(workflow)


def test_a_connection_the_script_states_is_stated_again() -> None:
    """Keep a script's own connect line although the stack's order implies the same."""

    source = _script(
        _CUBES,
        _SHARED,
        _LEFT,
        _RIGHT,
        "connect Left.output.image to Right.input.image",
    )

    workflow = _workflow(source)

    assert len(cast(list[object], workflow["links"])) == 1
    assert _written(workflow) == source

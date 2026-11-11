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
"""Verify SugarScript lowers naturally into immutable native-Cube plans."""

from __future__ import annotations

from copy import deepcopy
from dataclasses import dataclass

from sugarcubes.cube_model import CubeDocument
from sugarcubes.language import (
    SugarScriptCompileRequest,
    SugarScriptCompileResult,
    SugarScriptLanguageService,
)


@dataclass(frozen=True)
class _Resolver:
    """Resolve fixed validated documents without filesystem ownership."""

    documents: dict[str, CubeDocument]

    def resolve(self, cube_id: str, version_pin: str | None) -> CubeDocument:
        """Return an exact fixture while enforcing requested version pins."""

        try:
            document = self.documents[cube_id]
        except KeyError as error:
            raise ValueError(f"Cube '{cube_id}' is unavailable.") from error
        if version_pin is not None and version_pin != document.version:
            raise ValueError(
                f"Cube '{cube_id}' has version {document.version}, not {version_pin}."
            )
        return document


def test_compiler_resolves_composition_without_mutating_catalog_documents() -> None:
    """Lower aliases, selectors, flavors, activation, values, and connections."""

    source_document = _cube_document("source", include_output=True)
    sink_document = _cube_document("sink", include_input=True)
    original_source = deepcopy(source_document.to_dict())
    source = (
        'use "local/tests/source.cube"@1.0.0 with Detailed as Src repeat 2\n'
        'use "local/tests/source.cube" as Src repeat 1\n'
        'use "local/tests/sink.cube" as Out repeat 3\n'
        "let base = 4 * 2\n"
        "set Src2.sampler.steps = base + 2\n"
        'set Src2.sampler."CFG Scale" = 9\n'
        "set *.KSampler.cfg = 7\n"
        'set *.*.model = "must not replace a graph edge"\n'
        "disable Src3.sampler\n"
        "connect Src[1-3].output.image to Out[1-3].input.image\n"
    )

    result = SugarScriptLanguageService().compile(
        SugarScriptCompileRequest(
            source,
            _Resolver(
                {
                    source_document.cube_id: source_document,
                    sink_document.cube_id: sink_document,
                }
            ),
        )
    )

    assert result.is_valid
    assert result.diagnostics == ()
    assert result.plan is not None
    assert [instance.alias for instance in result.plan.instances] == [
        "Src1",
        "Src2",
        "Src3",
        "Out1",
        "Out2",
        "Out3",
    ]
    first, second, third = result.plan.instances[:3]
    assert _node_inputs(first.document, "sampler")["cfg"] == 7
    assert _node_inputs(second.document, "sampler") == {
        "cfg": 9,
        "model": ["provider", 0],
        "steps": 10,
    }
    assert _node(first.document, "sampler").get("mode") is None
    assert _node(third.document, "sampler")["mode"] == 4
    assert len(result.plan.connections) == 3
    assert result.plan.connections[0].source_binding == "output.image"
    assert result.plan.connections[0].target_binding == "input.image"
    assert source_document.to_dict() == original_source
    for instance in result.plan.instances:
        CubeDocument.from_dict(instance.document.to_dict())


def test_exact_assignment_wins_over_broad_selector_regardless_of_source_order() -> None:
    """Improve legacy wildcard-last precedence with stable specificity semantics."""

    document = _cube_document("source", include_output=True)
    result = _compile(
        'use "local/tests/source.cube" as Cube\n'
        "set Cube.sampler.cfg = 4\n"
        "set *.KSampler.cfg = 12\n",
        document,
    )

    assert result.plan is not None
    assert _node_inputs(result.plan.instances[0].document, "sampler")["cfg"] == 4


def test_script_values_override_cube_defaults_and_absent_values_retain_them() -> None:
    """Apply the SugarScript-to-Cube portion of the default precedence contract."""

    document = _cube_document("source")
    inherited = _compile('use "local/tests/source.cube" as Cube\n', document)
    overridden = _compile(
        'use "local/tests/source.cube" as Cube\nset Cube.sampler.cfg = 0\n',
        document,
    )

    assert inherited.plan is not None
    assert overridden.plan is not None
    assert _node_inputs(inherited.plan.instances[0].document, "sampler")["cfg"] == 1
    assert _node_inputs(overridden.plan.instances[0].document, "sampler")["cfg"] == 0


def test_adjacent_hash_metadata_binds_to_resolved_stable_field_identity() -> None:
    """Carry model companion data without interpreting or resolving its hash."""

    document = _cube_document("source")
    sha256 = "A" * 64
    result = _compile(
        'use "local/tests/source.cube" as Cube\n'
        "set Cube.sampler.cfg = 4\n"
        f"# sha256 {sha256}\n",
        document,
    )

    assert result.plan is not None
    annotation = result.plan.field_annotations[0]
    assert annotation.instance_id == result.plan.instances[0].instance_id
    assert annotation.node_symbol == "sampler"
    assert annotation.input_name == "cfg"
    assert annotation.namespace == "substitute.model_asset"
    assert annotation.payload == {"sha256": sha256}


def test_whole_node_link_copies_editable_state_without_replacing_graph_inputs() -> None:
    """Preserve Sugar-DSL node-link behavior used by attached recipe images."""

    source = _node_link_document("source", value="source prompt", mode=4)
    target = _node_link_document("target", value="target prompt", mode=0)
    result = _compile(
        'use "local/tests/source.cube" as "Text to Image"\n'
        'use "local/tests/target.cube" as "Diffusion Upscale"\n'
        'set "Diffusion Upscale"."positive prompt" = '
        '"Text to Image"."positive prompt"\n',
        source,
        target,
    )

    assert result.is_valid
    assert result.plan is not None
    linked = _node(result.plan.instances[1].document, "positive_prompt")
    assert linked["inputs"] == {
        "model": ["provider", 0],
        "value": "source prompt",
    }
    assert linked["mode"] == 4


def test_whole_node_link_is_applied_after_field_assignments() -> None:
    """Preserve deferred Sugar-DSL identity-link precedence for recipe images."""

    source = _node_link_document("source", value="source prompt", mode=0)
    target = _node_link_document("target", value="target prompt", mode=0)
    result = _compile(
        'use "local/tests/source.cube" as Source\n'
        'use "local/tests/target.cube" as Target\n'
        'set Target."positive prompt" = Source."positive prompt"\n'
        'set Target."positive prompt".value = "later target value"\n',
        source,
        target,
    )

    assert result.is_valid
    assert result.plan is not None
    assert (
        _node_inputs(result.plan.instances[1].document, "positive_prompt")["value"]
        == "source prompt"
    )


def test_whole_node_link_rejects_different_node_classes() -> None:
    """Fail closed when a legacy whole-node link joins unlike node contracts."""

    source = _node_link_document("source", value="source prompt", mode=0)
    target_payload = _node_link_document(
        "target", value="target prompt", mode=0
    ).to_dict()
    implementation = target_payload["implementation"]
    assert isinstance(implementation, dict)
    nodes = implementation["nodes"]
    assert isinstance(nodes, dict)
    target_node = nodes["positive_prompt"]
    assert isinstance(target_node, dict)
    target_node["class_type"] = "PrimitiveString"
    target = CubeDocument.from_dict(target_payload)

    result = _compile(
        'use "local/tests/source.cube" as Source\n'
        'use "local/tests/target.cube" as Target\n'
        'set Target."positive prompt" = Source."positive prompt"\n',
        source,
        target,
    )

    assert not result.is_valid
    assert result.plan is None
    assert "Node link class types differ" in result.diagnostics[0].message


def test_whole_node_link_rejects_different_editable_input_contracts() -> None:
    """Fail closed instead of partially copying unlike widget state."""

    source = _node_link_document("source", value="source prompt", mode=0)
    target_payload = _node_link_document(
        "target", value="target prompt", mode=0
    ).to_dict()
    implementation = target_payload["implementation"]
    assert isinstance(implementation, dict)
    nodes = implementation["nodes"]
    assert isinstance(nodes, dict)
    target_node = nodes["positive_prompt"]
    assert isinstance(target_node, dict)
    inputs = target_node["inputs"]
    assert isinstance(inputs, dict)
    inputs["weight"] = 1.0
    target = CubeDocument.from_dict(target_payload)

    result = _compile(
        'use "local/tests/source.cube" as Source\n'
        'use "local/tests/target.cube" as Target\n'
        'set Target."positive prompt" = Source."positive prompt"\n',
        source,
        target,
    )

    assert not result.is_valid
    assert result.plan is None
    assert "Node link editable input keys differ" in result.diagnostics[0].message


def test_whole_node_link_rejects_different_graph_input_contracts() -> None:
    """Fail closed instead of replacing target-owned graph topology."""

    source = _node_link_document("source", value="source prompt", mode=0)
    target_payload = _node_link_document(
        "target", value="target prompt", mode=0
    ).to_dict()
    implementation = target_payload["implementation"]
    assert isinstance(implementation, dict)
    nodes = implementation["nodes"]
    assert isinstance(nodes, dict)
    target_node = nodes["positive_prompt"]
    assert isinstance(target_node, dict)
    inputs = target_node["inputs"]
    assert isinstance(inputs, dict)
    inputs["model"] = ["provider", 1]
    target = CubeDocument.from_dict(target_payload)

    result = _compile(
        'use "local/tests/source.cube" as Source\n'
        'use "local/tests/target.cube" as Target\n'
        'set Target."positive prompt" = Source."positive prompt"\n',
        source,
        target,
    )

    assert not result.is_valid
    assert result.plan is None
    assert "Node link graph inputs differ" in result.diagnostics[0].message


def test_assignments_resolve_hidden_native_subgraph_widgets_by_name_and_label() -> None:
    """Address wrapper widgets exactly as historical SugarScript addressed them."""

    document = _native_subgraph_document()
    result = _compile(
        'use "local/tests/native-subgraph.cube" as Cube\n'
        'set *.*.sampler_name = "euler_ancestral"\n'
        "set Cube.KSampler.cfg = 5\n"
        'set Cube.Upscaler."Scale Factor" = 1.5\n',
        document,
    )

    assert result.is_valid
    assert result.plan is not None
    nodes = result.plan.instances[0].document.implementation.nodes
    assert nodes["ksampler"]["inputs"]["sampler_name"] == "euler_ancestral"
    assert nodes["ksampler"]["inputs"]["cfg"] == 5
    assert nodes["upscaler"]["inputs"]["value"] == 1.5


def test_semantic_hash_and_instance_ids_ignore_source_formatting() -> None:
    """Keep synchronization and workflow identity stable across cosmetic formatting."""

    document = _cube_document("source", include_output=True)
    first = _compile('use "local/tests/source.cube" as Cube\n', document)
    second = _compile("  USE 'local/tests/source.cube'  AS Cube\r\n", document)

    assert first.plan is not None
    assert second.plan is not None
    assert first.plan.semantic_hash == second.plan.semantic_hash
    assert first.plan.instances[0].instance_id == second.plan.instances[0].instance_id


def test_compiler_reports_multiple_located_semantic_failures_without_a_plan() -> None:
    """Recover from independent resolution errors and never emit a partial workflow."""

    document = _cube_document("source", include_output=True)
    result = _compile(
        'use "local/tests/source.cube" with Missing as Broken\n'
        'use "local/tests/absent.cube" as Absent\n'
        "set Unknown.sampler.cfg = 2\n",
        document,
    )

    assert not result.is_valid
    assert result.plan is None
    assert len(result.diagnostics) == 3
    assert {diagnostic.code for diagnostic in result.diagnostics} == {
        "sugarscript.compile.invalid_semantics"
    }
    assert [diagnostic.span.start.line for diagnostic in result.diagnostics] == [
        1,
        2,
        3,
    ]


def _compile(source: str, *documents: CubeDocument) -> SugarScriptCompileResult:
    """Compile source through one fixture resolver."""

    return SugarScriptLanguageService().compile(
        SugarScriptCompileRequest(
            source,
            _Resolver({document.cube_id: document for document in documents}),
        )
    )


def _cube_document(
    name: str,
    *,
    include_input: bool = False,
    include_output: bool = False,
) -> CubeDocument:
    """Create one validated Cube with script-facing controls and boundaries."""

    cube_id = f"local/tests/{name}.cube"
    return CubeDocument.from_dict(
        {
            "cube_id": cube_id,
            "version": "1.0.0",
            "description": f"{name} test Cube",
            "metadata": {"default_alias": name.title()},
            "implementation": {
                "nodes": {
                    "provider": {
                        "class_type": "ModelProvider",
                        "label": "Provider",
                        "inputs": {},
                    },
                    "sampler": {
                        "class_type": "KSampler",
                        "label": "Sampler",
                        "inputs": {
                            "cfg": 1,
                            "steps": 5,
                            "model": ["provider", 0],
                        },
                    },
                },
                "inputs": (
                    {"input.image": {"targets": ["sampler"]}} if include_input else {}
                ),
                "outputs": {"output.image": "sampler"} if include_output else {},
                "layout": {},
                "definitions": {},
                "subgraphs": [],
            },
            "surface": {
                "default_flavor_id": "default",
                "controls": [
                    {
                        "control_id": "sampler.cfg",
                        "symbol": "sampler",
                        "input_name": "cfg",
                        "label": "CFG Scale",
                        "class_type": "KSampler",
                        "value_type": "number",
                    },
                    {
                        "control_id": "sampler.steps",
                        "symbol": "sampler",
                        "input_name": "steps",
                        "label": "Steps",
                        "class_type": "KSampler",
                        "value_type": "number",
                    },
                ],
            },
            "flavors": {
                "authored": [
                    {"id": "default", "name": "Default", "values": {}},
                    {
                        "id": "detailed",
                        "name": "Detailed",
                        "values": {"sampler.cfg": 6, "sampler.steps": 20},
                    },
                ]
            },
        }
    )


def _node_link_document(name: str, *, value: str, mode: int) -> CubeDocument:
    """Create one Cube whose prompt node has editable and graph-owned inputs."""

    return CubeDocument.from_dict(
        {
            "cube_id": f"local/tests/{name}.cube",
            "version": "1.0.0",
            "description": f"{name} node-link Cube",
            "metadata": {"default_alias": name.title()},
            "implementation": {
                "nodes": {
                    "provider": {
                        "class_type": "ModelProvider",
                        "label": "Provider",
                        "inputs": {},
                    },
                    "positive_prompt": {
                        "class_type": "PrimitiveStringMultiline",
                        "label": "positive prompt",
                        "mode": mode,
                        "inputs": {
                            "model": ["provider", 0],
                            "value": value,
                        },
                    },
                },
                "inputs": {},
                "outputs": {},
                "layout": {},
                "definitions": {},
                "subgraphs": [],
            },
            "surface": {
                "default_flavor_id": "default",
                "controls": [
                    {
                        "control_id": "positive_prompt.value",
                        "symbol": "positive_prompt",
                        "input_name": "value",
                        "label": "value",
                        "class_type": "PrimitiveStringMultiline",
                        "value_type": "string",
                    }
                ],
            },
            "flavors": {
                "authored": [{"id": "default", "name": "Default", "values": {}}]
            },
        }
    )


def _native_subgraph_document() -> CubeDocument:
    """Create one Cube with widget inputs owned by embedded native subgraphs."""

    sampler_type = "fixture-sampler-subgraph"
    upscaler_type = "fixture-upscaler-subgraph"
    return CubeDocument.from_dict(
        {
            "cube_id": "local/tests/native-subgraph.cube",
            "version": "1.0.0",
            "description": "native subgraph widget fixture",
            "metadata": {"default_alias": "Native Subgraph"},
            "implementation": {
                "nodes": {
                    "ksampler": {
                        "class_type": sampler_type,
                        "label": "KSampler",
                        "inputs": {},
                    },
                    "upscaler": {
                        "class_type": upscaler_type,
                        "label": "Upscaler",
                        "inputs": {},
                    },
                },
                "inputs": {},
                "outputs": {},
                "layout": {},
                "definitions": {},
                "subgraphs": [
                    {
                        "id": sampler_type,
                        "name": "KSampler",
                        "inputs": [
                            {"name": "cfg", "label": "cfg", "type": "FLOAT"},
                            {
                                "name": "sampler_name",
                                "label": "sampler_name",
                                "type": "COMBO",
                            },
                        ],
                        "outputs": [],
                        "nodes": [],
                        "links": [],
                    },
                    {
                        "id": upscaler_type,
                        "name": "Upscaler",
                        "inputs": [
                            {"name": "value", "label": "Scale Factor", "type": "FLOAT"}
                        ],
                        "outputs": [],
                        "nodes": [],
                        "links": [],
                    },
                ],
            },
            "surface": {"default_flavor_id": "default", "controls": []},
            "flavors": {
                "authored": [{"id": "default", "name": "Default", "values": {}}]
            },
        }
    )


def _node(document: CubeDocument, symbol: str) -> dict[str, object]:
    """Read one compiled node payload from a mapping-shaped document."""

    implementation = document.to_dict()["implementation"]
    assert isinstance(implementation, dict)
    nodes = implementation["nodes"]
    assert isinstance(nodes, dict)
    node = nodes[symbol]
    assert isinstance(node, dict)
    return node


def _node_inputs(document: CubeDocument, symbol: str) -> dict[str, object]:
    """Read one compiled node's input mapping."""

    inputs = _node(document, symbol)["inputs"]
    assert isinstance(inputs, dict)
    return inputs

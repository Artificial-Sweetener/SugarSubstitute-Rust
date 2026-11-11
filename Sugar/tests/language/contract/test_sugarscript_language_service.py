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
"""Verify SugarCubes' presentation-neutral SugarScript language contract."""

from __future__ import annotations

from copy import deepcopy
from collections.abc import Mapping
from dataclasses import dataclass, field
from typing import cast

from sugarcubes.cube_model import CubeDocument
from sugarcubes.language import (
    SugarScriptCompileRequest,
    SugarScriptLanguageService,
    SugarScriptRenderRequest,
)
from sugarcubes.language.syntax import (
    BinaryExpression,
    ConnectStatement,
    DisableStatement,
    EnableStatement,
    LetStatement,
    ListExpression,
    LiteralExpression,
    NameExpression,
    PathExpression,
    RandomExpression,
    SetStatement,
    SugarScriptComment,
    UseStatement,
)


def test_parser_preserves_unknown_string_escapes_as_literal_path_separators() -> None:
    """Preserve Windows-style model paths while decoding supported escapes."""

    result = SugarScriptLanguageService().parse(
        'set Cube.prompt.value = "<lora:Anima\\style\\PeopleWorks:1.00>"\n'
    )

    statement = result.document.statements[0]
    assert isinstance(statement, SetStatement)
    assert isinstance(statement.value, LiteralExpression)
    assert statement.value.value == r"<lora:Anima\style\PeopleWorks:1.00>"
    assert result.diagnostics == ()


def test_parser_covers_legacy_statement_and_expression_families() -> None:
    """Parse the characterized Sugar DSL surface into strongly typed syntax."""

    source = (
        'use "source"@2.0.0 with "Detailed" as "Text to Image" repeat 2\n'
        'use "target" as Out\n'
        'connect "Text to Image"[1-2].output.image to Out.input.image\n'
        "let base_steps = 5 * (2 + 3)\n"
        "set *.KSampler.cfg = 7.5\n"
        "set Out.sampler.steps = base_steps\n"
        'set Out.prompt.value = ["café", [1, true], null,]\n'
        "set Out.sampler.seed = random\n"
        "disable Out.checkpoint\n"
        "enable Out.checkpoint\n"
    )

    result = SugarScriptLanguageService().parse(source)

    assert result.is_valid
    assert result.diagnostics == ()
    statements = result.document.statements
    first_use = statements[0]
    connection = statements[2]
    variable = statements[3]
    wildcard = statements[4]
    named_assignment = statements[5]
    list_assignment = statements[6]
    random_assignment = statements[7]
    assert isinstance(first_use, UseStatement)
    assert first_use.version_pin == "2.0.0"
    assert first_use.flavor == "Detailed"
    assert first_use.alias == "Text to Image"
    assert first_use.repeat == 2
    assert isinstance(connection, ConnectStatement)
    assert connection.source.alias_range is not None
    assert (
        connection.source.alias_range.start,
        connection.source.alias_range.end,
    ) == (1, 2)
    assert isinstance(variable, LetStatement)
    assert isinstance(variable.value, BinaryExpression)
    assert isinstance(wildcard, SetStatement)
    assert wildcard.target.parts == ("*", "KSampler", "cfg")
    assert isinstance(named_assignment, SetStatement)
    assert isinstance(named_assignment.value, NameExpression)
    assert isinstance(list_assignment, SetStatement)
    assert isinstance(list_assignment.value, ListExpression)
    assert isinstance(list_assignment.value.items[1], ListExpression)
    assert isinstance(random_assignment, SetStatement)
    assert isinstance(random_assignment.value, RandomExpression)
    assert isinstance(statements[8], DisableStatement)
    assert isinstance(statements[9], EnableStatement)


def test_parser_preserves_quoted_legacy_whole_node_links() -> None:
    """Parse the whole-node links embedded in historical Substitute PNGs."""

    result = SugarScriptLanguageService().parse(
        'set "Anima/Diffusion Upscale"."positive prompt" = '
        '"Anima/Text to Image"."positive prompt"\n'
    )

    assert result.is_valid
    assert result.diagnostics == ()
    statement = result.document.statements[0]
    assert isinstance(statement, SetStatement)
    assert statement.target.parts == ("Anima/Diffusion Upscale", "positive prompt")
    assert isinstance(statement.value, PathExpression)
    assert statement.value.path.parts == ("Anima/Text to Image", "positive prompt")


def test_parser_reports_multiple_located_errors_and_keeps_valid_statements() -> None:
    """Improve on the legacy fail-fast parser by recovering at statement boundaries."""

    result = SugarScriptLanguageService().parse(
        'use "valid" as First\n'
        "set First.node.value 4\n"
        "nonsense anything\n"
        'use "also-valid" as Second\n'
    )

    assert not result.is_valid
    assert [diagnostic.code for diagnostic in result.diagnostics] == [
        "sugarscript.parse.unexpected_token",
        "sugarscript.parse.unexpected_token",
    ]
    assert [diagnostic.span.start.line for diagnostic in result.diagnostics] == [2, 3]
    assert [
        statement.alias
        for statement in result.document.statements
        if isinstance(statement, UseStatement)
    ] == [
        "First",
        "Second",
    ]


def test_parser_requires_one_statement_per_source_line() -> None:
    """Reject accidental line concatenation with a precise token span."""

    result = SugarScriptLanguageService().parse('use "one" as One use "two" as Two\n')

    assert not result.is_valid
    assert result.document.statements == ()
    assert result.diagnostics[0].message == "Expected the end of the statement."
    assert result.diagnostics[0].span.start.column == 18


def test_substitute_extension_comments_are_typed_and_losslessly_preserved() -> None:
    """Recognize portable Substitute metadata without importing editor ownership."""

    source = (
        "# Project: Portrait Study\n"
        '# cube_output_persistence {"alias":"Hero","saved":false}\n'
        '# node_enabled {"alias":"Hero","node":"checkpoint","enabled":true}\n'
        "# sha256 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n"
        "# an unknown future extension stays exact\n"
        '# bypass use "cube" as Hero\n'
    )

    result = SugarScriptLanguageService().parse(source)

    assert result.is_valid
    assert [comment.extension_name for comment in result.document.comments] == [
        "project",
        "cube_output_persistence",
        "node_enabled",
        "sha256",
        None,
    ]
    assert result.document.comments[1].extension_payload == {
        "alias": "Hero",
        "saved": False,
    }
    bypassed = result.document.statements[0]
    assert isinstance(bypassed, UseStatement)
    assert bypassed.bypassed is True
    assert bypassed.span.start.line == 6
    assert bypassed.span.start.column == 10

    rendered = SugarScriptLanguageService().render(
        SugarScriptRenderRequest(result.document)
    )
    assert rendered.source == source
    assert [entry.rendered_line for entry in rendered.source_map] == [1, 2, 3, 4, 5, 6]


def test_malformed_supported_extension_is_warning_and_remains_round_trip_safe() -> None:
    """Keep a workflow-authoring companion usable when optional metadata is malformed."""

    source = '# seed_control {"alias":\nuse "cube" as Valid\n'
    service = SugarScriptLanguageService()
    result = service.parse(source)

    assert result.is_valid
    assert len(result.diagnostics) == 1
    assert result.diagnostics[0].severity.value == "warning"
    assert result.diagnostics[0].code == "sugarscript.extension.invalid_payload"
    assert service.render(SugarScriptRenderRequest(result.document)).source == source


def test_deterministic_renderer_normalizes_syntax_but_preserves_semantics() -> None:
    """Render one stable format and parse it back to equivalent typed content."""

    service = SugarScriptLanguageService()
    parsed = service.parse(
        "USE 'cube'@2.0 WITH fancy AS 'Hero Image' REPEAT 2\r\n"
        'set "Hero Image".prompt.value="café\\nlight"\r\n'
        "set *.KSampler.seed = -4 + 3 * 2\r\n"
    )
    assert parsed.is_valid

    first_render = service.render(SugarScriptRenderRequest(parsed.document))
    reparsed = service.parse(first_render.source)
    second_render = service.render(SugarScriptRenderRequest(reparsed.document))

    assert first_render.source == second_render.source
    assert first_render.source == (
        'use "cube"@2.0 with fancy as "Hero Image" repeat 2\n'
        'set "Hero Image".prompt.value = "café\\nlight"\n'
        "set *.KSampler.seed = -4 + 3 * 2\n"
    )
    assignment = reparsed.document.statements[1]
    assert isinstance(assignment, SetStatement)
    assert isinstance(assignment.value, LiteralExpression)
    assert assignment.value.value == "café\nlight"
    arithmetic = reparsed.document.statements[2]
    assert isinstance(arithmetic, SetStatement)
    assert isinstance(arithmetic.value, BinaryExpression)


def test_path_references_remain_distinct_from_names_and_string_literals() -> None:
    """Keep resolution intent explicit before workflow lowering begins."""

    result = SugarScriptLanguageService().parse(
        "let field = Cube.node.value\n"
        "let variable = field\n"
        'let text = "Cube.node.value"\n'
    )

    assert result.is_valid
    values = [
        statement.value
        for statement in result.document.statements
        if isinstance(statement, LetStatement)
    ]
    assert isinstance(values[0], PathExpression)
    assert isinstance(values[1], NameExpression)
    assert isinstance(values[2], LiteralExpression)


def test_plain_comments_remain_first_class_document_items() -> None:
    """Retain author context without interpreting it as executable language."""

    result = SugarScriptLanguageService().parse("# before\nuse cube\n# after\n")

    assert result.is_valid
    assert isinstance(result.document.items[0], SugarScriptComment)
    assert isinstance(result.document.items[1], UseStatement)
    assert isinstance(result.document.items[2], SugarScriptComment)


@dataclass
class _CubeResolver:
    """Resolve immutable test Cubes while recording the language boundary calls."""

    documents: dict[str, CubeDocument]
    calls: list[tuple[str, str | None]] = field(default_factory=list)

    def resolve(self, cube_id: str, version_pin: str | None) -> CubeDocument:
        """Return one exact test document or report a useful catalog failure."""

        self.calls.append((cube_id, version_pin))
        if cube_id not in self.documents:
            raise ValueError(f"Cube '{cube_id}' is not available.")
        document = self.documents[cube_id]
        if version_pin is not None and version_pin != document.version:
            raise ValueError(
                f"Cube '{cube_id}' has version {document.version}, not {version_pin}."
            )
        return document


def test_compiler_resolves_native_cube_instances_connections_and_values() -> None:
    """Lower language semantics without importing Comfy or mutating catalog Cubes."""

    source = _cube_document(
        "artificial-sweetener/base-cubes/source.cube",
        outputs={"output.image": {"symbol": "sampler", "slot": 0}},
    )
    sink = _cube_document(
        "artificial-sweetener/base-cubes/sink.cube",
        inputs={"input.image": [{"symbol": "sampler", "input": "image"}]},
    )
    original_source = deepcopy(source.to_dict())
    resolver = _CubeResolver({source.cube_id: source, sink.cube_id: sink})
    script = (
        f'use "{source.cube_id}"@1.0.0 with Detailed as Producer repeat 2\n'
        f'use "{sink.cube_id}" as Sink repeat 2\n'
        "let chosen_steps = 4 * 5\n"
        "set *.KSampler.cfg = 3\n"
        'set Producer1.sampler."CFG Scale" = 9\n'
        "set Producer[1-2].sampler.steps = chosen_steps\n"
        "set Producer1.sampler.seed = random\n"
        "disable Producer2.sampler\n"
        "connect Producer[1-2].output.image to Sink[1-2].input.image\n"
    )

    result = SugarScriptLanguageService().compile(
        SugarScriptCompileRequest(script, resolver)
    )

    assert result.is_valid
    assert result.diagnostics == ()
    assert result.plan is not None
    assert [instance.alias for instance in result.plan.instances] == [
        "Producer1",
        "Producer2",
        "Sink1",
        "Sink2",
    ]
    assert [instance.instance_id for instance in result.plan.instances] == [
        f"sugar-{result.plan.semantic_hash[:12]}-{index}" for index in range(1, 5)
    ]
    producer_one = _compiled_nodes(result.plan.instances[0].document.to_dict())
    producer_two = _compiled_nodes(result.plan.instances[1].document.to_dict())
    assert _compiled_inputs(producer_one["sampler"]) == {
        "cfg": 9,
        "steps": 20,
        "seed": {"$sugarscript": "random"},
    }
    assert _compiled_inputs(producer_two["sampler"])["cfg"] == 3
    assert _compiled_inputs(producer_two["sampler"])["steps"] == 20
    assert producer_two["sampler"]["mode"] == 4
    assert _compiled_inputs(producer_one["linked_sampler"])["cfg"] == ["provider", 0]
    producer_one_document = result.plan.instances[0].document
    producer_one_values = producer_one_document.authored_flavor_index()[
        producer_one_document.surface.default_flavor_id
    ].values
    assert producer_one_values["sampler.cfg"] == 9
    assert producer_one_values["sampler.steps"] == 20
    assert producer_one_values["sampler.seed"] == {"$sugarscript": "random"}
    assert [
        (connection.source_binding, connection.target_binding)
        for connection in result.plan.connections
    ] == [("output.image", "input.image"), ("output.image", "input.image")]
    assert resolver.calls == [
        (source.cube_id, "1.0.0"),
        (sink.cube_id, None),
    ]
    assert source.to_dict() == original_source


def test_exact_assignment_beats_wildcard_selector_independent_of_source_order() -> None:
    """Prefer specific author intent over broad defaults as a deliberate improvement."""

    document = _cube_document("artificial-sweetener/base-cubes/source.cube")
    resolver = _CubeResolver({document.cube_id: document})
    service = SugarScriptLanguageService()
    sources = (
        f'use "{document.cube_id}" as Cube\n'
        "set Cube.sampler.cfg = 11\n"
        "set *.KSampler.cfg = 4\n",
        f'use "{document.cube_id}" as Cube\n'
        "set *.KSampler.cfg = 4\n"
        "set Cube.sampler.cfg = 11\n",
    )

    values = []
    for script in sources:
        result = service.compile(SugarScriptCompileRequest(script, resolver))
        assert result.plan is not None
        nodes = _compiled_nodes(result.plan.instances[0].document.to_dict())
        values.append(_compiled_inputs(nodes["sampler"])["cfg"])

    assert values == [11, 11]


def test_semantic_hash_ignores_formatting_comments_and_source_locations() -> None:
    """Key native instance identity to normalized semantics rather than raw text."""

    document = _cube_document("artificial-sweetener/base-cubes/source.cube")
    resolver = _CubeResolver({document.cube_id: document})
    service = SugarScriptLanguageService()

    compact = service.compile(
        SugarScriptCompileRequest(
            f'use "{document.cube_id}" as Cube\nset Cube.sampler.steps=20\n',
            resolver,
        )
    )
    annotated = service.compile(
        SugarScriptCompileRequest(
            f'# author note\nUSE "{document.cube_id}" AS Cube\n'
            "set Cube.sampler.steps = 20\n",
            resolver,
        )
    )

    assert compact.plan is not None
    assert annotated.plan is not None
    assert compact.plan.semantic_hash == annotated.plan.semantic_hash
    assert (
        compact.plan.instances[0].instance_id == annotated.plan.instances[0].instance_id
    )


def test_compiler_returns_all_recoverable_semantic_failures_without_a_plan() -> None:
    """Keep invalid source out of workflow mutation while locating each failure."""

    document = _cube_document("artificial-sweetener/base-cubes/source.cube")
    resolver = _CubeResolver({document.cube_id: document})
    result = SugarScriptLanguageService().compile(
        SugarScriptCompileRequest(
            f'use "{document.cube_id}" as Cube\n'
            'use "artificial-sweetener/base-cubes/missing.cube" as Missing\n'
            "set Unknown.sampler.cfg = 7\n",
            resolver,
        )
    )

    assert not result.is_valid
    assert result.plan is None
    assert [diagnostic.code for diagnostic in result.diagnostics] == [
        "sugarscript.compile.invalid_semantics",
        "sugarscript.compile.invalid_semantics",
    ]
    assert [diagnostic.span.start.line for diagnostic in result.diagnostics] == [2, 3]


def _cube_document(
    cube_id: str,
    *,
    inputs: dict[str, object] | None = None,
    outputs: dict[str, object] | None = None,
) -> CubeDocument:
    """Build one validated native Cube contract for language lowering tests."""

    return CubeDocument.from_dict(
        {
            "cube_id": cube_id,
            "version": "1.0.0",
            "description": "Compiler contract Cube",
            "metadata": {"default_alias": "Contract Cube"},
            "implementation": {
                "nodes": {
                    "provider": {
                        "class_type": "PrimitiveNode",
                        "inputs": {"value": 5},
                    },
                    "sampler": {
                        "class_type": "KSampler",
                        "inputs": {"cfg": 1, "steps": 10, "seed": 1},
                    },
                    "linked_sampler": {
                        "class_type": "KSampler",
                        "inputs": {"cfg": ["provider", 0]},
                    },
                },
                "inputs": inputs or {},
                "outputs": outputs or {},
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
                    {
                        "control_id": "sampler.seed",
                        "symbol": "sampler",
                        "input_name": "seed",
                        "label": "Seed",
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
                        "values": {"sampler.cfg": 8, "sampler.steps": 30},
                    },
                ]
            },
        }
    )


def _compiled_nodes(
    payload: Mapping[str, object],
) -> Mapping[str, Mapping[str, object]]:
    """Narrow compiled document nodes at the serialized contract boundary."""

    implementation = payload.get("implementation")
    assert isinstance(implementation, Mapping)
    nodes = implementation.get("nodes")
    assert isinstance(nodes, Mapping)
    assert all(isinstance(node, Mapping) for node in nodes.values())
    return cast(Mapping[str, Mapping[str, object]], nodes)


def _compiled_inputs(node: Mapping[str, object]) -> Mapping[str, object]:
    """Narrow one compiled node's serialized input mapping."""

    inputs = node.get("inputs")
    assert isinstance(inputs, Mapping)
    return cast(Mapping[str, object], inputs)

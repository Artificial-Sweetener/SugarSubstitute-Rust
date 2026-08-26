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
"""Lower typed SugarScript into a deterministic native-workflow construction plan."""

from __future__ import annotations

from copy import deepcopy
from typing import Mapping

from ..cube_model import CubeDocument
from ..cube_model.merge import materialize_nodes
from .compiler_models import (
    CompiledCubeConnection,
    CompiledCubeInstance,
    SugarScriptCompileRequest,
    SugarScriptCompileResult,
    SugarScriptWorkflowPlan,
)
from .expression_evaluator import evaluate_expression
from .node_linking import apply_whole_node_link
from .parser import SugarScriptParser
from .resolved_instances import (
    ResolvedCubeInstance,
    expand_path,
    implementation_nodes,
    instance_aliases,
    read_instance_value,
    resolve_boundary,
    resolve_field,
    resolve_input,
    resolve_instance,
    resolve_node,
)
from .semantic_identity import compute_semantic_hash
from .source import (
    DiagnosticSeverity,
    SourcePosition,
    SourceSpan,
    SugarScriptDiagnostic,
)
from .syntax import (
    ConnectStatement,
    DisableStatement,
    EnableStatement,
    LetStatement,
    PathExpression,
    SetStatement,
    SugarPath,
    SugarStatement,
    UseStatement,
)


class SugarScriptCompiler:
    """Own alias, flavor, assignment, activation, and connection semantics."""

    def compile(self, request: SugarScriptCompileRequest) -> SugarScriptCompileResult:
        """Resolve source into a complete plan or stable located diagnostics."""

        parsed = SugarScriptParser(request.source).parse()
        if not parsed.is_valid:
            return SugarScriptCompileResult(None, parsed.diagnostics)
        semantic_hash = compute_semantic_hash(parsed.document.statements)
        instances: dict[str, ResolvedCubeInstance] = {}
        ordered_instances: list[ResolvedCubeInstance] = []
        repeat_counters: dict[str, int] = {}
        variables: dict[str, object] = {}
        connections: list[CompiledCubeConnection] = []
        deferred: list[SugarStatement] = []
        diagnostics = list(parsed.diagnostics)

        for statement in parsed.document.statements:
            if statement.bypassed and not isinstance(statement, UseStatement):
                continue
            try:
                if isinstance(statement, UseStatement):
                    self._add_use(
                        statement,
                        request,
                        semantic_hash,
                        instances,
                        ordered_instances,
                        repeat_counters,
                    )
                elif isinstance(statement, LetStatement):
                    variables[statement.name] = evaluate_expression(
                        statement.value,
                        variables,
                        lambda path: read_instance_value(instances, path),
                    )
                elif isinstance(statement, ConnectStatement):
                    connections.extend(_compile_connection(statement, instances))
                else:
                    deferred.append(statement)
            except (ValueError, KeyError) as error:
                diagnostics.append(_semantic_error(statement.span, str(error)))

        wildcard_sets = [
            item
            for item in deferred
            if isinstance(item, SetStatement) and item.target.parts[0] == "*"
        ]
        node_links = [
            item
            for item in deferred
            if isinstance(item, SetStatement)
            and item.target.parts[0] != "*"
            and len(item.target.parts) == 2
        ]
        exact_sets = [
            item
            for item in deferred
            if isinstance(item, SetStatement)
            and item.target.parts[0] != "*"
            and len(item.target.parts) != 2
        ]
        activations = [
            item
            for item in deferred
            if isinstance(item, (EnableStatement, DisableStatement))
        ]
        # Broad selectors establish defaults; exact author intent wins regardless
        # of source order. This improves the legacy wildcard-last behavior.
        for statement in (*wildcard_sets, *exact_sets):
            try:
                _apply_set(statement, instances, variables)
            except (ValueError, KeyError) as error:
                diagnostics.append(_semantic_error(statement.span, str(error)))
        # Whole-node links are identity/value declarations rather than ordinary
        # source-ordered assignments. Legacy Sugar-DSL applies them after every
        # field assignment so the canonical workflow captures their final meaning.
        for statement in node_links:
            try:
                _apply_node_link(statement, instances)
            except (ValueError, KeyError) as error:
                diagnostics.append(_semantic_error(statement.span, str(error)))
        for statement in activations:
            try:
                _apply_activation(statement, instances)
            except (ValueError, KeyError) as error:
                diagnostics.append(_semantic_error(statement.span, str(error)))

        if not ordered_instances and not diagnostics:
            start = SourcePosition(0, 1, 1)
            diagnostics.append(
                SugarScriptDiagnostic(
                    "sugarscript.compile.empty_workflow",
                    "SugarScript must declare at least one Cube.",
                    DiagnosticSeverity.ERROR,
                    SourceSpan(start, start),
                )
            )

        if any(item.severity is DiagnosticSeverity.ERROR for item in diagnostics):
            return SugarScriptCompileResult(None, tuple(diagnostics))
        frozen = tuple(
            CompiledCubeInstance(
                state.instance_id,
                state.alias,
                state.document.cube_id,
                state.document.version,
                state.bypassed,
                CubeDocument.from_dict(deepcopy(state.payload)),
                state.source_span,
            )
            for state in ordered_instances
        )
        return SugarScriptCompileResult(
            SugarScriptWorkflowPlan(semantic_hash, frozen, tuple(connections)),
            tuple(diagnostics),
        )

    @staticmethod
    def _add_use(
        statement: UseStatement,
        request: SugarScriptCompileRequest,
        semantic_hash: str,
        instances: dict[str, ResolvedCubeInstance],
        ordered: list[ResolvedCubeInstance],
        repeat_counters: dict[str, int],
    ) -> None:
        """Resolve and register each instance produced by one use statement."""

        document = request.cube_resolver.resolve(
            statement.cube_id, statement.version_pin
        )
        alias_base = statement.alias or statement.cube_id
        aliases = instance_aliases(statement, alias_base, repeat_counters)
        for alias in aliases:
            folded = alias.casefold()
            if folded in instances:
                raise ValueError(f"Alias '{alias}' is already declared.")
            flavor_id = _resolve_flavor(document, statement.flavor)
            payload = document.to_dict()
            implementation = payload.get("implementation")
            if not isinstance(implementation, dict):
                raise ValueError(
                    f"Cube '{document.cube_id}' has invalid implementation."
                )
            implementation["nodes"] = materialize_nodes(
                document, authored_flavor_id=flavor_id
            )
            instance = ResolvedCubeInstance(
                f"sugar-{semantic_hash[:12]}-{len(ordered) + 1}",
                alias,
                document,
                payload,
                statement.bypassed,
                statement.span,
            )
            instances[folded] = instance
            ordered.append(instance)


def _apply_set(
    statement: SetStatement,
    instances: Mapping[str, ResolvedCubeInstance],
    variables: Mapping[str, object],
) -> None:
    """Apply one exact or class-wildcard assignment after all Cubes are resolved."""

    value = evaluate_expression(
        statement.value,
        variables,
        lambda path: read_instance_value(instances, path),
    )
    if statement.target.parts[0] == "*":
        _apply_wildcard_set(statement.target, value, instances)
        return
    for path in expand_path(statement.target):
        instance, node_key, input_key = resolve_field(instances, path.parts)
        nodes = implementation_nodes(instance)
        inputs = nodes[node_key].setdefault("inputs", {})
        if not isinstance(inputs, dict):
            raise ValueError(f"Node '{instance.alias}.{node_key}' has invalid inputs.")
        inputs[input_key] = deepcopy(value)


def _apply_node_link(
    statement: SetStatement,
    instances: Mapping[str, ResolvedCubeInstance],
) -> None:
    """Apply one legacy whole-node link as validated native Cube state."""

    targets = expand_path(statement.target)
    if not isinstance(statement.value, PathExpression):
        raise ValueError("Node-link value must be a source node reference.")
    source_path = statement.value.path
    if source_path.alias_range is not None or len(source_path.parts) != 2:
        raise ValueError("Node-link value must be cube.node.")
    source_instance = resolve_instance(instances, source_path.parts[0])
    source_node_key = resolve_node(source_instance, source_path.parts[1])
    source_nodes = implementation_nodes(source_instance)
    for target_path in targets:
        target_instance = resolve_instance(instances, target_path.parts[0])
        target_node_key = resolve_node(target_instance, target_path.parts[1])
        apply_whole_node_link(
            source_nodes=source_nodes,
            source_node_key=source_node_key,
            source_label=f"{source_instance.alias}.{source_node_key}",
            target_nodes=implementation_nodes(target_instance),
            target_node_key=target_node_key,
            target_label=f"{target_instance.alias}.{target_node_key}",
        )


def _apply_wildcard_set(
    path: SugarPath,
    value: object,
    instances: Mapping[str, ResolvedCubeInstance],
) -> None:
    """Apply a wildcard to matching class/input fields across resolved Cubes."""

    if len(path.parts) != 3:
        raise ValueError("Wildcard set target must be '*.Class.input'.")
    class_selector, input_label = path.parts[1:]
    matches = 0
    for instance in instances.values():
        for node_key, node in implementation_nodes(instance).items():
            if class_selector != "*" and node.get("class_type") != class_selector:
                continue
            try:
                input_key = resolve_input(
                    instance.document, node_key, input_label, node
                )
            except KeyError:
                continue
            inputs = node.setdefault("inputs", {})
            if isinstance(inputs, dict):
                matches += 1
                if not _is_internal_graph_link(inputs.get(input_key), instance):
                    inputs[input_key] = deepcopy(value)
    if not matches:
        raise ValueError(
            f"Wildcard target '*.{class_selector}.{input_label}' matched no fields."
        )


def _apply_activation(
    statement: EnableStatement | DisableStatement,
    instances: Mapping[str, ResolvedCubeInstance],
) -> None:
    """Project explicit activation onto authored LiteGraph node mode."""

    for path in expand_path(statement.target):
        if len(path.parts) < 2:
            raise ValueError("Activation target must include Cube alias and node.")
        instance = resolve_instance(instances, path.parts[0])
        node_key = resolve_node(instance, ".".join(path.parts[1:]))
        node = implementation_nodes(instance)[node_key]
        if isinstance(statement, DisableStatement):
            node["mode"] = 4
        elif node.get("mode") == 4:
            node.pop("mode", None)


def _compile_connection(
    statement: ConnectStatement,
    instances: Mapping[str, ResolvedCubeInstance],
) -> list[CompiledCubeConnection]:
    """Resolve range-aware public boundary labels without modifying Cube internals."""

    sources = expand_path(statement.source)
    targets = expand_path(statement.target)
    if len(sources) != len(targets):
        raise ValueError("Connection ranges must contain the same number of Cubes.")
    result: list[CompiledCubeConnection] = []
    for source, target in zip(sources, targets, strict=True):
        source_instance = resolve_instance(instances, source.parts[0])
        target_instance = resolve_instance(instances, target.parts[0])
        source_binding = resolve_boundary(source_instance, source.parts[1:], "outputs")
        target_binding = resolve_boundary(target_instance, target.parts[1:], "inputs")
        result.append(
            CompiledCubeConnection(
                source_instance.instance_id,
                source_binding,
                target_instance.instance_id,
                target_binding,
            )
        )
    return result


def _resolve_flavor(document: CubeDocument, requested: str | None) -> str | None:
    """Resolve authored flavor ids and names case-insensitively."""

    if requested is None:
        return None
    matches = [
        flavor.id
        for flavor in document.flavors.authored
        if flavor.id.casefold() == requested.casefold()
        or flavor.name.casefold() == requested.casefold()
    ]
    if len(matches) != 1:
        raise KeyError(
            f"Authored flavor '{requested}' is not uniquely defined in '{document.cube_id}'."
        )
    return matches[0]


def _is_internal_graph_link(value: object, instance: ResolvedCubeInstance) -> bool:
    """Protect authored graph edges from broad literal selectors."""

    return (
        isinstance(value, list)
        and len(value) == 2
        and isinstance(value[0], str)
        and type(value[1]) is int
        and value[0] in implementation_nodes(instance)
    )


def _semantic_error(span: SourceSpan, message: str) -> SugarScriptDiagnostic:
    """Convert a caught resolution failure into one stable located diagnostic."""

    return SugarScriptDiagnostic(
        "sugarscript.compile.invalid_semantics",
        message.strip("'"),
        DiagnosticSeverity.ERROR,
        span,
    )

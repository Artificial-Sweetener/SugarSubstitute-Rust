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
"""Read the editor state a project script keeps beside its values.

The compiler writes every value a script implies into the Cube documents. A script also says
how those values came about: one value given to every Cube, a node following another Cube's
node, a seed held fixed, a Cube whose outputs are not saved. This owner records that, addressed
by stable identities, so a workflow built from a script can be written back as the same script.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping
from copy import deepcopy

from .compiler_models import (
    CompiledFieldEndpoint,
    CompiledNodeLink,
    CompiledOverride,
    CompiledPartialOverride,
    CompiledScriptState,
)
from .resolved_instances import (
    ResolvedCubeInstance,
    expand_path,
    implementation_nodes,
    resolve_field,
    resolve_input,
    resolve_instance,
    resolve_node,
)
from .syntax import (
    PathExpression,
    SetStatement,
    SugarExpression,
    SugarScriptComment,
    SugarScriptDocument,
)

_FIXED = "fixed"


def compile_node_links(
    document: SugarScriptDocument,
    instances: Mapping[str, ResolvedCubeInstance],
) -> tuple[CompiledNodeLink, ...]:
    """Return the script's node links, read before any of them is applied.

    Applying a link replaces the following node's values, so the values it held for itself
    are only readable beforehand.
    """

    node_links: list[CompiledNodeLink] = []
    for statement in document.statements:
        if (
            isinstance(statement, SetStatement)
            and statement.target.parts[0] != "*"
            and len(statement.target.parts) == 2
        ):
            try:
                node_links.extend(_node_links(statement, instances))
            except (ValueError, KeyError):
                continue
    return tuple(node_links)


def compile_script_state(
    document: SugarScriptDocument,
    instances: Mapping[str, ResolvedCubeInstance],
    evaluate: Callable[[SugarExpression], object],
    node_links: tuple[CompiledNodeLink, ...],
) -> CompiledScriptState:
    """Return the editor state of one script whose statements already resolved.

    Statements that name something the script does not declare were reported when their values
    were applied; state that cannot be resolved here is left out instead of reported twice.
    """

    overrides: list[CompiledOverride] = []
    authored: list[CompiledFieldEndpoint] = []
    for statement in document.statements:
        if not isinstance(statement, SetStatement):
            continue
        parts = statement.target.parts
        if len(parts) == 3 and parts[0] != "*":
            authored.extend(_authored_fields(statement, instances))
            continue
        if len(parts) != 3 or parts[1] != "*":
            continue
        try:
            override = _override(parts[2], evaluate(statement.value), instances)
        except (ValueError, KeyError):
            continue
        if override is not None:
            overrides.append(override)

    partial: list[CompiledPartialOverride] = []
    selections: list[tuple[str, bool]] = []
    fixed_fields: list[CompiledFieldEndpoint] = []
    fixed_overrides: list[str] = []
    unsaved: list[str] = []
    for comment in document.comments:
        payload = comment.extension_payload
        if not isinstance(payload, Mapping):
            continue
        name = comment.extension_name
        key = payload.get("key")
        if name == "global_override_value" and isinstance(key, str):
            mode = payload.get("mode")
            partial.append(
                CompiledPartialOverride(
                    key,
                    mode if isinstance(mode, str) else "global",
                    payload.get("value"),
                )
            )
        elif name == "global_override_selection" and isinstance(key, str):
            selected = payload.get("selected")
            if isinstance(selected, bool):
                selections.append((key, selected))
        elif name == "global_override_seed_control" and isinstance(key, str):
            if payload.get("mode") == _FIXED:
                fixed_overrides.append(key)
        elif name == "seed_control":
            endpoint = _seed_field(comment, instances)
            if endpoint is not None:
                fixed_fields.append(endpoint)
        elif name == "cube_output_persistence" and payload.get("saved") is False:
            alias = payload.get("alias")
            if isinstance(alias, str) and alias.casefold() in instances:
                unsaved.append(instances[alias.casefold()].instance_id)
    return CompiledScriptState(
        tuple(overrides),
        tuple(partial),
        tuple(selections),
        node_links,
        tuple(fixed_fields),
        tuple(fixed_overrides),
        tuple(unsaved),
        tuple(authored),
    )


def _authored_fields(
    statement: SetStatement,
    instances: Mapping[str, ResolvedCubeInstance],
) -> list[CompiledFieldEndpoint]:
    """Name the fields one `cube.node.input` assignment gives a value."""

    fields: list[CompiledFieldEndpoint] = []
    for path in expand_path(statement.target):
        try:
            instance, node_key, input_key = resolve_field(instances, path.parts)
        except (ValueError, KeyError):
            continue
        fields.append(CompiledFieldEndpoint(instance.instance_id, node_key, input_key))
    return fields


def _override(
    label: str,
    value: object,
    instances: Mapping[str, ResolvedCubeInstance],
) -> CompiledOverride | None:
    """Name every literal field one `*.*.input` assignment gave its value."""

    targets: list[CompiledFieldEndpoint] = []
    for instance in instances.values():
        nodes = implementation_nodes(instance)
        for node_key, node in nodes.items():
            try:
                input_key = resolve_input(instance.document, node_key, label, node)
            except KeyError:
                continue
            inputs = node.get("inputs")
            held = inputs.get(input_key) if isinstance(inputs, Mapping) else None
            if _is_graph_link(held, nodes):
                continue
            targets.append(
                CompiledFieldEndpoint(instance.instance_id, node_key, input_key)
            )
    if not targets:
        return None
    keys = {target.input_name for target in targets}
    return CompiledOverride(
        next(iter(keys)) if len(keys) == 1 else label, label, value, tuple(targets)
    )


def _node_links(
    statement: SetStatement,
    instances: Mapping[str, ResolvedCubeInstance],
) -> list[CompiledNodeLink]:
    """Name the node each target of one `cube.node = cube.node` statement follows."""

    if not isinstance(statement.value, PathExpression):
        return []
    source_path = statement.value.path
    if source_path.alias_range is not None or len(source_path.parts) != 2:
        return []
    source = resolve_instance(instances, source_path.parts[0])
    source_key = resolve_node(source, source_path.parts[1])
    source_nodes = implementation_nodes(source)
    inputs = source_nodes[source_key].get("inputs")
    names = [
        str(name)
        for name, value in (inputs.items() if isinstance(inputs, Mapping) else ())
        if not _is_graph_link(value, source_nodes)
    ]
    links: list[CompiledNodeLink] = []
    for target_path in expand_path(statement.target):
        target = resolve_instance(instances, target_path.parts[0])
        target_key = resolve_node(target, target_path.parts[1])
        held = implementation_nodes(target)[target_key].get("inputs")
        links.append(
            CompiledNodeLink(
                target.instance_id,
                target_key,
                source.instance_id,
                source_key,
                tuple(
                    (name, deepcopy(held[name]))
                    for name in names
                    if isinstance(held, Mapping) and name in held
                ),
            )
        )
    return links


def _seed_field(
    comment: SugarScriptComment,
    instances: Mapping[str, ResolvedCubeInstance],
) -> CompiledFieldEndpoint | None:
    """Resolve the field one `seed_control` comment holds fixed, named as the script names it."""

    payload = comment.extension_payload
    if not isinstance(payload, Mapping) or payload.get("mode") != _FIXED:
        return None
    path = tuple(payload.get(part) for part in ("alias", "node", "field"))
    if not all(isinstance(part, str) for part in path):
        return None
    try:
        instance, node_key, input_key = resolve_field(
            instances, tuple(str(part) for part in path)
        )
    except (ValueError, KeyError):
        return None
    return CompiledFieldEndpoint(instance.instance_id, node_key, input_key)


def _is_graph_link(value: object, nodes: Mapping[str, object]) -> bool:
    """Recognize an input fed by another node of the same Cube, which no script value owns."""

    return (
        isinstance(value, list)
        and len(value) == 2
        and isinstance(value[0], str)
        and type(value[1]) is int
        and value[0] in nodes
    )


__all__ = ["compile_node_links", "compile_script_state"]

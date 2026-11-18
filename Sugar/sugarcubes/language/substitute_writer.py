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
"""Write a workflow's Cubes as the Sugar script SugarSubstitute persisted for a project.

The statement forms, their order and the bypass marker are that application's persisted format,
kept exactly so a project saved here opens there and the reverse. This module knows nothing of
workflows or Cube documents: its caller names every Cube, node and field as the script names
them, and supplies the values to persist.
"""

from __future__ import annotations

import json
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from typing import TypeVar

from .substitute_codec import encode_literal, encode_segment, encode_version_pin

_BYPASS_NODE_MODE = 4
_Item = TypeVar("_Item")


@dataclass(frozen=True)
class ScriptFieldReference:
    """Name another Cube's field as the script addresses it."""

    alias: str
    node_label: str
    input_label: str


@dataclass(frozen=True)
class ScriptField:
    """Hold one field a Cube exposes: its script name and what the script assigns to it.

    A field assigned from another field is written as that reference; otherwise its value is
    written as a literal. `owned_by_override` marks a field whose value a workflow-wide override
    supplies, which the script writes once for every Cube instead of here.
    """

    input_key: str
    input_label: str
    value: object
    reference: ScriptFieldReference | None = None
    model_sha256: str | None = None
    lora_sha256: tuple[tuple[str, str], ...] = ()
    owned_by_override: bool = False


@dataclass(frozen=True)
class ScriptNode:
    """Hold one node of a Cube as the script sees it.

    `mode` is the node's authored execution mode and `enabled` an explicit activation the person
    chose, or `None` when they chose none. `linked_to` names the `(alias, node label)` this whole
    node takes its values from.
    """

    key: str
    label: str
    fields: tuple[ScriptField, ...] = ()
    mode: int = 0
    enabled: bool | None = None
    revealed: bool = False
    linked_to: tuple[str, str] | None = None

    @property
    def authored_bypass(self) -> bool:
        """Whether the Cube's author left this node bypassed."""

        return self.mode == _BYPASS_NODE_MODE

    @property
    def active(self) -> bool:
        """Whether the node runs: the person's choice, or else the author's."""

        return self.enabled if self.enabled is not None else not self.authored_bypass


@dataclass(frozen=True)
class ScriptCube:
    """Hold one Cube of the workflow in stack order; a missing pin follows the latest version."""

    alias: str
    cube_id: str
    version_pin: str | None
    nodes: tuple[ScriptNode, ...] = ()
    bypassed: bool = False
    save_outputs: bool = True


@dataclass(frozen=True)
class ScriptOverride:
    """Hold one workflow-wide override.

    An override every eligible field takes part in is written as one wildcard assignment under
    `label`; a partial one records its key, mode and value and leaves the assignments to its
    participants' own fields.
    """

    key: str
    label: str
    value: object
    full: bool = True
    mode: str = "global"


@dataclass(frozen=True)
class ScriptConnection:
    """Hold one explicit connection between two Cubes' boundary bindings."""

    source_alias: str
    source_binding: str
    target_alias: str
    target_binding: str


@dataclass(frozen=True)
class SubstituteScript:
    """Hold everything one project script persists, already named as the script names it."""

    cubes: tuple[ScriptCube, ...]
    overrides: tuple[ScriptOverride, ...] = ()
    override_selections: tuple[tuple[str, bool], ...] = ()
    fixed_seed_fields: tuple[ScriptFieldReference, ...] = ()
    fixed_seed_overrides: tuple[str, ...] = ()
    connections: tuple[ScriptConnection, ...] = ()


def write_substitute_script(script: SubstituteScript) -> str:
    """Return the script text: sections in persisted order, separated by one blank line."""

    aliases = [cube.alias for cube in script.cubes]
    if len(set(aliases)) != len(aliases):
        raise ValueError("A Sugar script cannot name two Cubes by the same alias.")
    known = set(aliases)
    for connection in script.connections:
        if connection.source_alias not in known or connection.target_alias not in known:
            raise ValueError("A Sugar script connection names an unknown Cube.")
    sections = [
        [_use_line(cube) for cube in script.cubes],
        [
            "# cube_output_persistence "
            + _compact({"alias": cube.alias, "saved": False})
            for cube in script.cubes
            if not cube.save_outputs
        ],
        [
            f"set *.*.{encode_segment(override.label)} = {encode_literal(override.value)}"
            for override in script.overrides
            if override.full
        ],
        [
            "# global_override_value "
            + _compact(
                {"key": override.key, "mode": override.mode, "value": override.value}
            )
            for override in script.overrides
            if not override.full
        ],
        [
            "# global_override_selection "
            + _compact({"key": key, "selected": selected})
            for key, selected in sorted(script.override_selections)
        ],
        [
            "# seed_control "
            + _compact(
                {
                    "alias": field.alias,
                    "field": field.input_label,
                    "mode": "fixed",
                    "node": field.node_label,
                }
            )
            for field in script.fixed_seed_fields
        ],
        [
            "# global_override_seed_control " + _compact({"key": key, "mode": "fixed"})
            for key in script.fixed_seed_overrides
        ],
        _activation_lines(script.cubes, "revealed"),
        _activation_lines(script.cubes, "enabled"),
        _activation_lines(script.cubes, "enable"),
        _activation_lines(script.cubes, "disable"),
        *([block] for block in map(_set_block, script.cubes) if block),
        [
            f"connect {_endpoint(connection.source_alias, connection.source_binding)} to "
            f"{_endpoint(connection.target_alias, connection.target_binding)}"
            for connection in script.connections
        ],
    ]
    return "\n\n".join("\n".join(section) for section in sections if section) + "\n"


def _use_line(cube: ScriptCube) -> str:
    """Declare one Cube, pinned when it names a version."""

    target = encode_segment(cube.cube_id)
    if cube.version_pin:
        target += f"@{encode_version_pin(cube.version_pin)}"
    return _marked(f"use {target} as {encode_segment(cube.alias)}", cube.bypassed)


def _activation_lines(cubes: Sequence[ScriptCube], kind: str) -> list[str]:
    """Collect one kind of node activation line for every Cube, in node order.

    Editor state is recorded against the node's key; the `enable` and `disable` statements
    address the node by its label, as every other statement does.
    """

    lines: list[str] = []
    for cube in cubes:
        for node in cube.nodes:
            command = _activation_command(node)
            line: str | None = None
            if kind == "revealed" and node.revealed:
                line = "# node_revealed " + _compact(
                    {"alias": cube.alias, "node": node.key}
                )
            elif kind == "enabled" and node.enabled is True and command is None:
                line = "# node_enabled " + _compact(
                    {"alias": cube.alias, "enabled": True, "node": node.key}
                )
            elif kind == command:
                line = f"{command} {encode_segment(cube.alias)}.{encode_segment(node.label)}"
            if line is not None:
                lines.append(_marked(line, cube.bypassed))
    return lines


def _activation_command(node: ScriptNode) -> str | None:
    """Name the statement that turns the author's activation into the person's, if they differ."""

    if node.authored_bypass and node.active:
        return "enable"
    if not node.authored_bypass and not node.active:
        return "disable"
    return None


def _set_block(cube: ScriptCube) -> str:
    """Write one Cube's node links and field assignments; an inactive node persists none."""

    alias = encode_segment(cube.alias)
    lines: list[str] = []
    for node in _positive_before_negative(
        cube.nodes, lambda node: (node.key, node.label)
    ):
        if not node.active:
            continue
        node_path = f"{alias}.{encode_segment(node.label)}"
        if node.linked_to is not None:
            source_alias, source_node = node.linked_to
            lines.append(
                f"set {node_path} = "
                f"{encode_segment(source_alias)}.{encode_segment(source_node)}"
            )
        for field in _positive_before_negative(
            node.fields, lambda field: (field.input_key,)
        ):
            lines.extend(_field_lines(node_path, field))
    return _marked("\n".join(lines), cube.bypassed) if lines else ""


def _field_lines(node_path: str, field: ScriptField) -> list[str]:
    """Write one assignment and the integrity comments that belong directly beneath it."""

    if field.owned_by_override:
        return []
    prefix = f"set {node_path}.{encode_segment(field.input_label)} = "
    if field.reference is not None:
        reference = field.reference
        lines = [
            prefix
            + ".".join(
                encode_segment(part)
                for part in (
                    reference.alias,
                    reference.node_label,
                    reference.input_label,
                )
            )
        ]
    elif isinstance(field.value, dict):
        # A mapping is a graph connection, never a value a person authored.
        return []
    else:
        lines = [prefix + encode_literal(field.value)]
    if field.model_sha256:
        lines.append(f"# sha256 {field.model_sha256.upper()}")
    if field.reference is not None or isinstance(field.value, str):
        lines.extend(
            "# lora_sha256 "
            + _compact({"name": name.strip(), "sha256": sha256.upper()})
            for name, sha256 in field.lora_sha256
            if name.strip()
        )
    return lines


def _endpoint(alias: str, binding: str) -> str:
    """Address one boundary binding, such as `output.image`, on a Cube."""

    parts = binding.split(".")
    # A binding qualified by its Cube names the Cube again; the alias already does.
    if len(parts) >= 2 and parts[1] in {"input", "output"}:
        parts = parts[1:]
    parts = [part for part in parts if part]
    return ".".join([encode_segment(alias), *map(encode_segment, parts)])


def _positive_before_negative(
    items: Sequence[_Item], text: Callable[[_Item], tuple[str, ...]]
) -> list[_Item]:
    """Keep the given order, except that positive prompts precede the first negative one."""

    ordered = list(items)
    polarities = [_prompt_polarity(*text(item)) for item in ordered]
    if "negative" not in polarities:
        return ordered
    first_negative = polarities.index("negative")
    moved = [
        item
        for index, item in enumerate(ordered)
        if index > first_negative and polarities[index] == "positive"
    ]
    if not moved:
        return ordered
    kept = [
        item
        for index, item in enumerate(ordered)
        if not (index > first_negative and polarities[index] == "positive")
    ]
    return [*kept[:first_negative], *moved, *kept[first_negative:]]


def _prompt_polarity(*parts: str) -> str | None:
    """Classify a name as a positive or negative prompt when it says exactly one of them."""

    text = " ".join(parts).casefold()
    if "prompt" not in text:
        return None
    positive, negative = "positive" in text, "negative" in text
    if positive == negative:
        return None
    return "positive" if positive else "negative"


def _marked(statement: str, bypassed: bool) -> str:
    """Persist a bypassed Cube's statement behind the marker, one physical line at a time."""

    if not bypassed:
        return statement
    return "\n".join(
        f"# bypass {line}" if line else "# bypass" for line in statement.splitlines()
    )


def _compact(payload: dict[str, object]) -> str:
    """Encode one metadata payload as the persisted compact, key-sorted JSON."""

    return json.dumps(payload, sort_keys=True, separators=(",", ":"))


__all__ = [
    "ScriptConnection",
    "ScriptCube",
    "ScriptField",
    "ScriptFieldReference",
    "ScriptNode",
    "ScriptOverride",
    "SubstituteScript",
    "write_substitute_script",
]

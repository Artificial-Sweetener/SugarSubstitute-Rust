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
"""Own how a workflow carries the editor state of the project script it came from.

A workflow built from a script records that state in its composition metadata. A workflow the
page has since edited also holds the page's own relation state, which then decides overrides
and node links; what the page has no place for is still read from the composition.
"""

from __future__ import annotations

import re
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
import json

from ..language.compiler_models import CompiledFieldEndpoint, CompiledScriptState

_COMPOSITION_KEY = "sugarcubes_composition"
_RELATIONS_KEY = "sugarcubes_relations"
_SCRIPT_STATE_KEY = "script_state"
_SEMANTIC_ALIASES = {
    "noise_seed": "seed",
    "sampler": "sampler_name",
    "sampling_method": "sampler_name",
    "cfg_scale": "cfg",
    "guidance_scale": "cfg",
}

FieldIdentity = tuple[str, str, str]
"""A Cube instance, a node symbol inside it, and an input of that node."""

NodeIdentity = tuple[str, str]
"""A Cube instance and a node symbol inside it."""


@dataclass(frozen=True)
class WorkflowOverride:
    """One value every Cube takes for the same field.

    `fields` names the fields it drives. It is `None` for an override the page keeps, which
    drives every field of its meaning.
    """

    key: str
    label: str
    value: object
    fields: frozenset[FieldIdentity] | None

    def owns(self, identity: FieldIdentity) -> bool:
        """Report whether this override supplies the named field's value."""

        if self.fields is not None:
            return identity in self.fields
        return field_meaning(identity[2]) == self.key


@dataclass(frozen=True)
class WorkflowScriptState:
    """Hold the script state one workflow carries, addressed by stable identities.

    `own_values` holds, for a field of a node that follows another, the value the field keeps
    for itself; the workflow's own field holds the value it follows. `authored_fields` names
    the fields the script assigned itself.
    """

    overrides: tuple[WorkflowOverride, ...] = ()
    node_links: Mapping[NodeIdentity, NodeIdentity] = field(default_factory=dict)
    own_values: Mapping[FieldIdentity, object] = field(default_factory=dict)
    partial_overrides: tuple[tuple[str, str, object], ...] = ()
    override_selections: tuple[tuple[str, bool], ...] = ()
    fixed_seed_fields: tuple[FieldIdentity, ...] = ()
    fixed_seed_overrides: tuple[str, ...] = ()
    authored_fields: frozenset[FieldIdentity] = frozenset()


def field_meaning(input_name: str) -> str:
    """Name what an input means across Cubes, as the page groups fields into overrides."""

    spaced = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", input_name).strip().lower()
    normalized = re.sub(r"[^a-z0-9]+", "_", spaced).strip("_")
    return _SEMANTIC_ALIASES.get(normalized, normalized)


def script_state_metadata(state: CompiledScriptState) -> dict[str, object]:
    """Return the composition entries that record one compiled script's editor state."""

    relations: list[dict[str, object]] = [
        {
            "relation_id": f"override:{override.key}",
            "kind": "global_override",
            "key": override.key,
            "label": override.label,
            "value": override.value,
            "targets": [_endpoint(target) for target in override.targets],
        }
        for override in state.overrides
    ]
    for link in state.node_links:
        for name, own_value in link.own_values:
            relations.append(
                {
                    "relation_id": (
                        f"link:{link.target_instance_id}:"
                        f"{link.target_node_symbol}:{name}"
                    ),
                    "kind": "field_link",
                    "source": {
                        "instance_id": link.source_instance_id,
                        "node_symbol": link.source_node_symbol,
                        "input_name": name,
                    },
                    "targets": [
                        {
                            "instance_id": link.target_instance_id,
                            "node_symbol": link.target_node_symbol,
                            "input_name": name,
                        }
                    ],
                    "own_value": own_value,
                }
            )
    carried: dict[str, object] = {
        "partial_overrides": [
            {"key": item.key, "mode": item.mode, "value": item.value}
            for item in state.partial_overrides
        ],
        "override_selections": [
            {"key": key, "selected": selected}
            for key, selected in state.override_selections
        ],
        "fixed_seed_fields": [_endpoint(target) for target in state.fixed_seed_fields],
        "fixed_seed_overrides": list(state.fixed_seed_overrides),
        # A script may assign a field twice and in any order; the record is a set.
        "authored_fields": [
            _endpoint(target)
            for target in sorted(
                set(state.authored_fields),
                key=lambda target: (
                    target.instance_id,
                    target.node_symbol,
                    target.input_name,
                ),
            )
        ],
    }
    carried = {name: entries for name, entries in carried.items() if entries}
    return {
        **({"value_relations": relations} if relations else {}),
        **({_SCRIPT_STATE_KEY: carried} if carried else {}),
    }


def read_workflow_script_state(workflow: Mapping[str, object]) -> WorkflowScriptState:
    """Read the script state a workflow carries; unreadable entries are left out."""

    extra = workflow.get("extra")
    composition = extra.get(_COMPOSITION_KEY) if isinstance(extra, Mapping) else None
    composition = composition if isinstance(composition, Mapping) else {}
    carried = composition.get(_SCRIPT_STATE_KEY)
    carried = carried if isinstance(carried, Mapping) else {}
    page = extra.get(_RELATIONS_KEY) if isinstance(extra, Mapping) else None
    fixed_overrides = tuple(
        key
        for key in _records(carried.get("fixed_seed_overrides"))
        if isinstance(key, str)
    )
    if isinstance(page, Mapping) and page.get("schema") == 1:
        overrides, fixed_overrides = _page_overrides(page.get("overrides"))
        node_links = _page_node_links(page.get("nodeLinks"))
        own_values = _page_own_values(page.get("locals"))
    else:
        overrides, node_links, own_values = _composed_relations(
            composition.get("value_relations")
        )
    partial = tuple(
        (
            str(item["key"]),
            str(item["mode"]) if isinstance(item.get("mode"), str) else "global",
            item.get("value"),
        )
        for item in _records(carried.get("partial_overrides"))
        if isinstance(item, Mapping) and isinstance(item.get("key"), str)
    )
    selections = tuple(
        (str(item["key"]), bool(item["selected"]))
        for item in _records(carried.get("override_selections"))
        if isinstance(item, Mapping)
        and isinstance(item.get("key"), str)
        and isinstance(item.get("selected"), bool)
    )
    fixed_fields = tuple(
        identity
        for identity in map(_field_identity, _records(carried.get("fixed_seed_fields")))
        if identity is not None
    )
    return WorkflowScriptState(
        overrides,
        node_links,
        own_values,
        partial,
        selections,
        fixed_fields,
        fixed_overrides,
        frozenset(
            identity
            for identity in map(
                _field_identity, _records(carried.get("authored_fields"))
            )
            if identity is not None
        ),
    )


def _page_overrides(
    value: object,
) -> tuple[tuple[WorkflowOverride, ...], tuple[str, ...]]:
    """Read the overrides the page keeps switched on, and which of them hold a fixed seed.

    The page names a second group of one meaning `meaning:field`; a script has one override
    per meaning, so only the first group is one.
    """

    overrides: list[WorkflowOverride] = []
    fixed: list[str] = []
    for key, state in value.items() if isinstance(value, Mapping) else ():
        if (
            not isinstance(key, str)
            or ":" in key
            or not isinstance(state, Mapping)
            or state.get("enabled") is not True
        ):
            continue
        overrides.append(WorkflowOverride(key, key, state.get("value"), None))
        if key == "seed" and state.get("seedMode") == "fixed":
            fixed.append(key)
    return tuple(overrides), tuple(fixed)


def _page_node_links(value: object) -> dict[NodeIdentity, NodeIdentity]:
    """Read the page's node links, which name each node as a JSON identity."""

    links: dict[NodeIdentity, NodeIdentity] = {}
    for target, source in value.items() if isinstance(value, Mapping) else ():
        target_node, source_node = _page_node(target), _page_node(source)
        if target_node is not None and source_node is not None:
            links[target_node] = source_node
    return links


def _page_own_values(value: object) -> dict[FieldIdentity, object]:
    """Read the values the page keeps for fields that currently show another value.

    The page names a field `[node identity, input]`, itself as JSON.
    """

    own: dict[FieldIdentity, object] = {}
    for name, held in value.items() if isinstance(value, Mapping) else ():
        try:
            parts = json.loads(name) if isinstance(name, str) else None
        except ValueError:
            continue
        if not isinstance(parts, list) or len(parts) != 2:
            continue
        node = _page_node(parts[0])
        if node is not None and isinstance(parts[1], str):
            own[(*node, parts[1])] = held
    return own


def _page_node(value: object) -> NodeIdentity | None:
    """Parse one `["cube", instance, symbol]` identity."""

    if not isinstance(value, str):
        return None
    try:
        parts = json.loads(value)
    except ValueError:
        return None
    if (
        isinstance(parts, list)
        and len(parts) == 3
        and parts[0] == "cube"
        and isinstance(parts[1], str)
        and isinstance(parts[2], str)
    ):
        return parts[1], parts[2]
    return None


def _composed_relations(
    value: object,
) -> tuple[
    tuple[WorkflowOverride, ...],
    dict[NodeIdentity, NodeIdentity],
    dict[FieldIdentity, object],
]:
    """Read overrides, node links and followers' own values from composition relations."""

    overrides: list[WorkflowOverride] = []
    links: dict[NodeIdentity, NodeIdentity] = {}
    own: dict[FieldIdentity, object] = {}
    for relation in _records(value):
        if not isinstance(relation, Mapping):
            continue
        targets = [
            identity
            for identity in map(_field_identity, _records(relation.get("targets")))
            if identity is not None
        ]
        kind = relation.get("kind")
        if kind == "global_override" and targets and "value" in relation:
            key, label = relation.get("key"), relation.get("label")
            name = key if isinstance(key, str) and key else targets[0][2]
            overrides.append(
                WorkflowOverride(
                    name,
                    label if isinstance(label, str) and label else name,
                    relation["value"],
                    frozenset(targets),
                )
            )
        elif kind == "field_link":
            source = _field_identity(relation.get("source"))
            if source is None:
                continue
            for target in targets:
                if target[:2] != source[:2]:
                    links[target[:2]] = source[:2]
                    if "own_value" in relation:
                        own[target] = relation["own_value"]
    return tuple(overrides), links, own


def _field_identity(value: object) -> FieldIdentity | None:
    """Parse one stable field endpoint."""

    if not isinstance(value, Mapping):
        return None
    parts = tuple(
        value.get(key) for key in ("instance_id", "node_symbol", "input_name")
    )
    if not all(isinstance(part, str) and part for part in parts):
        return None
    return str(parts[0]), str(parts[1]), str(parts[2])


def _endpoint(target: CompiledFieldEndpoint) -> dict[str, str]:
    """Write one stable field endpoint."""

    return {
        "instance_id": target.instance_id,
        "node_symbol": target.node_symbol,
        "input_name": target.input_name,
    }


def _records(value: object) -> Sequence[object]:
    """Return a JSON array's items, or none for any other value."""

    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        return value
    return ()


__all__ = [
    "FieldIdentity",
    "NodeIdentity",
    "WorkflowOverride",
    "WorkflowScriptState",
    "field_meaning",
    "read_workflow_script_state",
    "script_state_metadata",
]

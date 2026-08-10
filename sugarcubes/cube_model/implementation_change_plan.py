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

"""Describe every implementation-save outcome behind aggregate default choices."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from copy import deepcopy
from dataclasses import dataclass
from pathlib import PurePosixPath
from typing import Any, Literal

from .default_change_plan import build_default_change_plan
from .default_merge import merge_implementation_save_defaults
from .document import CubeDocument

ImplementationChangeDecision = Literal[
    "always", "overwrite_defaults", "save_prompt_fields"
]
_IDENTITY_KEYS = ("control_id", "id", "symbol", "name")
_MISSING = object()


@dataclass(frozen=True)
class ImplementationChange:
    """Represent one human-readable persisted-document difference."""

    section: str
    label: str
    path: str
    previous_value: Any
    proposed_value: Any
    previous_exists: bool
    proposed_exists: bool
    decision: ImplementationChangeDecision

    def to_dict(self) -> dict[str, Any]:
        """Return a transport-safe change description."""

        return {
            "section": self.section,
            "label": self.label,
            "path": self.path,
            "previous_value": deepcopy(self.previous_value),
            "proposed_value": deepcopy(self.proposed_value),
            "previous_exists": self.previous_exists,
            "proposed_exists": self.proposed_exists,
            "decision": self.decision,
        }


@dataclass(frozen=True)
class ImplementationChangePlan:
    """Carry the complete commit diff and its two aggregate default decisions."""

    cube_id: str
    display_name: str
    fingerprint: str
    overwrite_default_count: int
    prompt_default_count: int
    changes: tuple[ImplementationChange, ...]

    @property
    def requires_default_decision(self) -> bool:
        """Return whether the save needs author input about current defaults."""

        return self.overwrite_default_count > 0 or self.prompt_default_count > 0

    def to_dict(self) -> dict[str, Any]:
        """Return the backend preview payload."""

        return {
            "cube_id": self.cube_id,
            "display_name": self.display_name,
            "fingerprint": self.fingerprint,
            "requires_default_decision": self.requires_default_decision,
            "overwrite_default_count": self.overwrite_default_count,
            "prompt_default_count": self.prompt_default_count,
            "changes": [change.to_dict() for change in self.changes],
        }


def build_implementation_change_plan(
    existing: CubeDocument | None,
    exported: CubeDocument,
) -> ImplementationChangePlan:
    """Compare every possible persisted result without mutating either document."""

    default_plan = build_default_change_plan(existing, exported)
    overwrite_ids = frozenset(
        change.control_id for change in default_plan.changes if not change.is_multiline
    )
    prompt_ids = frozenset(
        change.control_id for change in default_plan.changes if change.is_multiline
    )
    baseline = merge_implementation_save_defaults(existing, exported)
    overwritten = merge_implementation_save_defaults(
        existing,
        exported,
        overwrite_control_ids=overwrite_ids,
    )
    with_prompts = merge_implementation_save_defaults(
        existing,
        exported,
        overwrite_control_ids=prompt_ids,
    )
    previous = existing.to_dict() if existing is not None else {}
    controls = _control_labels(exported)
    nodes = _node_labels(exported)
    changes = [
        *_diff_documents(
            previous,
            baseline.to_dict(),
            decision="always",
            controls=controls,
            nodes=nodes,
        ),
        *_diff_documents(
            baseline.to_dict(),
            overwritten.to_dict(),
            decision="overwrite_defaults",
            controls=controls,
            nodes=nodes,
        ),
        *_diff_documents(
            baseline.to_dict(),
            with_prompts.to_dict(),
            decision="save_prompt_fields",
            controls=controls,
            nodes=nodes,
        ),
    ]
    return ImplementationChangePlan(
        cube_id=exported.cube_id,
        display_name=_display_name(exported),
        fingerprint=default_plan.fingerprint,
        overwrite_default_count=len(overwrite_ids),
        prompt_default_count=len(prompt_ids),
        changes=tuple(changes),
    )


def _diff_documents(
    previous: Mapping[str, Any],
    proposed: Mapping[str, Any],
    *,
    decision: ImplementationChangeDecision,
    controls: Mapping[str, str],
    nodes: Mapping[str, str],
) -> list[ImplementationChange]:
    """Return stable semantic changes while omitting repository identity noise."""

    changes: list[ImplementationChange] = []
    _diff_value(
        previous,
        proposed,
        path=(),
        decision=decision,
        controls=controls,
        nodes=nodes,
        changes=changes,
    )
    return [change for change in changes if change.path != "cube_id"]


def _diff_value(
    previous: Any,
    proposed: Any,
    *,
    path: tuple[str, ...],
    decision: ImplementationChangeDecision,
    controls: Mapping[str, str],
    nodes: Mapping[str, str],
    changes: list[ImplementationChange],
) -> None:
    """Walk mappings and identified collections, collapsing atomic replacements."""

    if previous is not _MISSING and proposed is not _MISSING and previous == proposed:
        return
    if isinstance(previous, Mapping) and isinstance(proposed, Mapping):
        for key in sorted(set(previous) | set(proposed), key=str):
            _diff_value(
                previous.get(key, _MISSING),
                proposed.get(key, _MISSING),
                path=(*path, str(key)),
                decision=decision,
                controls=controls,
                nodes=nodes,
                changes=changes,
            )
        return
    identified = _identified_collections(previous, proposed)
    if identified is not None:
        previous_items, proposed_items = identified
        for key in sorted(set(previous_items) | set(proposed_items)):
            _diff_value(
                previous_items.get(key, _MISSING),
                proposed_items.get(key, _MISSING),
                path=(*path, key),
                decision=decision,
                controls=controls,
                nodes=nodes,
                changes=changes,
            )
        return
    section, label = _describe_change(path, previous, proposed, controls, nodes)
    changes.append(
        ImplementationChange(
            section=section,
            label=label,
            path=".".join(path),
            previous_value=None if previous is _MISSING else deepcopy(previous),
            proposed_value=None if proposed is _MISSING else deepcopy(proposed),
            previous_exists=previous is not _MISSING,
            proposed_exists=proposed is not _MISSING,
            decision=decision,
        )
    )


def _identified_collections(
    previous: Any,
    proposed: Any,
) -> tuple[dict[str, Any], dict[str, Any]] | None:
    """Index object lists by a shared stable identity when possible."""

    if not isinstance(previous, list) or not isinstance(proposed, list):
        return None
    combined = [*previous, *proposed]
    if not combined or any(not isinstance(item, Mapping) for item in combined):
        return None
    for identity_key in _IDENTITY_KEYS:
        previous_ids = [item.get(identity_key) for item in previous]
        proposed_ids = [item.get(identity_key) for item in proposed]
        identities = [*previous_ids, *proposed_ids]
        if (
            all(isinstance(identity, str) and identity for identity in identities)
            and len(previous_ids) == len(set(previous_ids))
            and len(proposed_ids) == len(set(proposed_ids))
        ):
            return (
                {str(item[identity_key]): item for item in previous},
                {str(item[identity_key]): item for item in proposed},
            )
    return None


def _describe_change(
    path: Sequence[str],
    previous: Any,
    proposed: Any,
    controls: Mapping[str, str],
    nodes: Mapping[str, str],
) -> tuple[str, str]:
    """Project canonical paths into concise product-facing sections and labels."""

    parts = list(path)
    if len(parts) >= 5 and parts[:2] == ["flavors", "authored"] and "values" in parts:
        control_id = parts[-1]
        return "Defaults", controls.get(control_id, _humanize(control_id))
    if len(parts) >= 3 and parts[:2] == ["implementation", "nodes"]:
        symbol = parts[2]
        node_label = nodes.get(symbol) or _mapping_label(previous, proposed) or _humanize(symbol)
        if len(parts) == 3:
            return "Nodes", node_label
        if len(parts) >= 5 and parts[3] == "inputs":
            return "Node settings", f"{node_label} · {_humanize(parts[4])}"
        return "Nodes", f"{node_label} · {_humanize(parts[-1])}"
    if parts[:2] == ["implementation", "inputs"] or parts[:2] == [
        "implementation",
        "outputs",
    ]:
        return "Cube interface", _humanize(parts[-1])
    if parts[:2] == ["implementation", "layout"] or parts[:2] == [
        "metadata",
        "surface_state",
    ]:
        return "Layout", _humanize(parts[-1])
    if parts[:2] == ["implementation", "definitions"]:
        return "Node definitions", _humanize(parts[-1])
    if parts[:2] == ["implementation", "subgraphs"]:
        return "Subgraphs", _mapping_label(previous, proposed) or _humanize(parts[-1])
    if parts and parts[0] == "surface":
        return "Controls", _mapping_label(previous, proposed) or _humanize(parts[-1])
    if parts and parts[0] == "metadata":
        return "Cube settings", _humanize(parts[-1])
    if parts and parts[0] == "flavors":
        return "Flavors", _mapping_label(previous, proposed) or _humanize(parts[-1])
    return "Cube", _humanize(parts[-1] if parts else "implementation")


def _mapping_label(previous: Any, proposed: Any) -> str:
    """Read the most useful label from an added or removed object."""

    for value in (proposed, previous):
        if not isinstance(value, Mapping):
            continue
        for key in ("label", "name", "control_id", "id"):
            candidate = value.get(key)
            if isinstance(candidate, str) and candidate.strip():
                return candidate.strip()
    return ""


def _control_labels(document: CubeDocument) -> dict[str, str]:
    """Index product labels for stable surface controls."""

    return {
        control.control_id: control.label or _humanize(control.input_name)
        for control in document.surface.controls
    }


def _node_labels(document: CubeDocument) -> dict[str, str]:
    """Index product labels for implementation nodes."""

    labels: dict[str, str] = {}
    for symbol, node in document.implementation.nodes.items():
        label = node.get("label")
        labels[symbol] = label.strip() if isinstance(label, str) and label.strip() else _humanize(symbol)
    return labels


def _display_name(document: CubeDocument) -> str:
    """Return a friendly cube name without repository or directory identity."""

    alias = document.metadata.get("default_alias")
    if isinstance(alias, str) and alias.strip():
        return alias.strip()
    filename = PurePosixPath(document.cube_id.replace("\\", "/")).name
    return filename[:-5] if filename.casefold().endswith(".cube") else filename


def _humanize(value: str) -> str:
    """Turn one canonical key into a concise display label."""

    return value.replace("_", " ").replace(".", " · ").strip().capitalize()

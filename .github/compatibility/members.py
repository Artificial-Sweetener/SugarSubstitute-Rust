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
"""Connect changed native object members to the SugarCubes expressions that consume them."""

from __future__ import annotations

import json
import re
from pathlib import Path


def change_sites(
    owner: str,
    member: str,
    inventory: dict[str, list[str]],
    sites: dict[tuple[str, str], list[int]],
) -> list[dict[str, object]]:
    """Resolve both typed and receiver-qualified expressions without matching unrelated widget fields."""
    aliases = {
        "app": {"app", "appref", "apprecord", "comfyapp", "comfyapplication"},
        "graph": {"graph", "comfygraph"},
        "canvas": {"canvas", "graphcanvas", "comfycanvas"},
        "node": {"node", "nodeData", "comfynode"},
        "widget": {"widget", "comfywidget"},
        "output": {"output", "comfyoutput"},
        "litegraph": {"litegraph", "litegraphhost"},
        "workflow": {"workflow", "workflowstore"},
    }.get(owner, {owner})
    receivers = "|".join(re.escape(value) for value in sorted(aliases))
    pattern = re.compile(
        rf"(?<![\w$])(?:{receivers})\??\.{re.escape(member)}(?![\w$])", re.IGNORECASE
    )
    consumers = []
    for path, entries in inventory.items():
        for entry in entries:
            if entry.startswith(
                ("host member ", "typed host member ", "host adapter member ")
            ) and pattern.search(entry):
                consumers.append(
                    {
                        "source": path,
                        "lines": sites.get((path, entry), []),
                        "expression": entry,
                    }
                )
    return consumers


def changed_members(
    before: Path,
    after: Path,
    inventory: dict[str, list[str]],
    sites: dict[tuple[str, str], list[int]],
) -> list[dict[str, object]]:
    """Compare captured native object shapes and keep additions as well as removals."""
    control, candidate = (
        json.loads(p.read_text(encoding="utf-8")) for p in (before, after)
    )
    if any(
        not isinstance(shape, dict)
        or any(
            not isinstance(values, list)
            or not all(isinstance(value, str) for value in values)
            for values in shape.values()
        )
        for shape in (control, candidate)
    ):
        raise ValueError("Malformed native member snapshot")
    changes = []
    for owner in sorted(set(control) | set(candidate)):
        previous, current = set(control.get(owner, [])), set(candidate.get(owner, []))
        for member in sorted(previous ^ current):
            changes.append(
                {
                    "member": f"{owner}.{member}",
                    "control": member in previous,
                    "candidate": member in current,
                    "consumers": change_sites(owner, member, inventory, sites),
                }
            )
    return changes

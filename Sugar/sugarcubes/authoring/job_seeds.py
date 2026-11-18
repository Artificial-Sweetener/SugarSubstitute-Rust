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
"""Read the seeds one finished or queued job ran its Cubes with.

Comfy records, for every job, the prompt it executed and the workflow it came from. A Cube's
nodes run under identifiers that begin with the Cube instance's identity, and the workflow
names each instance. Together they say which seed each named Cube used, so those seeds can
be taken into another workflow that holds Cubes of the same names.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass

from .workflow_script_state import field_meaning

_SEED = "seed"


@dataclass(frozen=True)
class JobSeed:
    """One seed a job gave one node of one named Cube, as exact decimal digits."""

    alias: str
    node_symbol: str
    input_name: str
    digits: str


def read_job_seeds(history: object) -> tuple[JobSeed, ...]:
    """Return the seeds of the one job a Comfy history reply describes.

    Args:
        history: Comfy's history reply for one job: either the job's own record or a mapping
            holding that record under the job's identifier.

    Returns:
        One entry per Cube node input that means a seed and holds a whole number, in the
        order the job's prompt lists them. A reply that describes no Cube job yields none.
    """

    record = _job_record(history)
    prompt = record.get("prompt") if record is not None else None
    if (
        not isinstance(prompt, Sequence)
        or isinstance(prompt, (str, bytes))
        or len(prompt) < 4
    ):
        return ()
    nodes, extra = prompt[2], prompt[3]
    aliases = _instance_aliases(extra)
    if not isinstance(nodes, Mapping) or not aliases:
        return ()
    # A longer identity is tried first, so one instance's identity never claims another's node.
    ordered = sorted(aliases, key=len, reverse=True)
    seeds: dict[tuple[str, str, str], JobSeed] = {}
    for node_id, node in nodes.items():
        inputs = node.get("inputs") if isinstance(node, Mapping) else None
        if not isinstance(node_id, str) or not isinstance(inputs, Mapping):
            continue
        instance = next(
            (candidate for candidate in ordered if node_id.startswith(f"{candidate}:")),
            None,
        )
        if instance is None:
            continue
        symbol = node_id[len(instance) + 1 :].split(":", 1)[0]
        for name, value in inputs.items():
            if (
                isinstance(name, str)
                and field_meaning(name) == _SEED
                and isinstance(value, int)
                and not isinstance(value, bool)
            ):
                key = (aliases[instance], symbol, name)
                seeds.setdefault(key, JobSeed(*key, str(value)))
    return tuple(seeds.values())


def _job_record(history: object) -> Mapping[str, object] | None:
    """Find the job's own record in a history reply."""

    if not isinstance(history, Mapping):
        return None
    if "prompt" in history:
        return history
    for value in history.values():
        if isinstance(value, Mapping) and "prompt" in value:
            return value
    return None


def _instance_aliases(extra: object) -> dict[str, str]:
    """Name each Cube instance of the workflow a job was queued from."""

    info = extra.get("extra_pnginfo") if isinstance(extra, Mapping) else None
    workflow = info.get("workflow") if isinstance(info, Mapping) else None
    nodes = workflow.get("nodes") if isinstance(workflow, Mapping) else None
    aliases: dict[str, str] = {}
    if not isinstance(nodes, Sequence) or isinstance(nodes, (str, bytes)):
        return aliases
    for node in nodes:
        properties = node.get("properties") if isinstance(node, Mapping) else None
        cube = (
            properties.get("sugarcubes_cube")
            if isinstance(properties, Mapping)
            else None
        )
        if not isinstance(cube, Mapping):
            continue
        instance, alias = cube.get("instance_id"), cube.get("instance_alias")
        if isinstance(instance, str) and instance and isinstance(alias, str) and alias:
            aliases[instance] = alias
    return aliases


__all__ = ["JobSeed", "read_job_seeds"]

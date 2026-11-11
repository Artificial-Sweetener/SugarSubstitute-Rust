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
"""Derive isolated spatial experiments while preserving the authored graph."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from copy import deepcopy
import math

from .models import ApiPrompt
from .spatial_probe_values import (
    SpatialFieldTarget,
    SpatialNodeContract,
    SpatialProbeExecution,
    SpatialProbeOutput,
    SpatialProbePlan,
    SpatialSource,
)

MEASUREMENT_CLASS = "SugarCubes.SpatialProbeMeasurement"
MEASUREMENT_NODE = "sugarcubes-spatial-probe-measurement"
MAX_PROBE_NODES = 128


def _link(value: object, prompt: ApiPrompt) -> tuple[str, int] | None:
    """Distinguish actual prompt links from ordinary list-valued widgets."""
    if isinstance(value, (list, tuple)) and len(value) == 2:
        node, slot = value
        if isinstance(node, str) and node in prompt and type(slot) is int and slot >= 0:
            return node, slot
    return None


def _inputs(node: Mapping[str, object]) -> Mapping[str, object]:
    """Narrow one admitted node input map before traversing its dependencies."""
    value = node.get("inputs")
    if not isinstance(value, Mapping) or any(not isinstance(key, str) for key in value):
        raise ValueError("The compiled workflow contains invalid node inputs.")
    return {str(key): item for key, item in value.items()}


class SpatialProbeGraph:
    """Own source attribution, path selection and bounded execution-only graph rewriting."""

    def __init__(self, contract: Callable[[str], SpatialNodeContract]) -> None:
        """Read registry contracts through an injected native adapter."""
        self._contract = contract

    def _node_contract(self, node: Mapping[str, object]) -> SpatialNodeContract:
        """Refuse unregistered classes instead of inferring behavior from display names."""
        name = node.get("class_type")
        if not isinstance(name, str) or not name or name == MEASUREMENT_CLASS:
            raise ValueError(
                "The compiled workflow contains an unsupported node class."
            )
        return self._contract(name)

    def _ancestors(self, prompt: ApiPrompt, node_id: str) -> set[str]:
        """Collect one actual dependency path and reject cycles and excessive work."""
        result: set[str] = set()
        active: set[str] = set()

        def visit(current: str) -> None:
            if current in active:
                raise ValueError(
                    "Scale discovery does not support cyclic execution paths."
                )
            if current in result:
                return
            if current not in prompt:
                raise ValueError("The selected output has a missing dependency.")
            active.add(current)
            for value in _inputs(prompt[current]).values():
                link = _link(value, prompt)
                if link:
                    visit(link[0])
            active.remove(current)
            result.add(current)
            if len(result) > MAX_PROBE_NODES:
                raise ValueError(
                    "This output path exceeds the bounded probe node budget."
                )

        visit(node_id)
        return result

    def prepare(
        self,
        prompt: ApiPrompt,
        width: tuple[SpatialFieldTarget, ...],
        height: tuple[SpatialFieldTarget, ...],
    ) -> SpatialProbePlan:
        """Capture an independent prompt and enumerate relevant typed output paths without execution."""
        copied = deepcopy(prompt)
        sources: list[SpatialSource] = []
        for first in width:
            for second in height:
                if first.node_id != second.node_id or first.node_id not in copied:
                    continue
                node = copied[first.node_id]
                contract = self._node_contract(node)
                if not any(kind in ("IMAGE", "LATENT") for kind in contract.outputs):
                    continue
                values = _inputs(node)
                if not all(
                    type(values.get(name)) in (int, float)
                    for name in (first.input_name, second.input_name)
                ):
                    continue
                source = SpatialSource(
                    first.node_id, first.input_name, second.input_name
                )
                if source not in sources:
                    sources.append(source)
        if not sources:
            raise ValueError(
                "These dimension fields do not identify an editable spatial source."
            )
        source_ids = {source.node_id for source in sources}
        outputs: list[SpatialProbeOutput] = []
        for sink_id, node in copied.items():
            if not self._node_contract(node).output_node:
                continue
            for name, value in _inputs(node).items():
                link = _link(value, copied)
                if not link:
                    continue
                upstream = self._node_contract(copied[link[0]])
                kind = (
                    upstream.outputs[link[1]] if link[1] < len(upstream.outputs) else ""
                )
                # Canvas dimensions are pixels; a latent output has no pixel frame until decoded.
                if kind != "IMAGE":
                    continue
                if not self._ancestors(copied, link[0]).intersection(source_ids):
                    continue
                meta = node.get("_meta")
                title = meta.get("title") if isinstance(meta, Mapping) else None
                label = (
                    title
                    if isinstance(title, str) and title
                    else str(node.get("class_type"))
                )
                outputs.append(
                    SpatialProbeOutput(
                        f"{sink_id}/{name}",
                        f"{label} · {sink_id}"[:256],
                        link[0],
                        link[1],
                        "IMAGE",
                    )
                )
        if len(outputs) > 64:
            raise ValueError(
                "Choose a workflow with fewer spatial output paths for discovery."
            )
        return SpatialProbePlan(copied, tuple(sources), tuple(outputs))

    def build(self, plan: SpatialProbePlan, output_id: str) -> SpatialProbeExecution:
        """Replace output sinks with one measurement and probe two small frames in a single queued job."""
        output = next((item for item in plan.outputs if item.id == output_id), None)
        if output is None:
            raise ValueError("The selected probe output is unavailable.")
        prompt = deepcopy(plan.prompt)

        def resolve(value: object, active: frozenset[str] = frozenset()) -> object:
            link = _link(value, prompt)
            if not link:
                return value
            node_id, _slot = link
            if node_id in active:
                raise ValueError(
                    "Scale discovery does not support cyclic execution paths."
                )
            identity = self._node_contract(prompt[node_id]).identity_input
            if identity:
                candidate = _inputs(prompt[node_id]).get(identity)
                if _link(candidate, prompt):
                    return resolve(candidate, active | {node_id})
            return list(link)

        for node in prompt.values():
            node["inputs"] = {
                name: resolve(value) for name, value in _inputs(node).items()
            }
        for source in plan.sources:
            node = prompt[source.node_id]
            if self._node_contract(node).fixed_image_frame:
                values = _inputs(node)
                prompt[source.node_id] = {
                    "class_type": "EmptyImage",
                    "inputs": {
                        "width": values[source.width],
                        "height": values[source.height],
                        "batch_size": 1,
                        "color": 0,
                    },
                }
        target = _link(resolve([output.node_id, output.slot]), prompt)
        if target is None:
            raise ValueError("The selected output has no measurable frame.")
        retained = self._ancestors(prompt, target[0])
        sources = [source for source in plan.sources if source.node_id in retained]
        if not sources:
            raise ValueError(
                "The selected output does not depend on these dimension fields."
            )
        minimum_width, minimum_height, quantum = 128, 64, 64
        for source in sources:
            limits = self._node_contract(prompt[source.node_id]).dimension_limits or {}
            for name, horizontal in ((source.width, True), (source.height, False)):
                minimum, _maximum, step = limits.get(name, (1, 300000, 1))
                quantum = math.lcm(quantum, step)
                if horizontal:
                    minimum_width = max(minimum_width, minimum)
                else:
                    minimum_height = max(minimum_height, minimum)
        first = (
            math.ceil(minimum_width / quantum) * quantum,
            math.ceil(minimum_height / quantum) * quantum,
        )
        second = (
            math.ceil(first[0] * 1.5 / quantum) * quantum,
            math.ceil(first[1] * 1.5 / quantum) * quantum,
        )
        if first == second or second[0] * second[1] > 262144:
            raise ValueError(
                "The source cannot accept sufficiently small valid probe frames."
            )
        combined: ApiPrompt = {}
        references: list[list[object]] = []
        for index, size in enumerate((first, second)):
            prefix = f"sugarcubes-spatial-{index}:"
            replacements = {node_id: prefix + node_id for node_id in retained}
            for node_id in retained:
                node = deepcopy(prompt[node_id])
                contract = self._node_contract(node)
                if contract.output_node:
                    raise ValueError(
                        "The output path contains a side-effecting output node."
                    )
                inputs = dict(_inputs(node))
                branch_source = next(
                    (item for item in sources if item.node_id == node_id), None
                )
                spatial_inputs = [
                    name
                    for name, kind in contract.inputs.items()
                    if kind in ("IMAGE", "LATENT", "MASK")
                    and _link(inputs.get(name), prompt)
                ]
                if branch_source:
                    if spatial_inputs and not contract.fixed_image_frame:
                        raise ValueError(
                            "This source needs external image geometry that cannot be safely replaced for probing."
                        )
                    if contract.fixed_image_frame:
                        node = {
                            "class_type": "EmptyImage",
                            "inputs": {
                                "width": size[0],
                                "height": size[1],
                                "batch_size": 1,
                                "color": 0,
                            },
                        }
                        inputs = dict(_inputs(node))
                    else:
                        inputs[branch_source.width], inputs[branch_source.height] = size
                elif (
                    any(
                        kind in ("IMAGE", "LATENT", "MASK") for kind in contract.outputs
                    )
                    and not spatial_inputs
                ):
                    raise ValueError(
                        "This output combines an independently sized spatial input."
                    )
                for name in ("batch_size", "steps", "max_steps", "total_steps"):
                    if type(inputs.get(name)) in (int, float):
                        inputs[name] = 1
                for name, value in inputs.items():
                    if type(value) in (int, float) and isinstance(value, (int, float)):
                        if ("scale" in name.lower() and value > 16) or (
                            name in ("width", "height") and value > 4096
                        ):
                            raise ValueError(
                                "The output path exceeds the small-frame probe budget."
                            )
                for name, value in inputs.items():
                    link = _link(value, prompt)
                    if link:
                        if link[0] not in retained:
                            raise ValueError(
                                "The spatial probe has an unresolved dependency."
                            )
                        inputs[name] = [replacements[link[0]], link[1]]
                node["inputs"] = inputs
                combined[replacements[node_id]] = node
            references.append([replacements[target[0]], target[1]])
        kind = "image" if output.kind == "IMAGE" else "latent"
        combined[MEASUREMENT_NODE] = {
            "class_type": MEASUREMENT_CLASS,
            "inputs": {f"{kind}_a": references[0], f"{kind}_b": references[1]},
        }
        if len(combined) > MAX_PROBE_NODES * 2 + 1:
            raise ValueError("The spatial probe exceeds its execution budget.")
        return SpatialProbeExecution(combined, MEASUREMENT_NODE, (first, second))

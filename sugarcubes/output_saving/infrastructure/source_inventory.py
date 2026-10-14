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
"""Adapt execution prompts and shared graph analysis into qualified saving sources."""

from __future__ import annotations

from collections import deque
from collections.abc import Mapping, Callable

from ...execution.models import ApiPrompt, ExecutionNodeOwner
from ...execution.output_instrumentation import is_instrumented_cube_output
from ...workflow_analysis import CubeGraphAnalysisService

from ..domain.run_plan import SavingSource, SourceKind
from ..domain.source_inventory import SavingSourceInventory
from ..domain.saver_contract import ImageSaverContract


class ComfySavingSourceInventory:
    """Own host source qualification without guessing saving behavior from output-node flags."""

    def __init__(
        self,
        *,
        qualified_class: Callable[[str], ImageSaverContract | None],
        image_output_class: Callable[[str], bool],
        analysis: CubeGraphAnalysisService | None = None,
    ) -> None:
        """Bind live class qualification and the existing authoritative cube stack analysis."""
        self._qualified = qualified_class
        self._image_output = image_output_class
        self._analysis = analysis or CubeGraphAnalysisService()

    def collect(
        self,
        prompt: ApiPrompt,
        *,
        workflow: object,
        owners: Mapping[str, ExecutionNodeOwner] | None = None,
    ) -> SavingSourceInventory:
        """Read workflow-local choices while retaining exact execution and source identities."""
        cube_nodes = {
            key: node
            for key, node in prompt.items()
            if node.get("class_type") == "SugarCubes.CubeOutput"
            and (
                is_instrumented_cube_output(key)
                or (
                    owners is not None
                    and key in owners
                    and owners[key].instance_id is not None
                )
            )
        }
        positions: dict[str, int] = {}
        active: tuple[str, ...] = ()
        if cube_nodes:
            if workflow is None:
                raise ValueError(
                    "Cube saving requires its serialized workflow to resolve active stack order."
                )
            analyzed = self._analysis.analyze(workflow)
            ordered = tuple(
                identity
                for segment in analyzed.segments
                for identity in segment.instance_ids
            )
            positions = {
                identity: index for index, identity in enumerate(ordered, start=1)
            }
            instances = {
                instance.instance_id: instance for instance in analyzed.instances
            }
            active = tuple(
                identity
                for identity in ordered
                if instances[identity].execution_mode not in {2, 4}
            )
        roots = _root_nodes(workflow)
        sources: list[SavingSource] = []
        unmanaged: list[str] = []
        contracts: dict[str, ImageSaverContract] = {}
        for node_id, node in prompt.items():
            class_type = node.get("class_type")
            inputs = node.get("inputs")
            if not isinstance(class_type, str) or not isinstance(inputs, Mapping):
                raise ValueError(
                    "Execution saving source requires a node class and input object."
                )
            contract = self._qualified(class_type)
            if class_type == "SugarCubes.CubeOutput":
                owner = owners.get(node_id) if owners is not None else None
                if not is_instrumented_cube_output(node_id):
                    if owner is None or owner.instance_id is None:
                        unmanaged.append(node_id)
                        continue
                    if owner.instance_id not in active:
                        raise ValueError(
                            "Internal cube output is absent from the active workflow stack."
                        )
                    contracts[node_id] = ImageSaverContract(
                        class_type, "value", SourceKind.CUBE
                    )
                    sources.append(
                        SavingSource(
                            node_id,
                            _text(inputs.get("default_alias")),
                            SourceKind.INTERNAL,
                            cube_id=owner.instance_id,
                        )
                    )
                    continue
                contracts[node_id] = ImageSaverContract(
                    class_type, "value", SourceKind.CUBE
                )
                identity = _text(inputs.get("instance_id"))
                if identity not in active:
                    raise ValueError(
                        "Cube output saving identity is absent from the active workflow stack."
                    )
                alias = _text(inputs.get("instance_alias")) or _text(
                    inputs.get("default_alias")
                )
                sources.append(
                    SavingSource(
                        node_id,
                        alias,
                        SourceKind.CUBE,
                        cube_id=identity,
                        cube_number=positions[identity],
                        cube_alias=alias,
                        seed=_upstream_seed(prompt, node_id),
                        enabled=inputs.get("save_outputs") is not False,
                    )
                )
            elif contract is not None:
                root = roots.get(node_id, {})
                properties = root.get("properties")
                properties = properties if isinstance(properties, Mapping) else {}
                owner = owners.get(node_id) if owners is not None else None
                internal = owner is not None and owner.instance_id is not None
                if internal and contract.kind is SourceKind.PREVIEW:
                    continue
                contracts[node_id] = contract
                meta = node.get("_meta")
                meta = meta if isinstance(meta, Mapping) else {}
                label = (
                    _text(properties.get("sugarcubes_output_label"))
                    or _text(root.get("title"))
                    or _text(meta.get("title"))
                    or class_type
                )
                sources.append(
                    SavingSource(
                        node_id,
                        label,
                        (SourceKind.INTERNAL if internal else contract.kind),
                        cube_id=owner.instance_id or "" if owner is not None else "",
                        seed=_upstream_seed(prompt, node_id),
                        enabled=properties.get("sugarcubes_save_outputs") is not False,
                    )
                )
            elif self._image_output(class_type):
                unmanaged.append(node_id)
        return SavingSourceInventory(
            tuple(sources), active, contracts, tuple(unmanaged)
        )


def _root_nodes(workflow: object) -> dict[str, Mapping[str, object]]:
    """Read only root presentation labels and persisted source choices from a host workflow."""
    if not isinstance(workflow, Mapping):
        return {}
    nodes = workflow.get("nodes")
    if not isinstance(nodes, list):
        return {}
    result: dict[str, Mapping[str, object]] = {}
    for node in nodes:
        if not isinstance(node, Mapping):
            continue
        identity = node.get("id")
        if isinstance(identity, str | int) and not isinstance(identity, bool):
            result[str(identity)] = {
                key: value for key, value in node.items() if isinstance(key, str)
            }
    return result


def _upstream_seed(prompt: ApiPrompt, source_id: str) -> str:
    """Find the closest submitted generation seed without reading currently armed UI widgets."""
    pending = deque([source_id])
    visited: set[str] = set()
    while pending:
        identity = pending.popleft()
        if identity in visited:
            continue
        visited.add(identity)
        node = prompt.get(identity)
        if node is None:
            continue
        inputs = node.get("inputs")
        if not isinstance(inputs, Mapping):
            continue
        for name in ("seed", "noise_seed"):
            value = inputs.get(name)
            if isinstance(value, int) and not isinstance(value, bool) and value >= 0:
                return str(value)
            if isinstance(value, str) and value.isascii() and value.isdigit():
                return value
        for value in inputs.values():
            if (
                isinstance(value, list)
                and len(value) == 2
                and isinstance(value[0], str)
                and isinstance(value[1], int)
                and not isinstance(value[1], bool)
                and value[0] in prompt
            ):
                pending.append(value[0])
    return ""


def _text(value: object) -> str:
    """Narrow optional host labels without interpreting markup or expression text."""
    return value.strip() if isinstance(value, str) else ""

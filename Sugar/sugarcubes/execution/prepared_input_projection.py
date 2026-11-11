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
"""Project decoded image assets into the canonical prompt without modifying authored loaders or workflows."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from copy import deepcopy
from dataclasses import dataclass, replace
from hashlib import sha256

from .errors import CubeLoweringError
from .models import (
    ApiPrompt,
    BoundaryBinding,
    ExecutionOutput,
    LoweringResult,
    NodeDefinitions,
)
from .prepared_input_values import PreparedInputAsset


@dataclass(frozen=True)
class PreparedInputProjection:
    """Carry the prompt and exact socket replacements needed by execution metadata."""

    prompt: ApiPrompt
    replacements: Mapping[ExecutionOutput, ExecutionOutput]
    origins: Mapping[str, str]
    definitions: NodeDefinitions


@dataclass(frozen=True)
class PreparedInputLowering:
    """Protect prepared source identity while retaining normal ownership and boundary semantics."""

    lowered: LoweringResult
    protected_nodes: frozenset[str]


class PreparedInputProjector:
    """Own one output replacement policy for ordinary prompts and compiled Cube workflows."""

    def __init__(
        self,
        resolve_definition: Callable[[str], Mapping[str, object] | None] | None = None,
    ) -> None:
        """Inject installed schemas without importing Comfy into graph preparation policy."""
        self._resolve_definition = resolve_definition

    def lower(
        self, lowered: LoweringResult, assets: tuple[PreparedInputAsset, ...]
    ) -> PreparedInputLowering:
        """Reconcile source ownership and exposed outputs before inheritance or optimization."""
        if not assets:
            return PreparedInputLowering(lowered, frozenset())
        projected = self.project(lowered.prompt, assets, lowered.node_definitions)
        owners = dict(lowered.node_owners)
        for node, origin in projected.origins.items():
            if origin not in owners:
                raise _invalid("The prepared source has no execution owner.")
            owners[node] = owners[origin]
        bindings: list[BoundaryBinding] = []
        for binding in lowered.boundary_bindings:
            target = (
                projected.replacements.get(
                    ExecutionOutput(binding.node_id, binding.output_index)
                )
                if binding.direction == "output" and binding.output_index is not None
                else None
            )
            bindings.append(
                replace(binding, node_id=target.node_id, output_index=target.slot)
                if target
                else binding
            )
        return PreparedInputLowering(
            replace(
                lowered,
                prompt=projected.prompt,
                node_owners=owners,
                boundary_bindings=tuple(bindings),
                node_definitions=projected.definitions,
            ),
            frozenset((*projected.origins.keys(), *projected.origins.values())),
        )

    def project(
        self,
        prompt: Mapping[str, Mapping[str, object]],
        assets: tuple[PreparedInputAsset, ...],
        definitions: NodeDefinitions | None = None,
    ) -> PreparedInputProjection:
        """Replace only captured spatial sockets, preserving custom loader settings and auxiliary outputs."""
        derived: ApiPrompt = {key: deepcopy(dict(node)) for key, node in prompt.items()}
        schemas = dict(definitions or {})
        replacements: dict[ExecutionOutput, ExecutionOutput] = {}
        origins: dict[str, str] = {}
        for asset in assets:
            source = self._source(prompt, asset)
            definition = self._definition(asset.class_type, schemas)
            outputs = definition.get("output")
            if (
                not isinstance(outputs, (tuple, list))
                or asset.slot >= len(outputs)
                or outputs[asset.slot] != asset.kind.upper()
            ):
                raise _invalid(
                    "The prepared source socket does not match its installed definition."
                )
            address = ExecutionOutput(source, asset.slot)
            if address in replacements:
                raise _invalid(
                    "The compiled source has competing prepared replacements."
                )
            class_type = "LoadImage" if asset.kind == "image" else "LoadImageMask"
            self._definition(class_type, schemas)
            digest = sha256(
                f"{source}\n{asset.slot}\n{asset.prepared_file}".encode()
            ).hexdigest()[:16]
            node_id = f"{source}:sc-input:{digest}"
            inputs: dict[str, object] = {"image": asset.prepared_file}
            if asset.kind == "mask":
                inputs["channel"] = "alpha"
            node: dict[str, object] = {"class_type": class_type, "inputs": inputs}
            if node_id in derived and derived[node_id] != node:
                raise _invalid(
                    "The prepared input node identity conflicts with the execution graph."
                )
            derived[node_id] = node
            replacements[address] = ExecutionOutput(node_id, 0)
            origins[node_id] = source
        for node in derived.values():
            values = node.get("inputs")
            if not isinstance(values, dict):
                continue
            for name, value in tuple(values.items()):
                if (
                    isinstance(value, list)
                    and len(value) == 2
                    and isinstance(value[0], str)
                    and type(value[1]) is int
                ):
                    target = replacements.get(ExecutionOutput(value[0], value[1]))
                    if target is not None:
                        values[name] = target.as_prompt_link()
        return PreparedInputProjection(derived, replacements, origins, schemas)

    def _definition(
        self, class_type: str, schemas: dict[str, Mapping[str, object]]
    ) -> Mapping[str, object]:
        """Require an actual supplied or installed schema before replacing a spatial output."""
        definition = schemas.get(class_type)
        if definition is None and self._resolve_definition is not None:
            definition = self._resolve_definition(class_type)
        if definition is None:
            raise _invalid("The prepared input loader definition is unavailable.")
        schemas[class_type] = definition
        return definition

    @staticmethod
    def _source(
        prompt: Mapping[str, Mapping[str, object]], asset: PreparedInputAsset
    ) -> str:
        """Prefer exact native paths and refuse ambiguous compiler aliases rather than guessing."""
        candidates: list[str] = []
        for node_id, node in prompt.items():
            values = node.get("inputs")
            if (
                node.get("class_type") == asset.class_type
                and isinstance(values, Mapping)
                and values.get(asset.field) == asset.file
            ):
                candidates.append(node_id)
        if asset.node_path in candidates:
            return asset.node_path
        if len(candidates) != 1:
            raise _invalid(
                "The prepared input no longer identifies one captured loader."
            )
        return candidates[0]


def _invalid(message: str) -> CubeLoweringError:
    """Keep source projection errors on the normal fail-closed execution diagnostic surface."""
    return CubeLoweringError("execution.prepared_inputs.projection", message)

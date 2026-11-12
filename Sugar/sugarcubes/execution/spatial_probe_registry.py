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
"""Adapt installed Comfy node schemas into typed shape contracts."""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence

from .spatial_probe_values import SpatialNodeContract


class SpatialProbeRegistry:
    """Retain schema-only native information without importing Comfy into probe policy."""

    def __init__(self, classes: Callable[[], Mapping[str, object]]) -> None:
        """Bind the installed registry rather than importing or downloading alternate nodes."""
        self._classes = classes
        self._contracts: dict[str, SpatialNodeContract] = {}
        self._definitions: dict[str, Mapping[str, object]] = {}

    def contract(self, name: str) -> SpatialNodeContract:
        """Decode legacy and current Comfy schemas at the same public compatibility boundary."""
        if name in self._contracts:
            return self._contracts[name]
        node = self._classes().get(name)
        if node is None:
            raise ValueError(f"The workflow node {name} is not registered.")
        get_info: object = getattr(node, "GET_NODE_INFO_V1", None)
        if callable(get_info):
            info: object = get_info()
            if not isinstance(info, Mapping):
                raise ValueError(f"The workflow node {name} has an invalid schema.")
            inputs: object = info.get("input")
            outputs: object = info.get("output")
            output_node = info.get("output_node") is True
        else:
            get_inputs: object = getattr(node, "INPUT_TYPES", None)
            if not callable(get_inputs):
                raise ValueError(f"The workflow node {name} has no input schema.")
            inputs = get_inputs()
            outputs = getattr(node, "RETURN_TYPES", ())
            output_node = getattr(node, "OUTPUT_NODE", False) is True
        if (
            not isinstance(inputs, Mapping)
            or not isinstance(outputs, Sequence)
            or isinstance(outputs, (str, bytes))
        ):
            raise ValueError(f"The workflow node {name} has an unsupported schema.")
        kinds: dict[str, str] = {}
        bounds: dict[str, tuple[int, int, int]] = {}
        for group in ("required", "optional"):
            fields: object = inputs.get(group, {})
            if not isinstance(fields, Mapping):
                continue
            for key, value in fields.items():
                if (
                    not isinstance(key, str)
                    or not isinstance(value, (list, tuple))
                    or not value
                ):
                    continue
                kind: object = value[0]
                if isinstance(kind, str):
                    kinds[key] = str(kind)
                options: object = value[1] if len(value) > 1 else {}
                if kind == "INT" and isinstance(options, Mapping):
                    minimum, maximum, step = (
                        options.get("min", 1),
                        options.get("max", 300000),
                        options.get("step", 1),
                    )
                    if (
                        all(type(item) is int for item in (minimum, maximum, step))
                        and isinstance(minimum, int)
                        and isinstance(maximum, int)
                        and isinstance(step, int)
                    ):
                        bounds[key] = (max(1, minimum), maximum, max(1, step))
        if any(not isinstance(kind, str) for kind in outputs):
            raise ValueError(f"The workflow node {name} has invalid output types.")
        identity: str | None = None
        module: object = getattr(node, "__module__", None)
        if name in ("KSampler", "KSamplerAdvanced") and module == "nodes":
            identity = "latent_image"
        elif name == "SetLatentNoiseMask" and module == "nodes":
            identity = "samples"
        elif name in ("SugarCubes.CubeInput", "SugarCubes.CubeOutput"):
            identity = "value"
        elif (
            name in ("SamplerCustom", "SamplerCustomAdvanced")
            and module == "comfy_extras.nodes_custom_sampler"
        ):
            identity = "latent_image"
        contract = SpatialNodeContract(
            kinds,
            tuple(str(kind) for kind in outputs),
            output_node,
            identity,
            name == "ImageScale" and module == "nodes",
            bounds,
        )
        self._contracts[name] = contract
        self._definitions[name] = {"input": inputs}
        return contract

    def definition(self, name: str) -> Mapping[str, object]:
        """Reuse the admitted input schema for the authoritative native queue encoder."""
        self.contract(name)
        return self._definitions[name]

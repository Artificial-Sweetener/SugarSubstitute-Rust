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
"""Qualify exact native image saving contracts without assuming arbitrary custom nodes are savers."""

from __future__ import annotations

from collections.abc import Mapping
import logging
import sys

from ..domain.run_plan import SourceKind
from ..domain.saver_contract import ImageSaverContract

_logger = logging.getLogger(__name__)


class ComfySavingNodeCapabilities:
    """Own dynamic class-map access at the trusted Comfy adapter boundary."""

    def catalog(self) -> list[dict[str, object]]:
        """Identify supported and native-only image outputs explicitly for management controls."""
        module = sys.modules.get("nodes")
        mappings: object = getattr(module, "NODE_CLASS_MAPPINGS", None)
        if not isinstance(mappings, Mapping):
            return []
        statuses: list[dict[str, object]] = []
        for name in mappings:
            if not isinstance(name, str) or not self.image_output_class(name):
                continue
            contract = self.qualified_class(name)
            statuses.append(
                {
                    "class_type": name,
                    "managed": contract is not None,
                    "kind": contract.kind.value if contract is not None else None,
                    "image_input": (
                        contract.image_input if contract is not None else None
                    ),
                }
            )
        return sorted(statuses, key=lambda item: str(item["class_type"]))

    def qualified_class(self, name: str) -> ImageSaverContract | None:
        """Require exact built-ins or explicit custom-author delegation before sink replacement."""
        module = sys.modules.get("nodes")
        mappings: object = getattr(module, "NODE_CLASS_MAPPINGS", None)
        node: object = mappings.get(name) if isinstance(mappings, Mapping) else None
        if (
            node is None
            or getattr(node, "RETURN_TYPES", None) != ("IMAGE",)
            or getattr(node, "OUTPUT_NODE", False) is not True
        ):
            return None
        if name in {"SaveImage", "PreviewImage"}:
            if node is not getattr(module, name, None):
                return None
            return ImageSaverContract(
                name,
                "images",
                SourceKind.SAVE if name == "SaveImage" else SourceKind.PREVIEW,
            )
        declaration: object = getattr(node, "SUGARCUBES_IMAGE_SAVER", None)
        if not isinstance(declaration, Mapping):
            return None
        version = declaration.get("schema_version")
        image_input = declaration.get("image_input")
        kind = declaration.get("kind")
        if (
            type(version) is not int
            or version != 1
            or declaration.get("mode") != "replace_image_passthrough"
            or not isinstance(image_input, str)
            or image_input not in self._image_inputs(name, node)
            or not isinstance(kind, str)
            or kind not in {"save", "preview"}
        ):
            _logger.warning(
                "Custom image saver declaration is unsupported; native saving remains active.",
                extra={"node_class": name},
            )
            return None
        return ImageSaverContract(name, image_input, SourceKind(kind))

    def image_output_class(self, name: str) -> bool:
        """Identify unmanaged image-output surfaces while leaving all their native side effects intact."""
        module = sys.modules.get("nodes")
        mappings: object = getattr(module, "NODE_CLASS_MAPPINGS", None)
        node: object = mappings.get(name) if isinstance(mappings, Mapping) else None
        if node is None or getattr(node, "OUTPUT_NODE", False) is not True:
            return False
        return bool(self._image_inputs(name, node))

    def _image_inputs(self, name: str, node: object) -> frozenset[str]:
        """Narrow optional custom-node schemas without allowing inspection failures to change execution."""
        inputs_method: object = getattr(node, "INPUT_TYPES", None)
        if not callable(inputs_method):
            return frozenset()
        try:
            schema: object = inputs_method()
        except Exception:
            _logger.exception(
                "Could not classify native output-node image inputs.",
                extra={"node_class": name},
            )
            return frozenset()
        if not isinstance(schema, Mapping):
            return frozenset()
        images: set[str] = set()
        for group in ("required", "optional"):
            fields = schema.get(group)
            if not isinstance(fields, Mapping):
                continue
            for field, value in fields.items():
                if (
                    isinstance(field, str)
                    and isinstance(value, tuple)
                    and value
                    and value[0] == "IMAGE"
                ):
                    images.add(field)
        return frozenset(images)

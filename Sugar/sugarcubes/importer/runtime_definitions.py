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
"""Adapt live Comfy and SugarCubes node-definition registries for imports."""

from __future__ import annotations

import importlib
from collections.abc import Callable
from typing import Any, Mapping, Optional, Set

from ..nodes import NODE_CLASS_MAPPINGS as SUGAR_NODE_MAPPINGS


class RuntimeNodeDefinitions:
    """Resolve node classes through embedded, Comfy, and SugarCubes registries."""

    def __init__(
        self,
        *,
        module_loader: Callable[[str], Any] = importlib.import_module,
    ) -> None:
        """Initialize lazy Comfy registry loading through an injectable adapter."""

        self._module_loader = module_loader
        self._comfy_nodes_resolved = False
        self._comfy_nodes_module: Any = None

    def has_definition(
        self,
        class_type: str,
        definitions: Mapping[str, Any],
        *,
        subgraph_ids: Optional[Set[str]] = None,
    ) -> bool:
        """Return whether any applicable registry defines the node class."""

        if class_type in definitions:
            return True
        if subgraph_ids and class_type in subgraph_ids:
            return True
        comfy_nodes = self._load_comfy_nodes_module()
        if comfy_nodes and hasattr(comfy_nodes, "NODE_CLASS_MAPPINGS"):
            mapping = getattr(comfy_nodes, "NODE_CLASS_MAPPINGS")
            if class_type in mapping:
                return True
        return class_type in SUGAR_NODE_MAPPINGS

    def _load_comfy_nodes_module(self) -> Any:
        """Lazily load Comfy's optional node registry once."""

        if self._comfy_nodes_resolved:
            return self._comfy_nodes_module
        try:
            self._comfy_nodes_module = self._module_loader("nodes")
        except (ImportError, ModuleNotFoundError):
            self._comfy_nodes_module = None
        self._comfy_nodes_resolved = True
        return self._comfy_nodes_module


DEFAULT_NODE_DEFINITIONS = RuntimeNodeDefinitions()

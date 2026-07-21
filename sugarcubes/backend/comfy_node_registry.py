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
"""Resolve the live ComfyUI node registry for SugarCubes export validation."""

from __future__ import annotations

import importlib
import logging
import sys
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from ..nodes import NODE_CLASS_MAPPINGS as SUGARCUBES_NODE_CLASS_MAPPINGS

_LOGGER = logging.getLogger(__name__)


def resolve_active_comfy_node_class_mappings(
    extension_root: Path,
) -> Mapping[str, Any]:
    """Return the live Comfy registry merged with SugarCubes marker nodes."""

    comfy_nodes_module = _load_active_comfy_nodes_module(extension_root)
    comfy_node_class_mappings = (
        _coerce_node_class_mappings(comfy_nodes_module)
        if comfy_nodes_module is not None
        else None
    )
    if comfy_node_class_mappings is None:
        _LOGGER.warning(
            "SugarCubes: active Comfy nodes registry unavailable; export validation "
            "is falling back to SugarCubes marker nodes only"
        )
        return dict(SUGARCUBES_NODE_CLASS_MAPPINGS)

    merged_mappings = dict(comfy_node_class_mappings)
    merged_mappings.update(SUGARCUBES_NODE_CLASS_MAPPINGS)
    return merged_mappings


def _coerce_node_class_mappings(module: object) -> Mapping[str, Any] | None:
    """Return ``NODE_CLASS_MAPPINGS`` when a host module exposes a mapping."""

    mapping = getattr(module, "NODE_CLASS_MAPPINGS", None)
    return mapping if isinstance(mapping, Mapping) else None


def _load_active_comfy_nodes_module(extension_root: Path) -> object | None:
    """Load the host Comfy ``nodes`` module without accepting local shadowing."""

    existing_module = sys.modules.get("nodes")
    if existing_module is not None and not _is_extension_nodes_module(
        existing_module, extension_root
    ):
        return existing_module

    try:
        comfy_nodes_module = importlib.import_module("nodes")
    except (ImportError, ModuleNotFoundError):
        return None

    if _is_extension_nodes_module(comfy_nodes_module, extension_root):
        return None
    return comfy_nodes_module


def _is_extension_nodes_module(module: object, extension_root: Path) -> bool:
    """Return whether a module resolves to SugarCubes' local node definitions."""

    module_file = getattr(module, "__file__", None)
    if not isinstance(module_file, str):
        return False
    try:
        return (
            Path(module_file).resolve()
            == (extension_root / "sugarcubes" / "nodes.py").resolve()
        )
    except OSError:
        return False

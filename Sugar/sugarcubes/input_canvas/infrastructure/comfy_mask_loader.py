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
"""Invoke registered file-mask loaders with validated native settings at the Comfy boundary."""

from __future__ import annotations

import asyncio
import inspect
from importlib import import_module
from typing import Protocol, cast

from .input_preview_settings import InputPreviewSettings, Primitive


class _Node(Protocol):
    """Describe native schema access without exposing untyped host values internally."""

    def INPUT_TYPES(self) -> object:
        """Return the actual installed input schema."""


def load_native_mask(file: str, settings: InputPreviewSettings) -> object:
    """Run only a registered unambiguous file-mask source, preserving its own processing semantics."""
    mapping: object = getattr(import_module("nodes"), "NODE_CLASS_MAPPINGS", None)
    constructor: object = (
        mapping.get(settings.class_type) if isinstance(mapping, dict) else None
    )
    if not callable(constructor):
        raise ValueError("The mask loader is not registered.")
    candidate: object = constructor()
    if not callable(getattr(candidate, "INPUT_TYPES", None)):
        raise ValueError("The mask loader has no native input schema.")
    schema = cast(_Node, candidate).INPUT_TYPES()
    outputs: object = getattr(candidate, "RETURN_TYPES", None)
    if (
        not isinstance(schema, dict)
        or not isinstance(outputs, (tuple, list))
        or outputs.count("MASK") != 1
        or schema.get("hidden")
    ):
        raise ValueError("The node is not an unambiguous file-mask source.")
    fields: dict[str, object] = {}
    required: set[str] = set()
    for section in ("required", "optional"):
        entries: object = schema.get(section, {})
        if not isinstance(entries, dict):
            raise ValueError("Invalid native mask loader schema.")
        for name, definition in entries.items():
            if not isinstance(name, str):
                raise ValueError("Invalid native mask input name.")
            fields[name] = definition
            if section == "required":
                required.add(name)
    uploads = [name for name, definition in fields.items() if _upload(definition)]
    if uploads != [settings.field]:
        raise ValueError("The mask loader must have one unambiguous input-file source.")
    kwargs: dict[str, object] = {}
    for name, definition in fields.items():
        if name == settings.field:
            metadata = _metadata(definition)
            kwargs[name] = (
                [file]
                if metadata.get("allow_batch") is True
                or metadata.get("multiselect") is True
                or isinstance(metadata.get("multi_select"), dict)
                else file
            )
        elif name in settings.inputs:
            kwargs[name] = _setting(settings.inputs[name], definition)
        elif name in required:
            raise ValueError(
                f"The mask setting {name} is unavailable or connected to a graph input."
            )
    if getattr(candidate, "INPUT_IS_LIST", False):
        kwargs = {name: [value] for name, value in kwargs.items()}
    function: object = getattr(candidate, "FUNCTION", None)
    method: object = (
        getattr(candidate, function, None) if isinstance(function, str) else None
    )
    if not callable(method):
        raise ValueError("The native mask execution function is unavailable.")
    result: object = method(**kwargs)
    if inspect.isawaitable(result):
        result = asyncio.run(_await_result(result))
    if isinstance(result, dict):
        if result.get("expand"):
            raise ValueError(
                "Graph-expanding mask loaders cannot be previewed independently."
            )
        result = result.get("result")
    elif not isinstance(result, (tuple, list)):
        if getattr(result, "expand", None):
            raise ValueError(
                "Graph-expanding mask loaders cannot be previewed independently."
            )
        result = getattr(result, "result", None)
    index = outputs.index("MASK")
    if not isinstance(result, (tuple, list)) or index >= len(result):
        raise RuntimeError("The native mask loader returned no effective mask.")
    return result[index]


async def _await_result(value: object) -> object:
    """Await a native async loader on the preview worker's own event loop."""
    if not inspect.isawaitable(value):
        raise RuntimeError("Invalid asynchronous native mask result.")
    return await value


def _metadata(definition: object) -> dict[str, object]:
    """Narrow only host-declared widget metadata."""
    if (
        isinstance(definition, (tuple, list))
        and len(definition) > 1
        and isinstance(definition[1], dict)
    ):
        return {
            key: value for key, value in definition[1].items() if isinstance(key, str)
        }
    return {}


def _upload(definition: object) -> bool:
    """Recognize input uploads using the same public host contract as frontend discovery."""
    metadata = _metadata(definition)
    return metadata.get("image_upload") is True and metadata.get("image_folder") in (
        None,
        "input",
    )


def _setting(value: Primitive, definition: object) -> Primitive:
    """Validate primitive settings against native types, choices and numeric bounds."""
    if not isinstance(definition, (tuple, list)) or not definition:
        raise ValueError("Invalid native mask setting schema.")
    kind: object = definition[0]
    metadata = _metadata(definition)
    choices: object = (
        kind
        if isinstance(kind, (tuple, list))
        else metadata.get("options") if kind == "COMBO" else None
    )
    if isinstance(choices, (tuple, list)):
        if value not in choices:
            raise ValueError("The mask setting is not a native choice.")
    elif kind == "BOOLEAN" and type(value) is bool:
        pass
    elif kind == "STRING" and isinstance(value, str):
        pass
    elif kind in ("INT", "FLOAT") and type(value) in (int, float):
        if kind == "INT" and type(value) is not int:
            raise ValueError("The mask setting requires an integer.")
        for key, lower in (("min", True), ("max", False)):
            bound = metadata.get(key)
            if (
                isinstance(bound, (int, float))
                and isinstance(value, (int, float))
                and (value < bound if lower else value > bound)
            ):
                raise ValueError("The mask setting exceeds native bounds.")
    else:
        raise ValueError("The mask preview requires standalone primitive settings.")
    return value

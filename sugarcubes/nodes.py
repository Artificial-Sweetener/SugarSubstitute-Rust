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
"""SugarCubes marker nodes for ComfyUI workflows."""

from __future__ import annotations

from typing import Literal, NamedTuple

from .runtime import (
    CubeOutputEvent,
    notify_cube_output_observers,
)

from .runtime.cube_output_images import build_cube_output_preview


class AnyType(str):
    """Sentinel type indicating the port accepts any data."""

    def __ne__(self, other: object) -> bool:
        """Report equality-compatible behavior for ComfyUI type checks."""

        return False

    def __eq__(self, other: object) -> bool:
        """Report equality-compatible behavior for ComfyUI type checks."""

        return True


ANY = AnyType("*")


def _ensure_default_alias(value: object, node_title: str) -> str:
    """Validate that the provided default alias is non-empty after stripping."""
    if not isinstance(value, str):
        raise TypeError(f"{node_title}: default_alias must be a string")
    cleaned = value.strip()
    if not cleaned:
        raise ValueError(f"{node_title}: default_alias is required")
    return cleaned


def _ensure_cube_id(value: object, node_title: str) -> str:
    """Validate that the provided cube id is non-empty after stripping."""
    if not isinstance(value, str):
        raise TypeError(f"{node_title}: cube_id must be a string")
    cleaned = value.strip()
    if not cleaned:
        return ""
    return cleaned


def _clean_optional_string(value: object) -> str:
    """Normalize optional marker metadata string values."""

    if not isinstance(value, str):
        return ""
    return value.strip()


def _value_type_name(value: object) -> str:
    """Return a stable display name for an opaque runtime value."""

    value_type = type(value)
    return f"{value_type.__module__}.{value_type.__qualname__}"


def _media_kind_for_value(value: object) -> Literal["value", "unknown"]:
    """Classify non-previewable values for the observer contract."""

    if value is None:
        return "unknown"
    return "value"


def _current_execution_context() -> _ExecutionContext:
    """Read Comfy's current execution context when Comfy exposes it."""

    try:
        from comfy_execution.utils import get_executing_context
    except (ImportError, ModuleNotFoundError):
        return _ExecutionContext(None, None, None)
    context = get_executing_context()
    if context is None:
        return _ExecutionContext(None, None, None)
    return _ExecutionContext(context.prompt_id, context.node_id, context.list_index)


class _ExecutionContext(NamedTuple):
    """Hold the Comfy execution identifiers used by cube output events."""

    prompt_id: str | None
    node_id: str | None
    list_index: int | None


class CubeInput:
    """Load legacy marker workflows before frontend Cube migration."""

    CATEGORY = "SugarCubes/Markers"
    DEPRECATED = True
    GRAPH_PASSTHROUGH_OUTPUTS = {0: "value"}
    RETURN_TYPES: tuple[AnyType, ...] = (ANY,)
    RETURN_NAMES = ("value",)
    FUNCTION = "forward"
    OUTPUT_NODE = False

    @classmethod
    def INPUT_TYPES(cls) -> dict[str, dict[str, tuple[object, ...]]]:
        """Declare the widget schema exposed by the marker node."""

        return {
            "required": {
                "value": (ANY,),
                "cube_id": ("STRING", {"default": "", "multiline": False}),
                "default_alias": ("STRING", {"default": "", "multiline": False}),
                "instance_alias": ("STRING", {"default": "", "multiline": False}),
                "instance_id": ("STRING", {"default": "", "multiline": False}),
            },
        }

    def forward(
        self,
        value: object,
        cube_id: str,
        default_alias: str,
        instance_alias: str = "",
        instance_id: str = "",
    ) -> tuple[object]:
        """Validate marker metadata and pass the input through unchanged."""

        _ensure_cube_id(cube_id, "CubeInput")
        _ensure_default_alias(default_alias, "CubeInput")
        return (value,)


class CubeOutput:
    """Load legacy output markers before frontend Cube migration."""

    CATEGORY = "SugarCubes/Markers"
    DEPRECATED = True
    GRAPH_PASSTHROUGH_OUTPUTS = {0: "value"}
    RETURN_TYPES: tuple[AnyType, ...] = (ANY,)
    RETURN_NAMES = ("value",)
    FUNCTION = "forward"
    OUTPUT_NODE = True

    @classmethod
    def INPUT_TYPES(cls) -> dict[str, dict[str, tuple[object, ...]]]:
        """Declare the widget schema exposed by the marker node."""

        return {
            "required": {
                "value": (ANY,),
                "cube_id": ("STRING", {"default": "", "multiline": False}),
                "default_alias": ("STRING", {"default": "", "multiline": False}),
                "instance_alias": ("STRING", {"default": "", "multiline": False}),
                "instance_id": ("STRING", {"default": "", "multiline": False}),
            },
            "optional": {"save_outputs": ("BOOLEAN", {"default": False})},
        }

    def forward(
        self,
        value: object,
        cube_id: str,
        default_alias: str,
        instance_alias: str = "",
        instance_id: str = "",
        save_outputs: bool = False,
    ) -> dict[str, object]:
        """Emit output metadata and pass the input through unchanged."""

        cleaned_cube_id = _ensure_cube_id(cube_id, "CubeOutput")
        cleaned_default_alias = _ensure_default_alias(default_alias, "CubeOutput")
        cleaned_instance_alias = _clean_optional_string(instance_alias)
        cleaned_instance_id = _clean_optional_string(instance_id)
        ui_payload, artifacts = build_cube_output_preview(
            value, save_outputs=save_outputs
        )
        context = _current_execution_context()
        event = CubeOutputEvent(
            version=1,
            prompt_id=context.prompt_id,
            node_id=context.node_id,
            list_index=context.list_index,
            cube_id=cleaned_cube_id,
            default_alias=cleaned_default_alias,
            instance_alias=cleaned_instance_alias or cleaned_default_alias,
            instance_id=cleaned_instance_id,
            media_kind="image" if artifacts else _media_kind_for_value(value),
            value_type=_value_type_name(value),
            artifacts=artifacts,
        )
        notify_cube_output_observers(event)
        return {
            "result": (value,),
            "ui": ui_payload,
        }


NODE_CLASS_MAPPINGS = {
    "SugarCubes.CubeInput": CubeInput,
    "SugarCubes.CubeOutput": CubeOutput,
}

NODE_DISPLAY_NAME_MAPPINGS = {
    "SugarCubes.CubeInput": "Cube Input",
    "SugarCubes.CubeOutput": "Cube Output",
}

__all__ = [
    "ANY",
    "AnyType",
    "CubeInput",
    "CubeOutput",
    "NODE_CLASS_MAPPINGS",
    "NODE_DISPLAY_NAME_MAPPINGS",
]

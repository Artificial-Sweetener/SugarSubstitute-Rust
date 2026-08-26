#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Reconcile omitted Cube inputs with current Comfy node defaults."""

from __future__ import annotations

from collections.abc import Callable, Mapping, MutableMapping, Sequence
from copy import deepcopy
from typing import TypeGuard

from ..cube_model.picker_fields import is_widget_field_spec, resolve_picker_fallback

NodeDefinitionResolver = Callable[[str], Mapping[str, object] | None]
_MISSING = object()


class LiveNodeDefaultReconciler:
    """Own the final Cube-omission to live-node-default precedence rule."""

    def __init__(self, resolve_definition: NodeDefinitionResolver) -> None:
        """Bind the host definition boundary without importing Comfy into domain code."""

        self._resolve_definition = resolve_definition

    def apply(self, prompt: MutableMapping[str, dict[str, object]]) -> None:
        """Fill only omitted or portable-null widget inputs by declared live default."""

        for node in prompt.values():
            class_type = node.get("class_type")
            inputs = node.get("inputs")
            if not isinstance(class_type, str) or not isinstance(inputs, dict):
                continue
            definition = self._resolve_definition(class_type)
            if not isinstance(definition, Mapping):
                continue
            for input_name, field_spec in _input_fields(definition):
                if input_name in inputs and inputs[input_name] is not None:
                    continue
                default = _declared_default(field_spec)
                if default is not _MISSING:
                    inputs[input_name] = deepcopy(default)


def _input_fields(
    definition: Mapping[str, object],
) -> tuple[tuple[str, object], ...]:
    """Return widget-backed required and optional fields in host order."""

    inputs = definition.get("input")
    if not isinstance(inputs, Mapping):
        return ()
    result: list[tuple[str, object]] = []
    for section_name in ("required", "optional"):
        section = inputs.get(section_name)
        if not isinstance(section, Mapping):
            continue
        for raw_name, field_spec in section.items():
            if isinstance(raw_name, str) and is_widget_field_spec(field_spec):
                result.append((raw_name, field_spec))
    return tuple(result)


def _declared_default(field_spec: object) -> object:
    """Return the host-declared default without inventing a resource value."""

    if not _is_sequence(field_spec):
        return _MISSING
    if len(field_spec) > 1 and isinstance(field_spec[1], Mapping):
        metadata = field_spec[1]
        if "default" in metadata:
            return metadata["default"]
    picker = resolve_picker_fallback(field_spec)
    return picker.value if picker is not None else _MISSING


def _is_sequence(value: object) -> TypeGuard[Sequence[object]]:
    """Narrow serialized field specifications without accepting strings."""

    return isinstance(value, Sequence) and not isinstance(
        value, (str, bytes, bytearray)
    )

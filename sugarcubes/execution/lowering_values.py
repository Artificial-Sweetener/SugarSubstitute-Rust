#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Parse canonical boundary and node-definition values for native lowering."""

from __future__ import annotations

from collections.abc import Mapping, Sequence


def input_targets(value: object) -> tuple[tuple[str, str], ...]:
    """Read canonical input targets from one boundary binding."""

    targets = value.get("targets") if isinstance(value, Mapping) else value
    if not isinstance(targets, Sequence) or isinstance(
        targets, (str, bytes, bytearray)
    ):
        return ()
    result: list[tuple[str, str]] = []
    for target in targets:
        if (
            isinstance(target, Sequence)
            and not isinstance(target, (str, bytes, bytearray))
            and len(target) >= 2
            and isinstance(target[0], str)
            and isinstance(target[1], str)
        ):
            result.append((target[0], target[1]))
    return tuple(result)


def output_endpoint(value: object) -> tuple[str, int] | None:
    """Read canonical string, tuple, or object output endpoint forms."""

    if isinstance(value, str) and value:
        return value, 0
    if isinstance(value, Mapping):
        symbol = value.get("symbol")
        slot = value.get("slot", 0)
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        if not value:
            return None
        symbol = value[0]
        slot = value[1] if len(value) > 1 else 0
    else:
        return None
    if (
        isinstance(symbol, str)
        and isinstance(slot, int)
        and not isinstance(slot, bool)
        and slot >= 0
    ):
        return symbol, slot
    return None


def definition_input_types(definition: Mapping[str, object]) -> dict[str, str]:
    """Map declared node input names to compact Comfy types."""

    envelope = definition.get("input")
    if not isinstance(envelope, Mapping):
        return {}
    result: dict[str, str] = {}
    for section_name in ("required", "optional", "hidden"):
        section = envelope.get(section_name)
        if not isinstance(section, Mapping):
            continue
        for name, spec in section.items():
            value_type = field_type(spec)
            if isinstance(name, str) and value_type is not None:
                result[name] = value_type
    return result


def field_type(value: object) -> str | None:
    """Read one compact Comfy input field type."""

    if isinstance(value, str):
        return value.upper()
    if (
        isinstance(value, Sequence)
        and not isinstance(value, (str, bytes, bytearray))
        and value
    ):
        return value[0].upper() if isinstance(value[0], str) else None
    return None


def string_list(value: object) -> tuple[str, ...]:
    """Return only string values from one declared output sequence."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        return ()
    return tuple(item.upper() for item in value if isinstance(item, str))


def connection(value: object) -> tuple[str, int] | None:
    """Recognize one canonical Comfy connection tuple."""

    if (
        isinstance(value, Sequence)
        and not isinstance(value, (str, bytes, bytearray))
        and len(value) == 2
        and isinstance(value[0], str)
        and isinstance(value[1], int)
        and not isinstance(value[1], bool)
        and value[1] >= 0
    ):
        return value[0], value[1]
    return None


def node_mode(node: Mapping[str, object]) -> int:
    """Return one authored LiteGraph execution mode without boolean coercion."""

    mode = node.get("mode", 0)
    return mode if isinstance(mode, int) and not isinstance(mode, bool) else 0


def identifier(value: object) -> str | None:
    """Normalize one persisted workflow identifier."""

    if isinstance(value, bool) or not isinstance(value, (str, int)):
        return None
    normalized = str(value).strip()
    return normalized or None

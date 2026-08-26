#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Evaluate explicitly allowlisted pure nodes for prompt optimization."""

from __future__ import annotations

import re
from collections.abc import Callable, Mapping

from .optimization_context import prompt_link_address

ALLOWLISTED_NODE_CLASSES = frozenset(
    {
        "PrimitiveString",
        "PrimitiveStringMultiline",
        "RegexExtract",
        "StringConcatenate",
        "PCLazyLoraLoader",
        "PCLazyLoraLoaderAdvanced",
        "PCLazyTextEncode",
        "PCLazyTextEncodeAdvanced",
    }
)
_STRING_CLASSES = frozenset(
    {"PrimitiveString", "PrimitiveStringMultiline", "RegexExtract", "StringConcatenate"}
)
_SUPPORTED_PATTERNS = frozenset({"<[^>]*>", "(?:^|>)([^<]+)(?=<|$)"})


def is_allowlisted_node_class(class_type: str) -> bool:
    """Return whether a node class has an explicit purity contract."""

    return class_type in ALLOWLISTED_NODE_CLASSES


def optimization_kind(class_type: str) -> str:
    """Return the stable report category for one allowlisted class."""

    if class_type in {"PCLazyLoraLoader", "PCLazyLoraLoaderAdvanced"}:
        return "lora_schedule_branch"
    if class_type in {"PCLazyTextEncode", "PCLazyTextEncodeAdvanced"}:
        return "text_conditioning"
    if class_type in _STRING_CLASSES:
        return "string_resource"
    return "pure_node"


def evaluate_string_output(
    node: Mapping[str, object],
    resolve_link: Callable[[str, int], str | None],
) -> str | None:
    """Evaluate the supported pure string subset without importing Comfy classes."""

    class_type = node.get("class_type")
    inputs = node.get("inputs")
    if not isinstance(class_type, str) or not isinstance(inputs, Mapping):
        return None
    if class_type in {"PrimitiveString", "PrimitiveStringMultiline"}:
        value = inputs.get("value")
        return value if isinstance(value, str) else None
    if class_type == "StringConcatenate":
        first = _resolve_string(inputs.get("string_a"), resolve_link)
        second = _resolve_string(inputs.get("string_b"), resolve_link)
        delimiter = _resolve_string(inputs.get("delimiter"), resolve_link)
        if first is None or second is None or delimiter is None:
            return None
        return delimiter.join((first, second))
    if class_type == "RegexExtract":
        return _evaluate_regex(inputs, resolve_link)
    return None


def should_preserve_when_string_eval_fails(node: Mapping[str, object]) -> bool:
    """Return whether failed evaluation makes structural interning unsafe."""

    return node.get("class_type") == "RegexExtract"


def _evaluate_regex(
    inputs: Mapping[str, object],
    resolve_link: Callable[[str, int], str | None],
) -> str | None:
    """Evaluate the characterized RegexExtract input shapes."""

    source = _resolve_string(inputs.get("string"), resolve_link)
    pattern = _resolve_string(inputs.get("regex_pattern"), resolve_link)
    mode = _resolve_string(inputs.get("mode"), resolve_link)
    group_index = inputs.get("group_index")
    if (
        source is None
        or pattern not in _SUPPORTED_PATTERNS
        or mode not in {"First Match", "All Matches", "First Group", "All Groups"}
        or not isinstance(group_index, int)
    ):
        return None
    flags = 0
    flags |= re.IGNORECASE if inputs.get("case_insensitive") is True else 0
    flags |= re.MULTILINE if inputs.get("multiline") is True else 0
    flags |= re.DOTALL if inputs.get("dotall") is True else 0
    try:
        return _extract_regex(source, pattern, mode, group_index, flags)
    except re.error:
        return ""


def _extract_regex(source: str, pattern: str, mode: str, group: int, flags: int) -> str:
    """Mirror the supported RegexExtract result modes."""

    if mode == "First Match":
        match = re.search(pattern, source, flags)
        return match.group(0) if match else ""
    if mode == "All Matches":
        matches = re.findall(pattern, source, flags)
        if not matches:
            return ""
        return "\n".join(
            str(value[0] if isinstance(value, tuple) else value) for value in matches
        )
    if mode == "First Group":
        match = re.search(pattern, source, flags)
        return match.group(group) if match and len(match.groups()) >= group else ""
    return "\n".join(
        match.group(group)
        for match in re.finditer(pattern, source, flags)
        if match.groups() and len(match.groups()) >= group
    )


def _resolve_string(
    value: object, resolve_link: Callable[[str, int], str | None]
) -> str | None:
    """Resolve a literal or linked string value."""

    if isinstance(value, str):
        return value
    address = prompt_link_address(value)
    if address is not None:
        return resolve_link(*address)
    return None

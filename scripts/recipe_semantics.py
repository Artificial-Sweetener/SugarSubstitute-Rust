#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Normalize and compare recipe prompt semantics for acceptance proofs."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping, Sequence


def semantic_prompt(prompt: Mapping[str, object]) -> dict[str, object]:
    """Normalize prompt whitespace and JSON-number spelling for comparison."""

    prompt_classes = {"PrimitiveString", "PrimitiveStringMultiline"}

    def normalize_value(value: object) -> object:
        if isinstance(value, float) and value.is_integer():
            return int(value)
        if isinstance(value, Mapping):
            return {str(key): normalize_value(item) for key, item in value.items()}
        if isinstance(value, Sequence) and not isinstance(
            value, (str, bytes, bytearray)
        ):
            return [normalize_value(item) for item in value]
        return value

    result: dict[str, object] = {}
    for node_id, value in prompt.items():
        if not isinstance(value, Mapping):
            result[node_id] = value
            continue
        node = dict(value)
        inputs = node.get("inputs")
        if node.get("class_type") in prompt_classes and isinstance(inputs, Mapping):
            node["inputs"] = {
                str(name): (
                    _normalize_prompt_whitespace(item)
                    if name == "value" and isinstance(item, str)
                    else item
                )
                for name, item in inputs.items()
            }
        result[node_id] = normalize_value(node)
    return result


def difference_count(left: object, right: object) -> int:
    """Count exact leaf differences between two faithful representations."""

    if isinstance(left, Mapping) and isinstance(right, Mapping):
        keys = set(left) | set(right)
        return sum(difference_count(left.get(key), right.get(key)) for key in keys)
    if (
        isinstance(left, Sequence)
        and isinstance(right, Sequence)
        and not isinstance(left, (str, bytes, bytearray))
        and not isinstance(right, (str, bytes, bytearray))
    ):
        length = max(len(left), len(right))
        return sum(
            difference_count(
                left[index] if index < len(left) else object(),
                right[index] if index < len(right) else object(),
            )
            for index in range(length)
        )
    return 0 if left == right else 1


def prompt_output_semantics(prompt: Mapping[str, object]) -> dict[str, object]:
    """Describe output DAGs independently of optimizer representative IDs."""

    cache: dict[str, object] = {}
    active: set[str] = set()

    def node_semantics(node_id: str) -> object:
        if node_id in cache:
            return cache[node_id]
        if node_id in active:
            raise ValueError(f"Optimized prompt contains a cycle at '{node_id}'")
        node = prompt.get(node_id)
        if not isinstance(node, Mapping):
            raise ValueError(
                f"Optimized prompt link references missing node '{node_id}'"
            )
        active.add(node_id)

        def value_semantics(value: object) -> object:
            if (
                isinstance(value, Sequence)
                and not isinstance(value, (str, bytes, bytearray))
                and len(value) == 2
                and isinstance(value[0], str)
                and isinstance(value[1], int)
                and value[0] in prompt
            ):
                return {"source": node_semantics(value[0]), "slot": value[1]}
            if isinstance(value, Mapping):
                return {
                    str(key): value_semantics(item)
                    for key, item in sorted(
                        value.items(), key=lambda pair: str(pair[0])
                    )
                }
            if isinstance(value, Sequence) and not isinstance(
                value, (str, bytes, bytearray)
            ):
                return [value_semantics(item) for item in value]
            return value

        semantics = {
            "class_type": node.get("class_type"),
            "inputs": value_semantics(node.get("inputs", {})),
        }
        active.remove(node_id)
        cache[node_id] = semantics
        return semantics

    outputs: dict[str, object] = {}
    for node_id, node in sorted(prompt.items()):
        if (
            not isinstance(node, Mapping)
            or node.get("class_type") != "SugarCubes.CubeOutput"
        ):
            continue
        inputs = node.get("inputs")
        input_map = inputs if isinstance(inputs, Mapping) else {}
        metadata = node.get("_meta")
        metadata_map = metadata if isinstance(metadata, Mapping) else {}
        key = f"{input_map.get('instance_alias')}:{metadata_map.get('title')}"
        if key in outputs:
            raise ValueError(f"Optimized prompt contains duplicate output '{key}'")
        outputs[key] = node_semantics(node_id)
    if not outputs:
        raise ValueError("Optimized prompt contains no instrumented Cube outputs")
    return outputs


def leaf_count(value: object) -> int:
    """Count scalar semantic values while treating prompt links as one field."""

    if isinstance(value, Mapping):
        return sum(leaf_count(item) for item in value.values())
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        return (
            1
            if len(value) == 2 and isinstance(value[0], str)
            else sum(leaf_count(item) for item in value)
        )
    return 1


def canonical_hash(value: object) -> str:
    """Hash canonical JSON without numeric or string coercion."""

    return hashlib.sha256(
        json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=False,
        ).encode("utf-8")
    ).hexdigest()


def _normalize_prompt_whitespace(value: str) -> str:
    """Collapse prompt whitespace without changing tokens or punctuation."""

    return re.sub(r"\s+", " ", value).strip()

#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Lower ordered native subgraph inputs into executable prompt values."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from copy import deepcopy

from ..cube_model.native_subgraph_defaults import (
    NativeSubgraphDefaultError,
    native_boundary_widget_names,
    native_widget_defaults,
)
from .errors import CubeLoweringError
from .native_subgraph_values import class_type
from .native_subgraph_values import definition
from .native_subgraph_values import native_boundary_name
from .native_subgraph_values import sequence


def lower_native_node_inputs(
    node: Mapping[str, object],
    *,
    scope_name: str,
    scope_payload: Mapping[str, object],
    links: Mapping[object, Mapping[str, object]],
    definitions: Mapping[str, object],
    external_inputs: Mapping[str, object],
    resolve_link: Callable[[Mapping[str, object]], object],
    omit_unset_widgets: bool = False,
) -> dict[str, object]:
    """Resolve native links and unlinked widget values in authored order.

    Args:
        node: Serialized native node whose inputs are being lowered.
        scope_name: Stable diagnostic identity for the containing subgraph.
        scope_payload: Serialized containing subgraph definition.
        links: Links indexed within the containing subgraph.
        definitions: Exact node definitions embedded with the Cube.
        external_inputs: Values supplied through the containing boundary.
        resolve_link: Resolves an ordinary internal native link.
        omit_unset_widgets: Omit null wrapper widgets so a nested node can use
            its own exact saved default.
    """

    entries = sequence(node.get("inputs"), "subgraph node inputs")
    node_class = class_type(node, f"{scope_name}:node")
    try:
        defaults = native_widget_defaults(
            node,
            definition(definitions, node_class),
            native_boundary_widget_names(node, links),
        )
    except NativeSubgraphDefaultError as error:
        raise CubeLoweringError(
            "execution.lowering.invalid_native_widget_defaults",
            str(error),
        ) from error
    result: dict[str, object] = {}
    for value in entries:
        if not isinstance(value, Mapping):
            continue
        name = value.get("name")
        if not isinstance(name, str) or not name:
            continue
        link = links.get(value.get("link"))
        if link is not None:
            boundary_name = native_boundary_name(scope_payload, link)
            if (
                boundary_name is not None
                and boundary_name not in external_inputs
                and name in defaults
            ):
                result[name] = deepcopy(defaults[name])
            else:
                result[name] = resolve_link(link)
        elif isinstance(value.get("widget"), Mapping) and name in defaults:
            persisted_default = defaults[name]
            if omit_unset_widgets and persisted_default is None:
                continue
            result[name] = deepcopy(persisted_default)
        else:
            result[name] = None
    return result

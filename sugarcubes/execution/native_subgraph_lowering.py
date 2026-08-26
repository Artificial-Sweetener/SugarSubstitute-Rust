#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Recursively flatten native Comfy subgraph wrappers inside one Cube."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from copy import deepcopy

from ..cube_model import CubeDocument
from .errors import CubeLoweringError
from .lowering_values import connection
from .native_subgraph_input_lowering import lower_native_node_inputs
from .native_subgraph_scope_validation import (
    native_scope_identity,
    validate_native_scope,
)
from .native_subgraph_values import class_type as _class_type
from .native_subgraph_values import cycle as _cycle
from .native_subgraph_values import definition as _definition
from .native_subgraph_values import identifier as _identifier
from .native_subgraph_values import missing_output as _missing_output
from .native_subgraph_values import mode as _mode
from .native_subgraph_values import native_bypass_entry as _native_bypass_entry
from .native_subgraph_values import native_links as _native_links
from .native_subgraph_values import native_name as _native_name
from .native_subgraph_values import native_nodes as _native_nodes
from .native_subgraph_values import node_id as _node_id
from .native_subgraph_values import root_bypass_value as _root_bypass_value
from .native_subgraph_values import sequence as _sequence
from .native_subgraph_values import string_mapping as _mapping
from .native_subgraph_values import subgraph_index as _subgraph_index

Prompt = dict[str, dict[str, object]]
DefinitionIndex = dict[str, Mapping[str, object]]


class NativeSubgraphDocumentLowerer:
    """Own recursive wrapper expansion while retaining the outer Cube owner."""

    def lower(
        self,
        instance_id: str,
        document: CubeDocument,
        public_outputs: tuple[tuple[str, int], ...] = (),
    ) -> tuple[Prompt, DefinitionIndex, dict[tuple[str, int], object]]:
        """Flatten one document into ordinary API nodes and embedded definitions."""

        context = _DocumentContext(instance_id, document)
        return context.lower(public_outputs)

    def lower_definition(
        self,
        instance_id: str,
        document: CubeDocument | None,
        definition: Mapping[str, object],
        external_inputs: Mapping[str, object],
        native_subgraphs: Sequence[Mapping[str, object]] = (),
        native_node_definitions: Mapping[str, object] | None = None,
    ) -> tuple[Prompt, DefinitionIndex, tuple[object, ...]]:
        """Flatten the authoritative saved native definition for one Cube."""

        prompt: Prompt = {}
        prompt_definitions: DefinitionIndex = {}
        subgraphs = (
            _subgraph_index(document.implementation.subgraphs)
            if document is not None
            else {}
        )
        subgraphs.update(_subgraph_index(native_subgraphs))
        definitions = (
            dict(document.implementation.definitions) if document is not None else {}
        )
        definitions.update(native_node_definitions or {})
        scope = _SubgraphScope(
            prefix=instance_id,
            payload=definition,
            external_inputs=external_inputs,
            subgraphs=subgraphs,
            definitions=definitions,
            prompt=prompt,
            prompt_definitions=prompt_definitions,
            ancestry=(native_scope_identity(definition, instance_id),),
        )
        return prompt, prompt_definitions, scope.outputs()


class _DocumentContext:
    """Resolve root implementation nodes against shared recursive collectors."""

    def __init__(self, instance_id: str, document: CubeDocument) -> None:
        self.instance_id = instance_id
        self.nodes = document.implementation.nodes
        self.definitions = document.implementation.definitions
        self.subgraphs = _subgraph_index(document.implementation.subgraphs)
        self.prompt: Prompt = {}
        self.prompt_definitions: DefinitionIndex = {}
        self.wrapper_outputs: dict[tuple[str, int], object] = {}
        self.resolving: set[tuple[str, int]] = set()

    def lower(
        self, public_outputs: tuple[tuple[str, int], ...]
    ) -> tuple[Prompt, DefinitionIndex, dict[tuple[str, int], object]]:
        """Resolve every active root node exactly once."""

        for symbol in sorted(self.nodes):
            if _mode(self.nodes[symbol]) not in {2, 4}:
                self._ensure_node(symbol)
        resolved_outputs = {
            endpoint: self._resolve_output(*endpoint) for endpoint in public_outputs
        }
        return self.prompt, self.prompt_definitions, resolved_outputs

    def _ensure_node(self, symbol: str) -> None:
        """Create an ordinary root node or expand a native wrapper."""

        node = self._node(symbol)
        class_type = _class_type(node, f"$.implementation.nodes.{symbol}")
        if class_type in self.subgraphs:
            self._expand_wrapper(symbol, node, class_type)
            return
        node_id = _node_id(self.instance_id, symbol)
        if node_id in self.prompt:
            return
        inputs = _mapping(node.get("inputs", {}), f"{symbol}.inputs")
        self.prompt[node_id] = {
            "class_type": class_type,
            "inputs": {
                str(name): self._resolve_value(value) for name, value in inputs.items()
            },
        }
        self.prompt_definitions[node_id] = _definition(self.definitions, class_type)

    def _expand_wrapper(
        self,
        symbol: str,
        node: Mapping[str, object],
        class_type: str,
    ) -> None:
        """Expand one root wrapper and cache every flattened output."""

        if any(source == symbol for source, _ in self.wrapper_outputs):
            return
        outer_inputs = _mapping(node.get("inputs", {}), f"{symbol}.inputs")
        resolved_inputs = {
            str(name): self._resolve_value(value)
            for name, value in outer_inputs.items()
        }
        scope = _SubgraphScope(
            prefix=_node_id(self.instance_id, symbol),
            payload=self.subgraphs[class_type],
            external_inputs=resolved_inputs,
            subgraphs=self.subgraphs,
            definitions=self.definitions,
            prompt=self.prompt,
            prompt_definitions=self.prompt_definitions,
            ancestry=(class_type,),
        )
        for index, value in enumerate(scope.outputs()):
            self.wrapper_outputs[(symbol, index)] = value

    def _resolve_value(self, value: object) -> object:
        """Resolve one root connection through bypass and wrapper boundaries."""

        source = connection(value)
        return deepcopy(value) if source is None else self._resolve_output(*source)

    def _resolve_output(self, symbol: str, output_index: int) -> object:
        """Return one root output's final ordinary node endpoint."""

        key = (symbol, output_index)
        if key in self.wrapper_outputs:
            return deepcopy(self.wrapper_outputs[key])
        if key in self.resolving:
            raise _cycle(symbol)
        self.resolving.add(key)
        try:
            node = self._node(symbol)
            mode = _mode(node)
            if mode == 2:
                return None
            if mode == 4:
                bypass = _root_bypass_value(node, output_index, self.definitions)
                return self._resolve_value(bypass) if bypass is not None else None
            class_type = _class_type(node, f"$.implementation.nodes.{symbol}")
            if class_type in self.subgraphs:
                self._expand_wrapper(symbol, node, class_type)
                if key not in self.wrapper_outputs:
                    raise _missing_output(class_type, output_index)
                return deepcopy(self.wrapper_outputs[key])
            self._ensure_node(symbol)
            return [_node_id(self.instance_id, symbol), output_index]
        finally:
            self.resolving.discard(key)

    def _node(self, symbol: str) -> Mapping[str, object]:
        """Return one root implementation node or fail closed."""

        node = self.nodes.get(symbol)
        if node is None:
            raise CubeLoweringError(
                "execution.lowering.unknown_internal_source",
                f"Internal connection references unknown Cube node '{symbol}'.",
            )
        return node


class _SubgraphScope:
    """Resolve one nested native subgraph scope into shared prompt collectors."""

    def __init__(
        self,
        *,
        prefix: str,
        payload: Mapping[str, object],
        external_inputs: Mapping[str, object],
        subgraphs: Mapping[str, Mapping[str, object]],
        definitions: Mapping[str, object],
        prompt: Prompt,
        prompt_definitions: DefinitionIndex,
        ancestry: tuple[str, ...],
    ) -> None:
        self.prefix = prefix
        self.payload = payload
        self.external_inputs = external_inputs
        self.subgraphs = subgraphs
        self.definitions = definitions
        self.prompt = prompt
        self.prompt_definitions = prompt_definitions
        self.ancestry = ancestry
        validate_native_scope(payload, ancestry)
        self.nodes = _native_nodes(payload)
        self.links = _native_links(payload)
        self.resolved_outputs: dict[tuple[str, int], object] = {}
        self.resolving: set[tuple[str, int]] = set()

    def outputs(self) -> tuple[object, ...]:
        """Resolve each public output from the virtual native output node."""

        entries = _sequence(self.payload.get("outputs"), "subgraph outputs")
        return tuple(self._public_output(index) for index in range(len(entries)))

    def _public_output(self, index: int) -> object:
        """Resolve one public output link or fail when it is disconnected."""

        link = next(
            (
                value
                for value in self.links.values()
                if _identifier(value.get("target_id")) == "-20"
                and value.get("target_slot") == index
            ),
            None,
        )
        if link is None:
            raise _missing_output(self.prefix, index)
        return self._resolve_link(link)

    def _resolve_link(self, link: Mapping[str, object]) -> object:
        """Resolve a current native link, including its virtual input origin."""

        origin_id = _identifier(link.get("origin_id"))
        origin_slot = link.get("origin_slot")
        if (
            origin_id is None
            or not isinstance(origin_slot, int)
            or isinstance(origin_slot, bool)
        ):
            raise CubeLoweringError(
                "execution.lowering.invalid_subgraph_link",
                f"Native subgraph '{self.prefix}' contains an invalid link.",
            )
        if origin_id == "-10":
            inputs = _sequence(self.payload.get("inputs"), "subgraph inputs")
            if origin_slot >= len(inputs):
                raise CubeLoweringError(
                    "execution.lowering.missing_subgraph_input",
                    f"Native subgraph '{self.prefix}' has no input {origin_slot}.",
                )
            input_entry = inputs[origin_slot]
            if not isinstance(input_entry, Mapping):
                raise CubeLoweringError(
                    "execution.lowering.missing_subgraph_input",
                    f"Native subgraph '{self.prefix}' has invalid input {origin_slot}.",
                )
            name = input_entry.get("name")
            return (
                deepcopy(self.external_inputs.get(name))
                if isinstance(name, str)
                else None
            )
        return self._resolve_node_output(origin_id, origin_slot)

    def _resolve_node_output(self, raw_id: str, output_index: int) -> object:
        """Resolve one inner output and recurse through wrapper nodes."""

        key = (raw_id, output_index)
        if key in self.resolved_outputs:
            return deepcopy(self.resolved_outputs[key])
        if key in self.resolving:
            raise _cycle(f"{self.prefix}:{raw_id}")
        node = self.nodes.get(raw_id)
        if node is None:
            raise CubeLoweringError(
                "execution.lowering.unknown_subgraph_node",
                f"Native subgraph '{self.prefix}' references unknown node '{raw_id}'.",
            )
        self.resolving.add(key)
        try:
            mode = _mode(node)
            if mode == 2:
                return None
            if mode == 4:
                entry = _native_bypass_entry(node, output_index, self.definitions)
                return self._resolve_entry(entry) if entry is not None else None
            class_type = _class_type(node, f"{self.prefix}:{raw_id}")
            if class_type in self.subgraphs:
                if class_type in self.ancestry:
                    raise CubeLoweringError(
                        "execution.lowering.subgraph_definition_cycle",
                        f"Native subgraph definition cycle reaches '{class_type}'.",
                    )
                child = _SubgraphScope(
                    prefix=f"{self.prefix}:{_native_name(node, raw_id)}",
                    payload=self.subgraphs[class_type],
                    external_inputs=self._input_values(
                        node,
                        omit_unset_widgets=True,
                    ),
                    subgraphs=self.subgraphs,
                    definitions=self.definitions,
                    prompt=self.prompt,
                    prompt_definitions=self.prompt_definitions,
                    ancestry=(*self.ancestry, class_type),
                )
                for index, value in enumerate(child.outputs()):
                    self.resolved_outputs[(raw_id, index)] = value
            else:
                self._ensure_node(raw_id, node, class_type)
                self.resolved_outputs[key] = [
                    self._prompt_id(node, raw_id),
                    output_index,
                ]
            if key not in self.resolved_outputs:
                raise _missing_output(f"{self.prefix}:{raw_id}", output_index)
            return deepcopy(self.resolved_outputs[key])
        finally:
            self.resolving.discard(key)

    def _ensure_node(
        self, raw_id: str, node: Mapping[str, object], class_type: str
    ) -> None:
        """Create one concrete inner node exactly once."""

        node_id = self._prompt_id(node, raw_id)
        if node_id in self.prompt:
            return
        self.prompt[node_id] = {
            "class_type": class_type,
            "inputs": self._input_values(node),
        }
        self.prompt_definitions[node_id] = _definition(self.definitions, class_type)

    def _input_values(
        self,
        node: Mapping[str, object],
        *,
        omit_unset_widgets: bool = False,
    ) -> dict[str, object]:
        """Resolve links and widgets, preserving nested local-default ownership."""

        return lower_native_node_inputs(
            node,
            scope_name=self.prefix,
            scope_payload=self.payload,
            links=self.links,
            definitions=self.definitions,
            external_inputs=self.external_inputs,
            resolve_link=self._resolve_link,
            omit_unset_widgets=omit_unset_widgets,
        )

    def _resolve_entry(self, entry: Mapping[str, object]) -> object:
        """Resolve one native input entry selected by bypass policy."""

        link = self.links.get(entry.get("link"))
        return self._resolve_link(link) if link is not None else None

    def _prompt_id(self, node: Mapping[str, object], raw_id: str) -> str:
        """Build one stable owner-qualified nested execution id."""

        return f"{self.prefix}:{_native_name(node, raw_id)}"

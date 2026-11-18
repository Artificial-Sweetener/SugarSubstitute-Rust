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
"""Answer what one Cube document declares about its inputs: which a person edits, what each
is called, and whether its number is a decimal."""

from __future__ import annotations

from collections.abc import Mapping, Sequence

from ..cube_model import CubeDocument
from ..cube_model.subgraph_boundary_widgets import index_boundary_widget_targets


class CubeInputDeclarations:
    """Read one Cube document's own account of its inputs."""

    def __init__(self, document: CubeDocument) -> None:
        """Index the document's face, node definitions and wrapper inputs."""

        implementation = document.implementation
        self._definitions = implementation.definitions
        self._wrappers = {
            subgraph.get("id"): subgraph
            for subgraph in implementation.subgraphs
            if isinstance(subgraph, Mapping)
        }
        self._surface_labels = {
            (control.symbol, control.input_name): control.label.strip()
            for control in document.surface.controls
            if control.label.strip()
        }
        self.exposed = self._exposed_fields(document)
        """The `(node, input)` pairs a person edits on this Cube."""

    def label(self, node_key: str, node: Mapping[str, object], input_key: str) -> str:
        """Name one input as the person sees it.

        The Cube's face names it first, then its node definition, then the wrapper's public
        input, and last the input's own key.
        """

        surface = self._surface_labels.get((node_key, input_key))
        if surface:
            return surface
        declared = self._definition_entry(node, input_key)
        options = declared[1] if declared is not None and len(declared) > 1 else None
        if isinstance(options, Mapping):
            for name in ("label", "localized_name"):
                label = options.get(name)
                if isinstance(label, str) and label.strip():
                    return label.strip()
        wrapper_input = self._wrapper_input(node, input_key)
        label = wrapper_input.get("label") if wrapper_input is not None else None
        return label.strip() if isinstance(label, str) and label.strip() else input_key

    def typed(
        self, node: Mapping[str, object], input_key: str, value: object
    ) -> object:
        """Give a whole number held for a decimal input its decimal form.

        A page holds every number as one kind, so `1.0` comes back from it as `1`. The input's
        declaration says which it is, and a script writes the two differently.
        """

        if not isinstance(value, int) or isinstance(value, bool):
            return value
        declared = self._definition_entry(node, input_key)
        declared_type: object = declared[0] if declared else None
        if declared_type is None:
            wrapper_input = self._wrapper_input(node, input_key)
            declared_type = (
                wrapper_input.get("type") if wrapper_input is not None else None
            )
        return float(value) if declared_type == "FLOAT" else value

    def _definition_entry(
        self, node: Mapping[str, object], input_key: str
    ) -> Sequence[object] | None:
        """Return the `[type, options]` entry a node definition gives one input."""

        class_type = node.get("class_type")
        definition = (
            self._definitions.get(class_type) if isinstance(class_type, str) else None
        )
        inputs = definition.get("input") if isinstance(definition, Mapping) else None
        for group in ("required", "optional"):
            declared = inputs.get(group) if isinstance(inputs, Mapping) else None
            entry = declared.get(input_key) if isinstance(declared, Mapping) else None
            if isinstance(entry, Sequence) and not isinstance(entry, (str, bytes)):
                return entry
        return None

    def _wrapper_input(
        self, node: Mapping[str, object], input_key: str
    ) -> Mapping[str, object] | None:
        """Return the public input a wrapper node declares under this name."""

        wrapper = self._wrappers.get(node.get("class_type"))
        entries = wrapper.get("inputs") if isinstance(wrapper, Mapping) else None
        if not isinstance(entries, Sequence) or isinstance(entries, (str, bytes)):
            return None
        for entry in entries:
            if isinstance(entry, Mapping) and entry.get("name") == input_key:
                return entry
        return None

    def _exposed_fields(self, document: CubeDocument) -> frozenset[tuple[str, str]]:
        """Name the fields a person edits on this Cube.

        Those are its surface controls and the widget-backed public inputs of its wrapper
        nodes, without any field the Cube's own boundary supplies from another Cube.
        """

        implementation = document.implementation
        fields = {
            (control.symbol, control.input_name)
            for control in document.surface.controls
        }
        for key, node in implementation.nodes.items():
            wrapper = (
                self._wrappers.get(node.get("class_type"))
                if isinstance(node, Mapping)
                else None
            )
            if isinstance(wrapper, Mapping):
                fields.update(
                    (key, name) for name in index_boundary_widget_targets(wrapper)
                )
        for binding in implementation.inputs.values():
            targets = (
                binding.get("targets") if isinstance(binding, Mapping) else binding
            )
            if not isinstance(targets, Sequence) or isinstance(targets, (str, bytes)):
                continue
            for target in targets:
                if (
                    isinstance(target, Sequence)
                    and not isinstance(target, (str, bytes))
                    and len(target) == 2
                ):
                    fields.discard((str(target[0]), str(target[1])))
        return frozenset(fields)


__all__ = ["CubeInputDeclarations"]

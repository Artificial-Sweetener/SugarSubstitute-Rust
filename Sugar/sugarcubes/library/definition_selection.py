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
"""Select exact saved workflow definitions before library operations."""

from __future__ import annotations

from ..workflow import CanonicalWorkflow, EmbeddedCubeDefinition


def select_saved_definition(
    workflow: CanonicalWorkflow, definition_id: str, expected_semantic_hash: str
) -> EmbeddedCubeDefinition:
    """Require current embedded content with first-save identity already assigned."""

    definition = workflow.definition_index().get(definition_id)
    if definition is None:
        raise ValueError(f"Embedded Cube definition '{definition_id}' is unavailable")
    if definition.semantic_hash != expected_semantic_hash:
        raise ValueError("Embedded Cube definition changed after classification")
    if definition.is_draft:
        raise ValueError(
            "Save the Cube draft before using saved Cube library operations"
        )
    return definition

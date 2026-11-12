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
"""Read portable source preferences from a serialized workflow at the host boundary."""

from collections.abc import Mapping

from ..domain.source_preferences import (
    SourceSavingPreference,
    decode_source_preferences,
)


def workflow_source_preferences(
    workflow: object,
) -> Mapping[str, SourceSavingPreference]:
    """Read execution-path choices once without modifying workflow metadata or shared definitions."""
    extra = workflow.get("extra") if isinstance(workflow, Mapping) else None
    header = (
        extra.get("sugarcubes_output_saving") if isinstance(extra, Mapping) else None
    )
    if header is None:
        return decode_source_preferences(None)
    if not isinstance(header, Mapping):
        raise ValueError("Workflow output-saving metadata must be an object.")
    return decode_source_preferences(header.get("source_preferences"))

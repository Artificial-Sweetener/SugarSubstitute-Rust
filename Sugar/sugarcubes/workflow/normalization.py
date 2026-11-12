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
#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Define the canonical workflow normalization collaboration boundary."""

from __future__ import annotations

from typing import Protocol


class WorkflowNormalizer(Protocol):
    """Normalize a serialized workflow without discarding source graph data."""

    def normalize(self, workflow_value: object) -> dict[str, object]:
        """Return a detached execution- and editing-complete workflow."""


__all__ = ["WorkflowNormalizer"]

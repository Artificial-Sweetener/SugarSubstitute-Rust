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
"""Provide an isolated native queue insertion boundary without running execution."""

from __future__ import annotations


class AcceptedPromptQueue:
    """Observe native insertion and simulate rejection without loading or executing nodes."""

    def __init__(self) -> None:
        """Retain inserted items separately from observer side effects."""
        self.items: list[object] = []
        self.reject = False

    def put(self, item: object) -> None:
        """Accept unchanged queue contents unless the fixture explicitly rejects insertion."""
        if self.reject:
            raise ValueError("Queue insertion rejected.")
        self.items.append(item)

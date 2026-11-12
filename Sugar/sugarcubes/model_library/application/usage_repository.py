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
"""Declare durable dispatch evidence and atomic update-decision persistence."""

from __future__ import annotations

from collections.abc import Callable
from typing import Protocol

from ..domain.model_updates import ModelUpdateState


class ModelUsageRepository(Protocol):
    """Keep retained identity separate from disposable catalog presentation metadata."""

    def load(self) -> ModelUpdateState:
        """Return the committed consent and usage snapshot."""

    def change(
        self, transition: Callable[[ModelUpdateState], ModelUpdateState]
    ) -> ModelUpdateState:
        """Apply one application-owned transition atomically and return its committed result."""

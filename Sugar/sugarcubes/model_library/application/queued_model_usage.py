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
"""Match accepted prompt values to exact catalog artifacts without loading model weights."""

from __future__ import annotations

from dataclasses import dataclass

from .model_update_service import ModelUpdateService
from .ports import ModelCatalogRepository

_USAGE_KINDS = frozenset(
    {"checkpoints", "diffusion_models", "loras", "vae", "controlnet", "upscale_models"}
)


@dataclass(frozen=True, slots=True)
class QueuedModelUsageRecorder:
    """Mirror Substitute's distinct-hash matching of successfully dispatched scalar values."""

    catalog: ModelCatalogRepository
    updates: ModelUpdateService

    def record_values(self, values: frozenset[str]) -> int:
        """Count each exact known hash once even when several nodes reference the same model."""
        recorded: set[str] = set()
        for entry in self.catalog.list_entries():
            if (
                entry.evidence.kind not in _USAGE_KINDS
                or entry.evidence.value not in values
                or entry.sha256 is None
                or entry.sha256 in recorded
            ):
                continue
            self.updates.record_usage(entry)
            recorded.add(entry.sha256)
        return len(recorded)

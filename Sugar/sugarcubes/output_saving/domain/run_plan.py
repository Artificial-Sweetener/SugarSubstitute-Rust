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
"""Freeze per-generation saving facts and eligible output source identities."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from enum import StrEnum

from .preferences import OutputSavingPreferences


class SourceKind(StrEnum):
    """Separate public cubes, supported ordinary savers, completed previews and internal sinks."""

    CUBE = "cube"
    SAVE = "save"
    PREVIEW = "preview"
    INTERNAL = "internal"


@dataclass(frozen=True, slots=True)
class SavingSource:
    """Address one execution sink without retaining a graph object or image tensor."""

    node_id: str
    label: str
    kind: SourceKind
    cube_id: str = ""
    cube_number: int | None = None
    cube_alias: str = ""
    seed: str = ""
    enabled: bool = True
    persist: bool = False


@dataclass(frozen=True, slots=True)
class SavingRun:
    """Capture preferences and metadata once so later UI changes cannot affect queued jobs."""

    run_id: str
    root: str
    workflow: str
    started_at: datetime
    preferences: OutputSavingPreferences
    metadata: tuple[tuple[str, str], ...] = ()
    metadata_enabled: bool = True
    generation_seed: str | None = None


@dataclass(frozen=True, slots=True)
class SavingTicket:
    """Bind an opaque server-issued identity to its immutable generation and source policy."""

    ticket_id: str
    run: SavingRun
    source: SavingSource

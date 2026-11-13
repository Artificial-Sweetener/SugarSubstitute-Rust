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
"""Describe authoritative usage and update preferences outside disposable metadata caches."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime

from .provider_releases import ModelProviderRelease


@dataclass(frozen=True, slots=True)
class ModelUsageRecord:
    """Retain observed model identity, actual dispatch counts and durable update choices."""

    sha256: str
    location_id: str
    kind: str
    model_id: str
    version_id: str
    base_model: str
    usage_count: int
    last_used_at: datetime | None
    dismissed_version_id: str = ""
    updates_disabled_for_model: bool = False


@dataclass(frozen=True, slots=True)
class ModelUpdateState:
    """Enable notifications by default and persist user choices independently of catalog churn."""

    enabled: bool = True
    records: tuple[ModelUsageRecord, ...] = ()


@dataclass(frozen=True, slots=True)
class ModelUpdateProposal:
    """Offer a compatible version while keeping the installed artifact authoritative."""

    current: ModelUsageRecord
    candidate: ModelProviderRelease

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
"""Retain bounded server-reviewed discovery choices independently of provider IO."""

from __future__ import annotations

from collections import OrderedDict
from collections.abc import Callable
from dataclasses import dataclass
import math
import time
import uuid

from ..domain.model_suggestions import ModelSuggestionPlan


@dataclass(frozen=True, slots=True)
class ReviewedDiscoveryPlan:
    """Bind a transient HTTP selection token to immutable reviewed provider evidence."""

    plan_id: str
    plan: ModelSuggestionPlan
    public_only: bool
    expires_at: float


class ReviewedDiscoveryPlans:
    """Bound retained evidence without turning a picker session into a second catalog."""

    def __init__(
        self,
        *,
        capacity: int = 32,
        lifetime: float = 1800,
        clock: Callable[[], float] = time.monotonic,
    ) -> None:
        """Inject retention limits and monotonic time for deterministic expiry proofs."""
        if capacity < 1 or not math.isfinite(lifetime) or lifetime <= 0:
            raise ValueError("Discovery retention limits must be positive.")
        self._capacity = capacity
        self._lifetime = lifetime
        self._clock = clock
        self._plans: OrderedDict[str, ReviewedDiscoveryPlan] = OrderedDict()

    def retain(
        self, plan: ModelSuggestionPlan, *, public_only: bool
    ) -> ReviewedDiscoveryPlan:
        """Issue a server token only after authoritative discovery has settled."""
        self._expire()
        reviewed = ReviewedDiscoveryPlan(
            uuid.uuid4().hex, plan, public_only, self._clock() + self._lifetime
        )
        self._plans[reviewed.plan_id] = reviewed
        while len(self._plans) > self._capacity:
            self._plans.popitem(last=False)
        return reviewed

    def get(self, plan_id: str) -> ReviewedDiscoveryPlan:
        """Reject expired or foreign tokens instead of reconstructing browser-supplied offers."""
        self._expire()
        reviewed = self._plans.get(plan_id)
        if reviewed is None:
            raise ValueError("The model suggestions expired. Explore models again.")
        return reviewed

    def release(self, plan_id: str) -> None:
        """Drop a closed picker's evidence without affecting an already accepted transfer."""
        self._plans.pop(plan_id, None)

    def _expire(self) -> None:
        """Remove expired leases without extending them through passive preview requests."""
        now = self._clock()
        for plan_id, reviewed in tuple(self._plans.items()):
            if reviewed.expires_at <= now:
                del self._plans[plan_id]

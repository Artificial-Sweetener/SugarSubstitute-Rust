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
"""Plan a complete Cube cart through the authoritative graph mutation owners."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
import re

from .cube_model import CubeDocument
from .workflow_analysis import CubeGraphAnalysis, CubeGraphAnalysisService
from .workflow_mutation import CubeGraphMutationService
from .workflow_cube_placement import cube_position_order
from .workflow_mutation_errors import CubeGraphMutationError


@dataclass(frozen=True, slots=True)
class ExistingCubeStackEntry:
    """Retain one exact existing instance and its live definition."""

    instance_id: str


@dataclass(frozen=True, slots=True)
class NewCubeStackEntry:
    """Add one exact portable artifact with an isolated stable identity and frame."""

    instance_id: str
    requested_alias: str
    document: CubeDocument
    size: tuple[float, float]


CubeStackEntry = ExistingCubeStackEntry | NewCubeStackEntry


class CubeStackMutationService:
    """Validate and plan one complete cart without mutating a caller's workflow."""

    def __init__(
        self,
        analysis: CubeGraphAnalysisService,
        mutations: CubeGraphMutationService,
    ) -> None:
        """Reuse canonical topology, removal, insertion and reorder policy owners."""

        self._analysis = analysis
        self._mutations = mutations

    def apply(
        self, workflow: object, entries: Sequence[CubeStackEntry]
    ) -> CubeGraphAnalysis:
        """Return one validated final plan, preserving ordinary nodes and retained Cubes.

        Additions and removals are staged on isolated normalized documents. Exact
        Cube permutations use the shared native placement owner. Execution
        links and ordinary graph records are preserved during arrangement.
        """

        before = self._analysis.analyze(workflow)
        existing = {instance.instance_id for instance in before.instances}
        requested_ids = tuple(entry.instance_id for entry in entries)
        if any(not value.strip() or value != value.strip() for value in requested_ids):
            raise CubeGraphMutationError("Cube cart instance identifiers are invalid.")
        if len(requested_ids) != len(set(requested_ids)):
            raise CubeGraphMutationError("Cube cart contains duplicate instances.")
        additions: list[NewCubeStackEntry] = []
        retained: set[str] = set()
        for entry in entries:
            if isinstance(entry, ExistingCubeStackEntry):
                if entry.instance_id not in existing:
                    raise CubeGraphMutationError(
                        "Cube cart contains an unavailable instance."
                    )
                retained.add(entry.instance_id)
                continue
            if entry.instance_id in existing:
                raise CubeGraphMutationError("A new Cube cart instance already exists.")
            if not entry.requested_alias.strip():
                raise CubeGraphMutationError("A new Cube cart alias is required.")
            additions.append(entry)
        current = before
        for instance in before.instances:
            if instance.instance_id not in retained:
                current = self._mutations.remove_cube(
                    current.normalized_workflow, instance_id=instance.instance_id
                )
        aliases = {instance.instance_alias for instance in current.instances}
        for entry in additions:
            alias = _unique_alias(entry.requested_alias, aliases)
            aliases.add(alias)
            current = self._mutations.append_cube(
                current.normalized_workflow,
                instance_id=entry.instance_id,
                alias=alias,
                bypassed=False,
                document=entry.document.to_dict(),
                size=entry.size,
            )
        return self._reorder(current, requested_ids)

    def _reorder(
        self, initial: CubeGraphAnalysis, requested: tuple[str, ...]
    ) -> CubeGraphAnalysis:
        """Arrange the complete cart through the shared native placement owner."""

        node_ids = {
            instance.instance_id: instance.node_id for instance in initial.instances
        }
        current_order = cube_position_order(initial.normalized_workflow, node_ids)
        return self._mutations.reorder(
            initial.normalized_workflow,
            segment_instance_ids=current_order,
            ordered_instance_ids=requested,
        )


def _unique_alias(requested: str, occupied: set[str]) -> str:
    """Allocate a readable workflow-local alias for repeated cart artifacts."""

    alias = requested.strip()
    if alias not in occupied:
        return alias
    match = re.fullmatch(r"(.+?) (\d+)", alias)
    stem = match.group(1) if match else alias
    suffix = int(match.group(2)) + 1 if match else 2
    while f"{stem} {suffix}" in occupied:
        suffix += 1
    return f"{stem} {suffix}"

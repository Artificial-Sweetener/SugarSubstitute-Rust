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
"""Project configured saving policy into execution-only sinks before any image writes."""

from __future__ import annotations

from copy import deepcopy
from dataclasses import replace
from typing import Protocol

from sugarcubes.execution.models import ApiPrompt

from ..domain.run_plan import SavingRun, SavingSource, SourceKind
from ..domain.source_policy import CubeSavingPolicy


class SavingTicketWriter(Protocol):
    """Persist validated run snapshots and opaque tickets before queue insertion."""

    def issue_run(
        self, run: SavingRun, sources: tuple[SavingSource, ...]
    ) -> tuple[str, ...]:
        """Atomically issue fresh sink inputs for one immutable queued generation."""


class OutputSavingPromptPreparer:
    """Own source eligibility and sink projection independently of canvas lifecycle."""

    def __init__(self, tickets: SavingTicketWriter) -> None:
        """Receive the ticket persistence boundary explicitly."""
        self._tickets = tickets

    def prepare(
        self,
        prompt: ApiPrompt,
        *,
        run: SavingRun,
        sources: tuple[SavingSource, ...],
        active_cube_ids: tuple[str, ...],
    ) -> ApiPrompt:
        """Preserve all upstream execution inputs while replacing supported saving sinks.

        Fresh tickets invalidate output sinks without invalidating generators.
        Source inventory must already be qualified by the host adapter; unknown
        custom savers never enter this execution-rewrite boundary.
        """
        selected = tuple(source for source in sources if self._managed(run, source))
        if not selected:
            return deepcopy(prompt)
        if len({source.node_id for source in selected}) != len(selected):
            raise ValueError(
                "Saving source inventory contains duplicate execution identities."
            )
        muted = frozenset(
            source.cube_id
            for source in sources
            if source.kind is SourceKind.CUBE and not source.enabled
        )
        cube_policy = CubeSavingPolicy.create(
            mode=run.preferences.persistence_mode.value,
            active_cube_ids=active_cube_ids,
            muted_cube_ids=muted,
        )
        for source in selected:
            self._validate_sink(prompt, source)
        result = deepcopy(prompt)
        captured_sources: list[SavingSource] = []
        for source in selected:
            persist = (
                cube_policy.saves(source.cube_id)
                if source.kind is SourceKind.CUBE
                else source.kind is not SourceKind.INTERNAL and source.enabled
            )
            captured_sources.append(replace(source, persist=persist))
        tickets = self._tickets.issue_run(run, tuple(captured_sources))
        if len(tickets) != len(selected):
            raise RuntimeError(
                "Saving ticket issuer returned an incomplete source transaction."
            )
        for source, ticket in zip(selected, tickets, strict=True):
            node = result[source.node_id]
            inputs = node["inputs"]
            if not isinstance(inputs, dict):
                raise ValueError("Saving sink inputs must be an object.")
            if source.kind is SourceKind.CUBE:
                inputs["save_outputs"] = cube_policy.saves(source.cube_id)
                inputs["save_ticket"] = ticket
            else:
                node["class_type"] = "SugarCubes.ManagedImageOutput"
                node["inputs"] = {"images": inputs["images"], "save_ticket": ticket}
        return result

    @staticmethod
    def _managed(run: SavingRun, source: SavingSource) -> bool:
        """Keep built-in cube saving separate from the two ordinary-output opt-ins."""
        if source.kind in {SourceKind.CUBE, SourceKind.INTERNAL}:
            return True
        return run.preferences.manage_workflow and (
            source.kind is SourceKind.SAVE or run.preferences.save_previews
        )

    @staticmethod
    def _validate_sink(prompt: ApiPrompt, source: SavingSource) -> None:
        """Reject stale or unqualified source inventory before creating any tickets."""
        node = prompt.get(source.node_id)
        if node is None or not isinstance(node.get("inputs"), dict):
            raise ValueError("Saving source is missing from the execution prompt.")
        expected = (
            "SugarCubes.CubeOutput"
            if source.kind is SourceKind.CUBE
            else "PreviewImage" if source.kind is SourceKind.PREVIEW else "SaveImage"
        )
        if node.get("class_type") != expected:
            raise ValueError(
                "Saving source inventory does not match its qualified node class."
            )
        inputs = node.get("inputs")
        if not isinstance(inputs, dict):
            raise ValueError("Saving source inputs must be an object.")
        key = "value" if source.kind is SourceKind.CUBE else "images"
        if key not in inputs:
            raise ValueError("Saving source has no image input.")

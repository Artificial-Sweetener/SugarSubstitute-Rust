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
"""Capture server output settings and graph facts before native prompt validation."""

from __future__ import annotations

from collections.abc import Mapping, Callable
from datetime import datetime
from copy import deepcopy
import json
import logging
import sqlite3

from ...execution.models import ApiPrompt, ExecutionNodeOwner, JsonValue
from ..application.prompt_preparer import OutputSavingPromptPreparer
from ..application.settings_service import OutputSavingSettingsService
from ..domain.run_plan import SavingRun, SourceKind
from .settings_paths import HostSavingSettingsPaths
from .source_inventory import ComfySavingSourceInventory
from .ticket_journal import SavingTicketJournal

_LOGGER = logging.getLogger(__name__)


class OutputSavingQueuePolicy:
    """Own host adaptation of one immutable queued policy without depending on browser state."""

    def __init__(
        self,
        *,
        settings: OutputSavingSettingsService,
        paths: HostSavingSettingsPaths,
        inventory: ComfySavingSourceInventory,
        journal: SavingTicketJournal,
        metadata_enabled: Callable[[], bool],
        clock: Callable[[], datetime] = lambda: datetime.now().astimezone(),
    ) -> None:
        """Bind the authoritative settings, source and ticket owners at the host boundary."""
        self._settings = settings
        self._paths = paths
        self._inventory = inventory
        self._journal = journal
        self._preparer = OutputSavingPromptPreparer(journal)
        self._metadata_enabled = metadata_enabled
        self._clock = clock

    def prepare(
        self,
        prompt_id: str,
        prompt: ApiPrompt,
        *,
        workflow: Mapping[str, object],
        owners: Mapping[str, ExecutionNodeOwner],
        extra_data: Mapping[str, JsonValue],
    ) -> ApiPrompt:
        """Capture queued values while honoring legacy Substitute's local saving authority.

        Legacy host context explicitly retains client-side output persistence.
        Hosts delegating persistence declare ``outputSavingOwner=sugarcubes``.
        """
        for node in prompt.values():
            class_type = node.get("class_type")
            inputs = node.get("inputs")
            if class_type in {
                "SugarCubes.ManagedImageOutput",
                "SugarCubes.OutputSavingPolicyError",
            } or (
                class_type == "SugarCubes.CubeOutput"
                and isinstance(inputs, Mapping)
                and inputs.get("save_ticket") not in (None, "")
            ):
                raise ValueError(
                    "Execution-only saving nodes and tickets cannot be submitted as authored workflow data."
                )
        host_context = extra_data.get("substitute")
        if (
            isinstance(host_context, Mapping)
            and type(host_context.get("schemaVersion")) is int
            and host_context.get("schemaVersion") == 1
            and isinstance(host_context.get("workflowId"), str)
            and host_context.get("outputSavingOwner") != "sugarcubes"
        ):
            return deepcopy(prompt)
        try:
            preferences = self._settings.load()
            inventory = self._inventory.collect(
                prompt, workflow=workflow, owners=owners
            )
            enabled = self._metadata_enabled()
            metadata = (
                (
                    ("workflow", json.dumps(workflow, ensure_ascii=False)),
                    ("prompt", json.dumps(prompt, ensure_ascii=False)),
                )
                if enabled
                else ()
            )
            context = extra_data.get("sugarcubes_output_saving")
            if not isinstance(context, Mapping):
                recipe_extra = workflow.get("extra")
                context = (
                    recipe_extra.get("sugarcubes_output_saving")
                    if isinstance(recipe_extra, Mapping)
                    else None
                )
            name = (
                context.get("workflow_name") if isinstance(context, Mapping) else None
            )
            workflow_name = (
                name if isinstance(name, str) and name.strip() else "Workflow"
            )
            run = SavingRun(
                run_id=prompt_id,
                root=str(self._paths.root(preferences)),
                workflow=workflow_name,
                started_at=self._clock(),
                preferences=preferences,
                metadata=metadata,
                metadata_enabled=enabled,
                generation_seed=next(
                    (
                        source.seed
                        for identity in inventory.active_cube_ids
                        for source in inventory.sources
                        if source.kind is SourceKind.CUBE
                        and source.cube_id == identity
                        and source.seed
                    ),
                    "",
                ),
            )
            return self._preparer.prepare(
                prompt,
                run=run,
                sources=inventory.sources,
                active_cube_ids=inventory.active_cube_ids,
                contracts=inventory.contracts,
            )
        except (OSError, ValueError, sqlite3.Error) as error:
            _LOGGER.exception(
                "Output saving policy could not be prepared before queue insertion.",
                extra={"prompt_id": prompt_id},
            )
            self.discard(prompt_id)
            raise RuntimeError(
                f"Output saving could not be prepared: {error}"
            ) from error

    def discard(self, prompt_id: str) -> None:
        """Retire policy metadata while preserving host outcomes and every saved image file."""
        try:
            self._journal.discard_run(prompt_id)
        except (OSError, sqlite3.Error):
            _LOGGER.exception(
                "Output saving run cleanup failed; saved files and numbering remain intact.",
                extra={"prompt_id": prompt_id},
            )

    def retain_active(self, live_ids: frozenset[str]) -> None:
        """Retire prior-host preparation metadata after validating the current live queue snapshot."""
        self._journal.retain_runs(live_ids)

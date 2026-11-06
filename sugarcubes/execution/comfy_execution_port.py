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
"""Validate and queue prepared Cube prompts through the active Comfy runtime."""

from __future__ import annotations

import time
import uuid
import logging
from collections.abc import Awaitable, Callable, Sequence
from copy import deepcopy
from typing import Protocol

from .comfy_prompt_value_encoding import encode_atomic_list_widget_values
from .models import ComfyQueueReceipt, PreparedCubeExecution
from .node_resolvers import ExecutionNodeResolution, ExecutionNodeResolutionError
from .ports import QueuedPromptPolicy
from .queue_observers import (
    QueueObserverPublisher,
    RequiredQueueObserverFailure,
    ValidatedQueueEvent,
)

_LOGGER = logging.getLogger(__name__)


class PromptQueueLike(Protocol):
    """Describe Comfy's validated prompt queue insertion surface."""

    def put(self, item: object) -> None:
        """Insert one native Comfy queue tuple."""


class NodeReplacementManagerLike(Protocol):
    """Describe Comfy's node replacement normalization surface."""

    def apply_replacements(self, prompt: object) -> None:
        """Apply registered class replacements in place."""


class PromptServerLike(Protocol):
    """Describe the active Comfy server state required for direct queueing."""

    @property
    def number(self) -> float:
        """Return the next Comfy queue number."""

    @number.setter
    def number(self, value: float) -> None:
        """Advance Comfy's queue counter."""

    @property
    def prompt_queue(self) -> PromptQueueLike:
        """Return the active native prompt queue."""

    @property
    def node_replace_manager(self) -> NodeReplacementManagerLike:
        """Return Comfy's replacement manager."""


class ComfyExecutionModuleLike(Protocol):
    """Describe Comfy's prompt validation contract."""

    @property
    def SENSITIVE_EXTRA_DATA_KEYS(self) -> Sequence[str]:
        """Return extra-data keys Comfy stores outside normal metadata."""

    def validate_prompt(
        self,
        prompt_id: str,
        prompt: object,
        partial_execution_list: object,
    ) -> Awaitable[tuple[bool, object, object, object]]:
        """Validate a final prompt and return Comfy's native result tuple."""


class DirectComfyExecutionPort:
    """Mirror Comfy queue semantics without relying on fail-open prompt hooks."""

    def __init__(
        self,
        *,
        prompt_server: PromptServerLike,
        execution_module: ComfyExecutionModuleLike,
        observers: QueueObserverPublisher | None = None,
        uuid_factory: Callable[[], uuid.UUID] = uuid.uuid4,
        time_source: Callable[[], float] = time.time,
        prompt_policy: QueuedPromptPolicy | None = None,
        node_resolvers: ExecutionNodeResolution | None = None,
    ) -> None:
        """Bind the active Comfy server and validation module."""

        self._prompt_server = prompt_server
        self._execution = execution_module
        self._observers = observers
        self._uuid_factory = uuid_factory
        self._time_source = time_source
        self._prompt_policy = prompt_policy
        self._node_resolvers = node_resolvers

    async def queue(self, prepared: PreparedCubeExecution) -> ComfyQueueReceipt:
        """Resolve detached host implementations before saving policy and native validation."""

        prompt = deepcopy(prepared.prompt)
        encode_atomic_list_widget_values(prompt, prepared.node_definitions)
        self._prompt_server.node_replace_manager.apply_replacements(prompt)
        prompt_id = str(self._uuid_factory())
        queued = False
        try:
            if self._node_resolvers is not None:
                try:
                    prompt = self._node_resolvers.resolve(prompt)
                except ExecutionNodeResolutionError:
                    return ComfyQueueReceipt(
                        accepted=False,
                        prompt_id=prompt_id,
                        number=None,
                        error={
                            "type": "sugarcubes_execution_node_resolution_failed",
                            "message": (
                                "A host integration could not resolve an execution node."
                            ),
                        },
                        execution_prompt=deepcopy(prompt),
                    )
            if self._prompt_policy is not None:
                try:
                    prompt = self._prompt_policy.prepare(
                        prompt_id,
                        prompt,
                        workflow=prepared.workflow,
                        owners=prepared.node_owners,
                        extra_data=prepared.extra_data,
                    )
                except (OSError, RuntimeError, ValueError) as error:
                    _LOGGER.exception(
                        "Output policy preparation rejected direct queue insertion.",
                        extra={"prompt_id": prompt_id},
                    )
                    return ComfyQueueReceipt(
                        accepted=False,
                        prompt_id=prompt_id,
                        number=None,
                        error={
                            "type": "sugarcubes_output_saving_preparation_failed",
                            "message": str(error),
                        },
                        execution_prompt=deepcopy(prompt),
                    )
            partial_targets = list(prepared.queue.partial_execution_targets) or None
            valid = await self._execution.validate_prompt(
                prompt_id,
                prompt,
                partial_targets,
            )
            node_errors = valid[3]
            if not valid[0] or (
                prepared.queue.atomic and _has_node_errors(node_errors)
            ):
                return ComfyQueueReceipt(
                    accepted=False,
                    prompt_id=prompt_id,
                    number=None,
                    error=valid[1],
                    node_errors=node_errors,
                    execution_prompt=deepcopy(prompt),
                )
            if self._observers is not None:
                try:
                    self._observers.notify(
                        ValidatedQueueEvent(
                            prompt_id=prompt_id,
                            prompt=deepcopy(prompt),
                            extra_data=deepcopy(dict(prepared.extra_data)),
                            report=prepared.report,
                            client_id=prepared.queue.client_id,
                        )
                    )
                except RequiredQueueObserverFailure:
                    return ComfyQueueReceipt(
                        accepted=False,
                        prompt_id=prompt_id,
                        number=None,
                        error={
                            "type": "sugarcubes_queue_observer_failed",
                            "message": (
                                "A required pre-queue observer rejected the execution."
                            ),
                        },
                        node_errors=node_errors,
                        execution_prompt=deepcopy(prompt),
                    )
            number = self._resolve_number(prepared)
            extra_data = deepcopy(dict(prepared.extra_data))
            if prepared.queue.client_id is not None:
                extra_data["client_id"] = prepared.queue.client_id
            sensitive: dict[str, object] = {}
            for key in self._execution.SENSITIVE_EXTRA_DATA_KEYS:
                if key in extra_data:
                    sensitive[key] = extra_data.pop(key)
            extra_data["create_time"] = int(self._time_source() * 1000)
            self._prompt_server.prompt_queue.put(
                (number, prompt_id, prompt, extra_data, valid[2], sensitive)
            )
            queued = True
            return ComfyQueueReceipt(
                accepted=True,
                prompt_id=prompt_id,
                number=number,
                node_errors=node_errors,
                execution_prompt=deepcopy(prompt),
            )
        finally:
            if not queued and self._prompt_policy is not None:
                self._prompt_policy.discard(prompt_id)

    def _resolve_number(self, prepared: PreparedCubeExecution) -> float:
        """Preserve Comfy's explicit, front, and incrementing queue semantics."""

        explicit = prepared.queue.number
        if explicit is not None:
            return float(explicit)
        number = float(self._prompt_server.number)
        if prepared.queue.front:
            number = -number
        self._prompt_server.number = float(self._prompt_server.number) + 1
        return number


def _has_node_errors(value: object) -> bool:
    """Return whether Comfy reported any invalid output branches."""

    if isinstance(value, dict | tuple | list | set | frozenset):
        return bool(value)
    return value is not None

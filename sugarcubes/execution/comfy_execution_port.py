#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Validate and queue prepared Cube prompts through the active Comfy runtime."""

from __future__ import annotations

import time
import uuid
from collections.abc import Awaitable, Callable, Sequence
from copy import deepcopy
from typing import Protocol

from .models import ComfyQueueReceipt, PreparedCubeExecution


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
        uuid_factory: Callable[[], uuid.UUID] = uuid.uuid4,
        time_source: Callable[[], float] = time.time,
    ) -> None:
        """Bind the active Comfy server and validation module."""

        self._prompt_server = prompt_server
        self._execution = execution_module
        self._uuid_factory = uuid_factory
        self._time_source = time_source

    async def queue(self, prepared: PreparedCubeExecution) -> ComfyQueueReceipt:
        """Apply replacements, validate once, and queue atomically when accepted."""

        prompt = deepcopy(prepared.prompt)
        self._prompt_server.node_replace_manager.apply_replacements(prompt)
        prompt_id = str(self._uuid_factory())
        partial_targets = list(prepared.queue.partial_execution_targets) or None
        valid = await self._execution.validate_prompt(
            prompt_id,
            prompt,
            partial_targets,
        )
        node_errors = valid[3]
        if not valid[0] or (prepared.queue.atomic and _has_node_errors(node_errors)):
            return ComfyQueueReceipt(
                accepted=False,
                prompt_id=prompt_id,
                number=None,
                error=valid[1],
                node_errors=node_errors,
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
        return ComfyQueueReceipt(
            accepted=True,
            prompt_id=prompt_id,
            number=number,
            node_errors=node_errors,
        )

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

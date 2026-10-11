#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#    SPDX-License-Identifier: AGPL-3.0-or-later
"""Adapt explicit registered-node preparation capabilities to the Cube queue."""

from __future__ import annotations

import logging
from collections.abc import Callable, Mapping
from typing import cast

from ..execution.errors import CubeQueueError
from ..execution.models import ApiPrompt

_LOGGER = logging.getLogger(__name__)
_API_VERSION = 1


class ComfyNodePromptPreparer:
    """Invoke declared dependency owners once per queued registered node class."""

    def __init__(self, registry: Callable[[], Mapping[str, object]]) -> None:
        """Bind the active Comfy registry without importing or loading any models."""
        self._registry = registry

    def prepare(self, prompt: ApiPrompt) -> None:
        """Run explicit capabilities before host validation and reject failures."""
        registry = self._registry()
        classes = dict.fromkeys(
            str(node.get("class_type", "")) for node in prompt.values()
        )
        for name in classes:
            node_class = registry.get(name)
            version = getattr(
                node_class, "SUGARCUBES_PROMPT_PREPARATION_API_VERSION", None
            )
            if version is None:
                continue
            callback = getattr(node_class, "prepare_sugarcubes_prompt", None)
            if version != _API_VERSION or not callable(callback):
                raise CubeQueueError(
                    "execution.queue.preparation_contract",
                    f"Node '{name}' declares an unsupported Cube prompt preparation capability.",
                )
            try:
                cast(Callable[[ApiPrompt], None], callback)(prompt)
            except (ValueError, TypeError, RuntimeError, KeyError, AttributeError) as error:
                _LOGGER.exception("Cube prompt preparation failed: node_class=%s", name)
                raise CubeQueueError(
                    "execution.queue.preparation_failed",
                    f"Node '{name}' could not prepare its Cube sampling dependencies: {error}",
                ) from error

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
"""Resolve host-owned execution classes without changing canonical Cube state."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from copy import deepcopy
import logging
from threading import RLock
from typing import Protocol

from .models import ApiPrompt

_logger = logging.getLogger(__name__)
ExecutionNodeResolver = Callable[[Mapping[str, object]], str | None]


class ExecutionNodeResolutionError(RuntimeError):
    """Reject an execution whose host-owned node contract cannot be resolved."""


class ExecutionNodeResolution(Protocol):
    """Select runtime implementations for the final normalized graph."""

    def resolve(self, prompt: ApiPrompt) -> ApiPrompt:
        """Return detached nodes with unchanged identities, inputs, and topology."""


class ExecutionNodeResolverRegistry:
    """Own class-only substitution by trusted process-local integrations.

    Callbacks run synchronously on the queue caller's thread, outside the registry
    lock, over independent detached nodes. Integrations own authorization and must
    not block. A resolution snapshots registrations once; later registration or
    teardown affects the next resolution. Canonical inputs and topology cannot be
    changed through the callback's return value.
    """

    def __init__(self) -> None:
        """Initialize registration state independent of request lifetimes."""
        self._resolvers: list[ExecutionNodeResolver] = []
        self._lock = RLock()

    def register(self, resolver: ExecutionNodeResolver) -> None:
        """Register one callback identity once across host initialization."""
        with self._lock:
            if not any(existing is resolver for existing in self._resolvers):
                self._resolvers.append(resolver)

    def unregister(self, resolver: ExecutionNodeResolver) -> None:
        """Release an integration without disrupting a running request snapshot."""
        with self._lock:
            self._resolvers = [
                existing for existing in self._resolvers if existing is not resolver
            ]

    def resolve(self, prompt: ApiPrompt) -> ApiPrompt:
        """Apply unambiguous class selections atomically to a detached prompt."""
        with self._lock:
            resolvers = tuple(self._resolvers)
        result = deepcopy(prompt)
        for node_id, node in prompt.items():
            selections: set[str] = set()
            for resolver in resolvers:
                try:
                    selection = resolver(deepcopy(node))
                except Exception as error:
                    _logger.exception(
                        "Execution node resolution failed node_id=%s", node_id
                    )
                    raise ExecutionNodeResolutionError(
                        "A host integration could not authorize an execution node."
                    ) from error
                if selection is None:
                    continue
                if (
                    not isinstance(selection, str)
                    or not selection
                    or selection.strip() != selection
                ):
                    _logger.error(
                        "Execution node resolver returned an invalid class node_id=%s",
                        node_id,
                    )
                    raise ExecutionNodeResolutionError(
                        "A host integration returned an invalid execution class."
                    )
                selections.add(selection)
            if len(selections) > 1:
                _logger.error("Execution node resolvers disagree node_id=%s", node_id)
                raise ExecutionNodeResolutionError(
                    "Host integrations disagree about an execution node."
                )
            if selections:
                result[node_id]["class_type"] = selections.pop()
        return result


__all__ = [
    "ExecutionNodeResolution",
    "ExecutionNodeResolutionError",
    "ExecutionNodeResolver",
    "ExecutionNodeResolverRegistry",
]

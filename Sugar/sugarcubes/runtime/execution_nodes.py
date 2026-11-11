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
"""Publish the process-local registry for authorized execution implementations."""

from ..execution.node_resolvers import (
    ExecutionNodeResolver,
    ExecutionNodeResolverRegistry,
)

EXECUTION_NODE_RESOLVER_API_VERSION = 1
_EXECUTION_NODE_RESOLVERS = ExecutionNodeResolverRegistry()


def execution_node_resolver_registry() -> ExecutionNodeResolverRegistry:
    """Return the registry used by the active Comfy execution port."""
    return _EXECUTION_NODE_RESOLVERS


def register_execution_node_resolver(resolver: ExecutionNodeResolver) -> None:
    """Register a trusted, synchronous version-1 class-selection callback.

    Return one nonempty, unpadded runtime class or None for each detached node.
    Exceptions and disagreeing selections reject the whole queue candidate.
    Registration is identity-idempotent; retain the callback to unregister it.
    This Python extension contract does not grant browser or filesystem authority.
    """
    _EXECUTION_NODE_RESOLVERS.register(resolver)


def unregister_execution_node_resolver(resolver: ExecutionNodeResolver) -> None:
    """Release one callback identity for subsequent execution snapshots.

    An in-progress resolution retains its already captured callback snapshot.
    """
    _EXECUTION_NODE_RESOLVERS.unregister(resolver)


__all__ = [
    "EXECUTION_NODE_RESOLVER_API_VERSION",
    "execution_node_resolver_registry",
    "register_execution_node_resolver",
    "unregister_execution_node_resolver",
]

#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Adapt Git ancestry inspection to dependency version policy."""

from __future__ import annotations

import logging
from pathlib import Path

from .repository_service import RepositoryOperationError, RepositoryService

_logger = logging.getLogger(__name__)


class DependencyGitAncestryInspector:
    """Inspect and cache Git commit ancestry for installed dependencies."""

    def __init__(self, repositories: RepositoryService) -> None:
        """Initialize the inspector with SugarCubes repository access."""

        self._repositories = repositories
        self._cache: dict[tuple[str, str, str], bool] = {}

    @property
    def cached_check_count(self) -> int:
        """Return the number of cached ancestry checks."""

        return len(self._cache)

    def contains(self, source_path: str, ancestor: str, descendant: str) -> bool:
        """Return whether `descendant` contains `ancestor` in a checkout."""

        if ancestor == descendant:
            return True
        repo_path = Path(source_path)
        key = (str(repo_path.resolve()), ancestor, descendant)
        cached = self._cache.get(key)
        if cached is not None:
            return cached
        try:
            contains = self._repositories.is_ancestor(
                repo_path,
                ancestor,
                descendant,
            )
        except (OSError, RepositoryOperationError, ValueError) as exc:
            _logger.debug(
                "SugarCubes: Git ancestry check failed",
                extra={
                    "repo_path": str(repo_path),
                    "ancestor": ancestor,
                    "descendant": descendant,
                    "error": str(exc),
                },
            )
            return False
        self._cache[key] = contains
        return contains

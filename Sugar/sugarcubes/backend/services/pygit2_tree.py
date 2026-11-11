#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Traverse libgit2 trees for repository history and artifact reads."""

from __future__ import annotations

from collections.abc import Iterator

import pygit2


def tree_entry_id(tree: pygit2.Tree, relative_path: str) -> pygit2.Oid | None:
    """Return a path object identifier when it exists in one tree."""

    try:
        return tree[relative_path].id
    except KeyError:
        return None


def walk_tree_paths(
    repository: pygit2.Repository,
    tree: pygit2.Tree,
    prefix: str = "",
) -> Iterator[str]:
    """Yield repository-relative blob paths recursively."""

    for entry in tree:
        if entry.name is None:
            continue
        relative_path = f"{prefix}/{entry.name}" if prefix else entry.name
        value = repository[entry.id]
        if isinstance(value, pygit2.Tree):
            yield from walk_tree_paths(repository, value, relative_path)
        elif isinstance(value, pygit2.Blob):
            yield relative_path


__all__ = ["tree_entry_id", "walk_tree_paths"]

#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Execute one libgit2 network operation for the bounded parent adapter."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
import tempfile
from typing import Literal

import pygit2


def main() -> int:
    """Execute the requested network operation and emit one JSON response."""

    parser = _build_parser()
    arguments = parser.parse_args()
    try:
        payload = _execute(arguments)
    except (KeyError, OSError, ValueError, pygit2.GitError) as exc:
        message = str(exc)
        kind = "reference_not_found" if _is_missing_reference(message) else "failure"
        sys.stdout.write(json.dumps({"kind": kind, "message": message}))
        return 1
    sys.stdout.write(json.dumps(payload))
    return 0


def _build_parser() -> argparse.ArgumentParser:
    """Build the closed command surface accepted from the parent adapter."""

    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="operation", required=True)
    clone = subparsers.add_parser("clone")
    clone.add_argument("repository_url")
    clone.add_argument("target_path", type=Path)
    clone.add_argument("--branch")
    clone.add_argument("--depth", type=int, default=0)
    fetch = subparsers.add_parser("fetch")
    fetch.add_argument("repository_path", type=Path)
    fetch.add_argument("--remote", default="origin")
    fetch.add_argument("--branch")
    fetch.add_argument("--tags", action="store_true")
    remote_head = subparsers.add_parser("remote-head")
    remote_head.add_argument("repository_url")
    remote_head.add_argument("branch")
    return parser


def _execute(arguments: argparse.Namespace) -> dict[str, str]:
    """Dispatch one parsed operation through pygit2."""

    if arguments.operation == "clone":
        repository = pygit2.clone_repository(
            arguments.repository_url,
            arguments.target_path,
            checkout_branch=arguments.branch,
            depth=arguments.depth,
            proxy=_proxy_for(arguments.repository_url),
        )
        repository.free()
        return {}
    if arguments.operation == "fetch":
        repository = pygit2.Repository(
            pygit2.discover_repository(arguments.repository_path)
        )
        try:
            remote = repository.remotes[arguments.remote]
            refspecs: list[str] = []
            if arguments.branch:
                refspecs.append(
                    f"+refs/heads/{arguments.branch}:"
                    f"refs/remotes/{arguments.remote}/{arguments.branch}"
                )
            if arguments.tags:
                refspecs.append("+refs/tags/*:refs/tags/*")
            remote.fetch(refspecs or None, proxy=_proxy_for(remote.url or ""))
        finally:
            repository.free()
        return {}
    with tempfile.TemporaryDirectory(prefix="sugarcubes-remote-") as directory:
        repository = pygit2.init_repository(directory, bare=True)
        try:
            remote = repository.remotes.create_anonymous(arguments.repository_url)
            heads = remote.list_heads(proxy=_proxy_for(arguments.repository_url))
            expected = f"refs/heads/{arguments.branch}"
            commit_id = next(
                (str(head.oid) for head in heads if head.name == expected),
                "",
            )
            return {"commitId": commit_id}
        finally:
            repository.free()


def _is_missing_reference(message: str) -> bool:
    """Classify libgit2's branch-missing and empty-remote failures."""

    normalized = message.casefold()
    return (
        "remote branch" in normalized
        and "not found" in normalized
        or "refs/remotes/" in normalized
        and "not found" in normalized
        or "does not appear to have any commits yet" in normalized
    )


def _proxy_for(repository_url: str) -> Literal[True] | None:
    """Enable libgit2's configured proxy discovery for network repositories."""

    return True if repository_url.startswith(("http://", "https://")) else None


if __name__ == "__main__":
    raise SystemExit(main())

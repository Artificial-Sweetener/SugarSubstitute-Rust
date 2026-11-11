#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Reject system-Git dependencies from SugarCubes runtime code."""

from __future__ import annotations

import ast
from pathlib import Path

from .model import Diagnostic, Severity

_GIT_EXECUTABLES = frozenset({"git", "git.bat", "git.cmd", "git.exe"})


def scan_system_git_dependencies(root: Path) -> tuple[Diagnostic, ...]:
    """Return blocking diagnostics from authored SugarCubes Python sources."""

    source_root = root / "sugarcubes"
    if not source_root.is_dir():
        return ()
    diagnostics: list[Diagnostic] = []
    for path in sorted(source_root.rglob("*.py")):
        relative_path = path.relative_to(root).as_posix()
        try:
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=relative_path)
        except (OSError, UnicodeError, SyntaxError):
            continue
        violation = _first_violation(tree)
        if violation is not None:
            diagnostics.append(
                Diagnostic(
                    path=relative_path,
                    line=getattr(violation, "lineno", 1),
                    rule="GIT001",
                    severity=Severity.ERROR,
                    message=(
                        "System Git is forbidden in SugarCubes runtime code; use "
                        "the pygit2 repository owner instead."
                    ),
                )
            )
    return tuple(diagnostics)


def _first_violation(tree: ast.AST) -> ast.AST | None:
    """Return the first system-Git command, discovery, or configuration node."""

    for node in ast.walk(tree):
        if _is_system_git_command(node) or _is_system_git_discovery(node):
            return node
        if (
            isinstance(node, ast.Constant)
            and isinstance(node.value, str)
            and node.value == "GIT_PYTHON_GIT_EXECUTABLE"
        ):
            return node
        if isinstance(node, (ast.Import, ast.ImportFrom)) and _imports_gitpython(node):
            return node
    return None


def _is_system_git_command(node: ast.AST) -> bool:
    """Return whether a literal process argument invokes Git or `where git`."""

    if isinstance(node, (ast.List, ast.Tuple)) and node.elts:
        values = tuple(_string_constant(element) for element in node.elts[:2])
        if _is_git_executable(values[0]):
            return True
        return (
            values[0] is not None
            and _executable_name(values[0]) in {"where", "where.exe"}
            and len(values) > 1
            and _is_git_executable(values[1])
        )
    if not isinstance(node, ast.Call) or not node.args:
        return False
    function_name = (
        node.func.attr
        if isinstance(node.func, ast.Attribute)
        else node.func.id if isinstance(node.func, ast.Name) else ""
    )
    command = _string_constant(node.args[0])
    return (
        function_name in {"Popen", "call", "check_call", "check_output", "run"}
        and command is not None
        and _is_git_executable(command.split(maxsplit=1)[0])
    )


def _is_system_git_discovery(node: ast.AST) -> bool:
    """Return whether authored code explicitly searches for a Git executable."""

    if not isinstance(node, ast.Call) or not node.args:
        return False
    function = node.func
    is_which = (
        isinstance(function, ast.Attribute)
        and function.attr == "which"
        or isinstance(function, ast.Name)
        and function.id == "which"
    )
    return is_which and _is_git_executable(_string_constant(node.args[0]))


def _imports_gitpython(node: ast.Import | ast.ImportFrom) -> bool:
    """Return whether one import statement loads GitPython's `git` package."""

    if isinstance(node, ast.ImportFrom):
        return node.module == "git" or bool(
            node.module and node.module.startswith("git.")
        )
    return any(
        alias.name == "git" or alias.name.startswith("git.") for alias in node.names
    )


def _string_constant(node: ast.AST) -> str | None:
    """Return a literal string without evaluating authored code."""

    return (
        node.value
        if isinstance(node, ast.Constant) and isinstance(node.value, str)
        else None
    )


def _is_git_executable(value: str | None) -> bool:
    """Recognize portable and Windows system-Git executable names."""

    return value is not None and _executable_name(value) in _GIT_EXECUTABLES


def _executable_name(value: str) -> str:
    """Extract an executable name independent of the checker host platform."""

    return value.replace("\\", "/").rsplit("/", maxsplit=1)[-1].casefold()


__all__ = ["scan_system_git_dependencies"]

#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.

"""Keep Cube execution independent from workflow-package prompt conventions."""

from __future__ import annotations

from pathlib import Path

_ROOT = Path(__file__).resolve().parents[3]
_EXECUTION_BOUNDARY = (
    _ROOT / "sugarcubes" / "execution",
    _ROOT / "sugarcubes" / "workflow",
    _ROOT / "sugarcubes" / "workflow_analysis.py",
    _ROOT / "sugarcubes" / "workflow_mutation.py",
    _ROOT / "sugarcubes" / "backend" / "execution_routes.py",
    _ROOT / "sugarcubes" / "backend" / "workflow_analysis_routes.py",
    _ROOT / "sugarcubes" / "backend" / "workflow_mutation_routes.py",
)
_FORBIDDEN_WORKFLOW_SEMANTICS = (
    "simplesyrup",
    "[sep]",
    "regional prompt",
    "regional_prompt",
    "regional mask",
    "regional_mask",
)


def test_execution_boundary_has_no_workflow_package_prompt_policy() -> None:
    """Reject package-specific prompt or mask policy in Cube graph execution."""

    violations: list[str] = []
    for source_path in _python_sources(_EXECUTION_BOUNDARY):
        source = source_path.read_text(encoding="utf-8").casefold()
        for forbidden in _FORBIDDEN_WORKFLOW_SEMANTICS:
            if forbidden in source:
                violations.append(f"{source_path.relative_to(_ROOT)}: {forbidden}")

    assert violations == []


def _python_sources(paths: tuple[Path, ...]) -> tuple[Path, ...]:
    """Return every Python source governed by the execution boundary."""

    sources: set[Path] = set()
    for path in paths:
        if path.is_dir():
            sources.update(path.rglob("*.py"))
        elif path.is_file():
            sources.add(path)
    return tuple(sorted(sources))

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
"""Scan authored Python and TypeScript structure and dependency directions."""

from __future__ import annotations

import ast
import hashlib
import posixpath
import re
import subprocess
from pathlib import Path, PurePosixPath

from .model import ArchitecturePolicy, Diagnostic, Severity

_TYPESCRIPT_MODULE_PATTERN = re.compile(
    r"(?:\b(?:import|export)\b[\s\S]*?\bfrom\s*|\bimport\s*)"
    r"[\"'](?P<module>[^\"']+)[\"']",
    re.MULTILINE,
)
_COMMENT_PREFIXES = ("#", "//", "/*", "*", "*/")


def source_paths(root: Path, policy: ArchitecturePolicy) -> tuple[str, ...]:
    """Return deterministic authored source paths covered by the policy."""

    found: set[str] = set()
    visible = visible_repository_paths(root)
    for relative_root in policy.structure.source_roots:
        candidate = root / relative_root
        candidates: tuple[Path, ...]
        if candidate.is_file():
            candidates = (candidate,)
        elif candidate.is_dir():
            candidates = tuple(candidate.rglob("*"))
        else:
            continue
        for path in candidates:
            if not path.is_file() or path.suffix not in policy.structure.extensions:
                continue
            relative = path.relative_to(root).as_posix()
            if _excluded(relative, policy.structure.excluded_prefixes):
                continue
            if visible is not None and relative not in visible:
                continue
            found.add(relative)
    return tuple(sorted(found))


def visible_repository_paths(root: Path) -> frozenset[str] | None:
    """Return tracked and unignored paths, or no filter outside a Git worktree."""

    if not (root / ".git").exists():
        return None
    completed = subprocess.run(  # noqa: S603
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=root,
        check=False,
        capture_output=True,
    )
    if completed.returncode != 0:
        message = completed.stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(f"cannot inventory Git-visible sources: {message}")
    return frozenset(
        value.decode("utf-8", errors="strict").replace("\\", "/")
        for value in completed.stdout.split(b"\0")
        if value
    )


def _excluded(path: str, prefixes: tuple[str, ...]) -> bool:
    """Return whether one path belongs to an explicitly excluded source tree."""

    return any(
        path == prefix.rstrip("/") or path.startswith(prefix) for prefix in prefixes
    )


def production_lines(source: str) -> int:
    """Count non-empty, non-comment-only authored source lines."""

    return sum(
        1
        for line in source.splitlines()
        if (trimmed := line.strip()) and not trimmed.startswith(_COMMENT_PREFIXES)
    )


def source_fingerprint(root: Path, paths: tuple[str, ...]) -> str:
    """Hash exact normalized paths and contents for current-state debt tracking."""

    digest = hashlib.sha256()
    for path in sorted(paths):
        digest.update(path.encode("utf-8"))
        digest.update(b"\0")
        digest.update((root / path).read_bytes().replace(b"\r\n", b"\n"))
        digest.update(b"\0")
    return digest.hexdigest()


def scan_structure(
    root: Path,
    policy: ArchitecturePolicy,
    paths: tuple[str, ...],
) -> tuple[tuple[Diagnostic, ...], dict[str, int]]:
    """Return size diagnostics and metrics for all governed authored sources."""

    diagnostics: list[Diagnostic] = []
    metrics: dict[str, int] = {}
    for path in paths:
        lines = production_lines((root / path).read_text(encoding="utf-8-sig"))
        metrics[path] = lines
        if lines > policy.structure.hard_lines:
            diagnostics.append(
                Diagnostic(
                    path=path,
                    rule="STRUCT003",
                    severity=Severity.ERROR,
                    message=(
                        f"{lines} production lines exceed the hard ceiling "
                        f"{policy.structure.hard_lines}; classify the file as a "
                        "cohesive structural exception or fingerprinted "
                        "mixed-responsibility remediation before extending it"
                    ),
                )
            )
        elif lines > policy.structure.soft_lines:
            diagnostics.append(
                Diagnostic(
                    path=path,
                    rule="STRUCT002",
                    severity=Severity.WARNING,
                    message=(
                        f"{lines} production lines exceed the soft ceiling "
                        f"{policy.structure.soft_lines}; reassess cohesion before "
                        "extending this file"
                    ),
                )
            )
    return tuple(diagnostics), metrics


def scan_dependencies(
    root: Path,
    policy: ArchitecturePolicy,
    paths: tuple[str, ...],
) -> tuple[Diagnostic, ...]:
    """Return forbidden Python and TypeScript dependency-direction diagnostics."""

    diagnostics: list[Diagnostic] = []
    for path in paths:
        matching_rules = tuple(rule for rule in policy.dependencies if rule.owns(path))
        if not matching_rules:
            continue
        source = (root / path).read_text(encoding="utf-8-sig")
        if PurePosixPath(path).suffix in {".py", ".pyi"}:
            try:
                imports = _python_imports(path, source)
            except SyntaxError as error:
                diagnostics.append(
                    Diagnostic(
                        path=path,
                        line=error.lineno or 1,
                        rule="DEPENDENCY_PARSE",
                        severity=Severity.ERROR,
                        message=f"cannot parse Python imports: {error.msg}",
                    )
                )
                continue
            for rule in matching_rules:
                forbidden = tuple(
                    imported
                    for imported in imports
                    if imported.startswith(rule.forbidden_python_imports)
                )
                if forbidden:
                    diagnostics.append(
                        Diagnostic(
                            path=path,
                            rule=f"DEPENDENCY_{rule.name}",
                            severity=Severity.ERROR,
                            message=f"forbidden imports: {', '.join(sorted(forbidden))}",
                        )
                    )
            continue
        imports = _typescript_imports(path, source)
        for rule in matching_rules:
            forbidden_paths = tuple(
                imported
                for imported in imports
                if imported.startswith(rule.forbidden_typescript_paths)
            )
            if forbidden_paths:
                diagnostics.append(
                    Diagnostic(
                        path=path,
                        rule=f"DEPENDENCY_{rule.name}",
                        severity=Severity.ERROR,
                        message=(
                            "forbidden imports: "
                            f"{', '.join(sorted(forbidden_paths))}"
                        ),
                    )
                )
    return tuple(diagnostics)


def _python_imports(path: str, source: str) -> frozenset[str]:
    """Return normalized imports from one Python source module."""

    tree = ast.parse(source, filename=path)
    module_parts = list(PurePosixPath(path).with_suffix("").parts)
    if module_parts[-1] == "__init__":
        module_parts.pop()
    else:
        module_parts.pop()
    imports: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            imports.update(alias.name for alias in node.names)
        elif isinstance(node, ast.ImportFrom):
            if node.level == 0:
                if node.module:
                    imports.add(node.module)
                continue
            keep = max(0, len(module_parts) - node.level + 1)
            resolved = module_parts[:keep]
            if node.module:
                resolved.extend(node.module.split("."))
            if resolved:
                imports.add(".".join(resolved))
    return frozenset(imports)


def _typescript_imports(path: str, source: str) -> frozenset[str]:
    """Return repository-relative targets imported by one TypeScript module."""

    parent = PurePosixPath(path).parent.as_posix()
    imports: set[str] = set()
    for match in _TYPESCRIPT_MODULE_PATTERN.finditer(source):
        module = match.group("module")
        if not module.startswith("."):
            continue
        resolved = posixpath.normpath(posixpath.join(parent, module))
        for suffix in (".js", ".ts", ".tsx"):
            if resolved.endswith(suffix):
                resolved = resolved[: -len(suffix)]
                break
        imports.add(resolved)
    return frozenset(imports)


__all__ = [
    "production_lines",
    "scan_dependencies",
    "scan_structure",
    "source_fingerprint",
    "source_paths",
    "visible_repository_paths",
]

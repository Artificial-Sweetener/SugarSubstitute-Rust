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
"""Static standards audit for SugarCubes Python runtime modules."""

from __future__ import annotations

import ast
import sys
from pathlib import Path
from typing import Iterable

DOCSTRING_TARGETS = (
    "__init__.py",
    "sugarcubes/backend/routes.py",
    "sugarcubes/backend/services/cube_export_service.py",
    "sugarcubes/backend/services/cube_library_service.py",
    "sugarcubes/backend/services/cube_load_service.py",
    "sugarcubes/backend/services/cube_metadata_service.py",
    "sugarcubes/backend/validation/request_parsers.py",
    "sugarcubes/exporter/graph.py",
    "sugarcubes/exporter/io.py",
    "sugarcubes/exporter/serializer.py",
    "sugarcubes/exporter/validation.py",
    "sugarcubes/exporter/versioning.py",
    "sugarcubes/importer/loader.py",
    "sugarcubes/nodes.py",
)

RUNTIME_ROOTS = ("sugarcubes",)
ROOT_RUNTIME_FILES = ("__init__.py",)
OPTIONAL_FALLBACK_CONTEXTS = {
    (
        "sugarcubes/exporter/definition_snapshot.py",
        ("collect_definitions",),
    ),
    (
        "sugarcubes/runtime/cube_output_events.py",
        ("CubeOutputEventBus", "notify"),
    ),
}


def _repo_root() -> Path:
    """Return the repository root for audit commands and tests."""

    return Path(__file__).resolve().parents[1]


def _iter_runtime_python_files(root: Path) -> Iterable[Path]:
    """Yield runtime Python files covered by the standards audit."""

    for relative in ROOT_RUNTIME_FILES:
        path = root / relative
        if path.exists():
            yield path
    for relative in RUNTIME_ROOTS:
        for path in sorted((root / relative).rglob("*.py")):
            yield path


def _read_tree(path: Path) -> ast.Module:
    """Parse one Python file using the repository's source encoding."""

    return ast.parse(path.read_text(encoding="utf-8-sig"), filename=str(path))


def _caught_exception_name(node: ast.ExceptHandler) -> str | None:
    """Return the caught exception name when it can be resolved statically."""

    if node.type is None:
        return None
    if isinstance(node.type, ast.Name):
        return node.type.id
    if isinstance(node.type, ast.Attribute):
        return node.type.attr
    if isinstance(node.type, ast.Tuple):
        names = []
        for item in node.type.elts:
            if isinstance(item, ast.Name):
                names.append(item.id)
            elif isinstance(item, ast.Attribute):
                names.append(item.attr)
        return ",".join(names)
    return ast.unparse(node.type)


def _call_name(node: ast.Call) -> str:
    """Return a dotted call target name when one is statically visible."""

    func = node.func
    if isinstance(func, ast.Name):
        return func.id
    if isinstance(func, ast.Attribute):
        parts: list[str] = [func.attr]
        value = func.value
        while isinstance(value, ast.Attribute):
            parts.append(value.attr)
            value = value.value
        if isinstance(value, ast.Name):
            parts.append(value.id)
        return ".".join(reversed(parts))
    return ""


def _handler_has_call(node: ast.ExceptHandler, names: set[str]) -> bool:
    """Return whether the exception handler body calls one of the given names."""

    for child in ast.walk(node):
        if isinstance(child, ast.Call) and _call_name(child) in names:
            return True
    return False


def _handler_has_backend_mapping(node: ast.ExceptHandler) -> bool:
    """Return whether the handler maps the failure to a typed boundary response."""

    for child in ast.walk(node):
        if isinstance(child, ast.Raise):
            if child.exc is None:
                return True
            if (
                isinstance(child.exc, ast.Call)
                and _call_name(child.exc) == "BackendError"
            ):
                return True
        if isinstance(child, ast.Return):
            value = child.value
            if isinstance(value, ast.Call) and _call_name(value) in {
                "json_error",
                "json_error_from_exception",
            }:
                return True
    return False


class _ExceptionVisitor(ast.NodeVisitor):
    """Collect bare and broad exception handlers from one module."""

    def __init__(self, relative_path: str) -> None:
        self.relative_path = relative_path
        self.context: list[str] = []
        self.failures: list[str] = []

    def visit_ClassDef(self, node: ast.ClassDef) -> None:
        """Track the class nesting while visiting child nodes."""

        self.context.append(node.name)
        self.generic_visit(node)
        self.context.pop()

    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
        """Track the function nesting while visiting child nodes."""

        self.context.append(node.name)
        self.generic_visit(node)
        self.context.pop()

    def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> None:
        """Track the async function nesting while visiting child nodes."""

        self.context.append(node.name)
        self.generic_visit(node)
        self.context.pop()

    def visit_ExceptHandler(self, node: ast.ExceptHandler) -> None:
        """Validate one exception handler against the repository policy."""

        context = tuple(self.context)
        if node.type is None:
            self.failures.append(
                f"{self.relative_path}:{node.lineno} uses bare except in {'.'.join(context) or '<module>'}"
            )
            self.generic_visit(node)
            return

        caught_name = _caught_exception_name(node)
        if caught_name == "Exception":
            if (self.relative_path, context) in OPTIONAL_FALLBACK_CONTEXTS:
                if not (
                    _handler_has_call(node, {"warnings.append", "_logger.warning"})
                    or _handler_has_call(
                        node, {"_logger.exception", "LOGGER.exception"}
                    )
                ):
                    self.failures.append(
                        f"{self.relative_path}:{node.lineno} broad catch in {'.'.join(context)} is missing diagnostics"
                    )
            elif not (
                _handler_has_call(node, {"_logger.exception", "_logger.warning"})
                and _handler_has_backend_mapping(node)
            ):
                self.failures.append(
                    f"{self.relative_path}:{node.lineno} broad catch in {'.'.join(context)} is not an allowed boundary wrapper"
                )

        self.generic_visit(node)


def _audit_runtime_exceptions(root: Path) -> list[str]:
    """Audit runtime Python files for bare and disallowed broad exception handlers."""

    failures: list[str] = []
    for path in _iter_runtime_python_files(root):
        relative = path.relative_to(root).as_posix()
        visitor = _ExceptionVisitor(relative)
        visitor.visit(_read_tree(path))
        failures.extend(visitor.failures)
    return failures


def _collect_missing_docstrings(path: Path) -> list[str]:
    """Collect missing module, class, function, and method docstrings."""

    tree = _read_tree(path)
    missing: list[str] = []
    if ast.get_docstring(tree) is None:
        missing.append("<module>")
    for node in tree.body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            if ast.get_docstring(node) is None:
                missing.append(node.name)
            if isinstance(node, ast.ClassDef):
                for item in node.body:
                    if isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef)):
                        if ast.get_docstring(item) is None:
                            missing.append(f"{node.name}.{item.name}")
    return missing


def _audit_docstrings(root: Path) -> list[str]:
    """Audit the remediation docstring scope defined by the plan."""

    failures: list[str] = []
    for relative in DOCSTRING_TARGETS:
        path = root / relative
        missing = _collect_missing_docstrings(path)
        for name in missing:
            failures.append(f"{relative} missing docstring for {name}")
    return failures


def run_audit(root: Path | None = None) -> list[str]:
    """Run the Python standards audit and return human-readable failures."""

    repo_root = root or _repo_root()
    return _audit_runtime_exceptions(repo_root) + _audit_docstrings(repo_root)


def main() -> int:
    """Run the standards audit as a CLI command."""

    failures = run_audit()
    if failures:
        sys.stderr.write("Python standards audit failed:\n")
        for failure in failures:
            sys.stderr.write(f" - {failure}\n")
        return 1
    sys.stdout.write("Python standards audit passed.\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())

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
"""Define typed architecture policy, registry, and diagnostic values."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date
from enum import StrEnum
from pathlib import PurePosixPath


class Severity(StrEnum):
    """Identify whether an architecture diagnostic blocks the gate."""

    WARNING = "warning"
    ERROR = "error"


class WaiverKind(StrEnum):
    """Distinguish cohesive exceptions from mixed-responsibility remediation."""

    STRUCTURAL = "structural"
    REMEDIATION = "remediation"


@dataclass(frozen=True, slots=True)
class StructurePolicy:
    """Describe authored source inventory and structural size thresholds."""

    source_roots: tuple[str, ...]
    extensions: frozenset[str]
    excluded_prefixes: tuple[str, ...]
    soft_lines: int
    hard_lines: int


@dataclass(frozen=True, slots=True)
class DependencyRule:
    """Forbid dependency directions below exact protected path patterns."""

    name: str
    paths: tuple[str, ...]
    forbidden_python_imports: tuple[str, ...]
    forbidden_typescript_paths: tuple[str, ...]

    def owns(self, path: str) -> bool:
        """Return whether this rule protects one repository path."""

        candidate = PurePosixPath(path)
        return any(candidate.match(pattern) for pattern in self.paths)


@dataclass(frozen=True, slots=True)
class MenuPolicy:
    """Declare exact owners for context actions, popup transport and searchable dialog rows."""

    context_menu: str = "frontend/comfyui/ui/menus/ContextMenu.ts"
    floating_surface: str = "frontend/comfyui/ui/menus/FloatingSurface.ts"
    control_runtime: str = "frontend/comfyui/ui/menus/MenuControlRuntime.ts"
    search_dialogs: tuple[str, ...] = ()
    choice_controls: tuple[str, ...] = ()


@dataclass(frozen=True, slots=True)
class EditorBoundary:
    """Identify one editor's package and exclusive native API owner."""

    name: str
    root: str
    infrastructure: str
    modules: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class IntegrationPolicy:
    """Protect portable editor contracts from concrete host dependencies."""

    contracts: str
    shared: str
    editors: tuple[EditorBoundary, ...]


@dataclass(frozen=True, slots=True)
class ArchitecturePolicy:
    """Collect the repository's executable structural architecture contract."""

    schema: int
    structure: StructurePolicy
    dependencies: tuple[DependencyRule, ...]
    mutation_observer_owners: tuple[str, ...] = ()
    document_observer_owners: tuple[str, ...] = ()
    menus: MenuPolicy = MenuPolicy()
    integrations: IntegrationPolicy | None = None


@dataclass(frozen=True, slots=True)
class ArchitectureDebt:
    """Record current, fingerprinted mixed-responsibility source state."""

    identifier: str
    owner: str
    paths: tuple[str, ...]
    fingerprint: str
    review_by: date
    responsibilities: tuple[str, ...]
    next_extraction: str


@dataclass(frozen=True, slots=True)
class ArchitectureWaiver:
    """Bound one exact structural exception in scope, size, and time."""

    identifier: str
    kind: WaiverKind
    rule: str
    path: str
    owner: str
    justification: str
    review_by: date
    max_lines: int
    debt: str | None
    next_limit: int | None


@dataclass(frozen=True, slots=True)
class Diagnostic:
    """Describe one deterministic architecture validation result."""

    path: str
    rule: str
    severity: Severity
    message: str
    line: int = 1

    def render(self) -> str:
        """Render this diagnostic in compiler-compatible form."""

        return (
            f"{self.path}:{self.line}: {self.severity.value} "
            f"{self.rule}: {self.message}"
        )


@dataclass(frozen=True, slots=True)
class ArchitectureResult:
    """Return visible diagnostics and the gate outcome."""

    diagnostics: tuple[Diagnostic, ...]

    @property
    def succeeded(self) -> bool:
        """Return whether no blocking architecture diagnostics remain."""

        return not any(
            diagnostic.severity is Severity.ERROR for diagnostic in self.diagnostics
        )


__all__ = [
    "ArchitectureDebt",
    "ArchitecturePolicy",
    "ArchitectureResult",
    "ArchitectureWaiver",
    "DependencyRule",
    "Diagnostic",
    "Severity",
    "StructurePolicy",
    "WaiverKind",
]

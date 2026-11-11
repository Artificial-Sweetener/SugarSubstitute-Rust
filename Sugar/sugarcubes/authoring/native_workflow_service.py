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
"""Coordinate SugarScript compilation with the native Cube import boundary."""

from __future__ import annotations

from typing import Mapping, Protocol

from ..cube_model import CubeDocument
from ..language import SugarScriptCompileRequest, SugarScriptLanguageService
from ..language.compiler_models import SugarScriptCubeResolver
from ..language.source import (
    DiagnosticSeverity,
    SugarScriptDiagnostic,
)
from .native_workflow_models import (
    NativeCubeImport,
    NativeWorkflowAuthoringResult,
    NativeWorkflowImportPlan,
)


class NativeCubeImportPreparer(Protocol):
    """Prepare a validated in-memory Cube through SugarCubes' normal importer."""

    def prepare(self, document: CubeDocument) -> Mapping[str, object]:
        """Return one JSON-ready payload for native frontend construction."""


class SugarScriptWorkflowAuthoringService:
    """Own the language-to-native-import application use case."""

    def __init__(
        self,
        *,
        language: SugarScriptLanguageService,
        resolver: SugarScriptCubeResolver,
        preparer: NativeCubeImportPreparer,
    ) -> None:
        """Bind language, catalog, and importer ports without host dependencies."""

        self._language = language
        self._resolver = resolver
        self._preparer = preparer

    def compile(self, source: str) -> NativeWorkflowAuthoringResult:
        """Create a complete atomic plan while leaving the host graph untouched."""

        compiled = self._language.compile(
            SugarScriptCompileRequest(source, self._resolver)
        )
        if compiled.plan is None:
            return NativeWorkflowAuthoringResult(None, compiled.diagnostics)
        imports: list[NativeCubeImport] = []
        diagnostics = list(compiled.diagnostics)
        for instance in compiled.plan.instances:
            try:
                payload = self._preparer.prepare(instance.document)
            except (RuntimeError, ValueError) as error:
                diagnostics.append(
                    SugarScriptDiagnostic(
                        "sugarscript.prepare.invalid_cube",
                        f"Cube '{instance.alias}' could not be prepared: {error}",
                        DiagnosticSeverity.ERROR,
                        instance.source_span,
                    )
                )
                continue
            imports.append(
                NativeCubeImport(
                    instance.instance_id,
                    instance.alias,
                    instance.bypassed,
                    payload,
                )
            )
        if any(
            diagnostic.severity is DiagnosticSeverity.ERROR
            for diagnostic in diagnostics
        ):
            return NativeWorkflowAuthoringResult(None, tuple(diagnostics))
        return NativeWorkflowAuthoringResult(
            NativeWorkflowImportPlan(
                compiled.plan.semantic_hash,
                tuple(imports),
                compiled.plan.connections,
                compiled.plan.field_annotations,
            ),
            compiled.diagnostics,
        )

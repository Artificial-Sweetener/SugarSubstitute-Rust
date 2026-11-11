//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Compile complete source-current semantic effects through independently schedulable canonical phases. */
import type { EditorState, StateEffect } from '@codemirror/state';
import {
  prepareNativeStructureSteps,
  installSemanticStructure,
  semanticStructures,
} from './structure.js';
import { prepareDocumentDiagnostics, installDocumentDiagnostics } from './document-diagnostics.js';
import { currentUnmeasuredProjection, installProjection } from './rich-projection.js';
import { prepareProjection } from './projection-preparation.js';
import {
  duplicateDiagnostics,
  prepareNativeDuplicates,
  installNativeDuplicates,
} from './duplicate-diagnostics.js';
import { prepareConditioning, installConditioning } from './conditioning-state.js';

/** Retain partial output privately; the refresh lifetime owner admits only a complete current effect batch. */
export function* prepareSemanticEffects(
  state: EditorState,
): Generator<void, readonly StateEffect<unknown>[], void> {
  const semantic = yield* prepareNativeStructureSteps(state);
  const effects: StateEffect<unknown>[] = [
    installDocumentDiagnostics.of(prepareDocumentDiagnostics(state, semantic.documentSemantics)),
  ];
  yield;
  effects.push(
    installProjection.of(currentUnmeasuredProjection(state) ?? prepareProjection(state, semantic)),
  );
  yield;
  if (state.field(duplicateDiagnostics, false))
    effects.push(installNativeDuplicates.of(prepareNativeDuplicates(state, semantic.nativeValues)));
  yield;
  if (semantic.kind === 'prompt')
    effects.push(installConditioning.of(prepareConditioning(state, semantic)));
  if (semantic !== state.field(semanticStructures).native)
    effects.push(installSemanticStructure.of({ purpose: 'native', prepared: semantic }));
  return effects;
}

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
/** One native state owner indexes canonical duplicate findings and projects selectable source without a second semantic buffer. */
import { requireValue } from '../../core/src/required.js';
import { RangeSet, RangeValue, StateEffect, StateField, type EditorState } from '@codemirror/state';
import { Decoration, EditorView, type DecorationSet } from '@codemirror/view';
import {
  prepareDuplicateDiagnostics,
  type DuplicateDiagnostic,
  type PreparedDuplicateDiagnostics,
} from '../../services/src/diagnostics/duplicates.js';
import { sourcePositionsToDocument } from '../../core/src/line-endings.js';
import { lineEndings, readExactSource } from './exact-source.js';
import { documentKind } from './document-kind.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { conditioningContext } from './conditioning-context.js';
import { featureProfile } from './features.js';
import type { PreparedNativeDocumentValues } from './document-values.js';
import { duplicateIgnores, duplicateIgnoreKey, duplicateIgnored } from './duplicate-ignore.js';
/** The actual finding remains immutable; containing RangeSet coordinates use logical-document UTF-16. */
export class DuplicateDiagnosticValue extends RangeValue {
  readonly diagnostic: DuplicateDiagnostic;
  readonly from: number;
  readonly to: number;
  /** Capture actual canonical output and its logical-document UTF-16 bounds. */
  constructor(diagnostic: DuplicateDiagnostic, from: number, to: number) {
    super();
    this.diagnostic = diagnostic;
    this.from = from;
    this.to = to;
    Object.freeze(this);
  }
  /** Persist only the donor's range/key identity, never a second source. */
  get ignoreKey(): string {
    return duplicateIgnoreKey(
      this.diagnostic.range.from,
      this.diagnostic.range.to,
      this.diagnostic.finding.duplicate.key,
    );
  }
}
/** Prepared source/context binding and native interval index are derived from the sole actual structure. */
export interface NativeDuplicateDiagnostics {
  readonly freshness: EditorFreshness | null;
  readonly prepared: PreparedDuplicateDiagnostics | null;
  readonly findings: RangeSet<DuplicateDiagnosticValue>;
  /** Complete immutable source marks remain available when session suppression is reset. */
  readonly baseDecorations: DecorationSet;
  readonly decorations: DecorationSet;
}
const issued = new WeakSet<NativeDuplicateDiagnostics>();
/** Empty pending state publishes no obsolete finding or edit authority. */
function empty(): NativeDuplicateDiagnostics {
  return Object.freeze({
    freshness: null,
    prepared: null,
    findings: RangeSet.empty,
    baseDecorations: Decoration.none,
    decorations: Decoration.none,
  });
}
/** Profile eligibility is consumed from the prepared public feature decision, not another preference rule. */
function enabled(state: EditorState): boolean {
  return state
    .facet(featureProfile)
    .decisions.some(
      (decision) => decision.feature === 'duplicate_segment_diagnostics' && decision.enabled,
    );
}
/** Install and session changes project from the complete prepared base under the actual authoritative ignore field. */
function ignoredProjection(
  current: NativeDuplicateDiagnostics,
  state: EditorState,
): NativeDuplicateDiagnostics {
  return Object.freeze({
    ...current,
    decorations: current.baseDecorations.update({
      filter: (_from, _to, mark) =>
        !duplicateIgnored(
          state,
          (mark.spec.duplicateDiagnostic as DuplicateDiagnosticValue).ignoreKey,
        ),
    }),
  });
}
/** Prepare actual service findings and bulk exact/logical mapping off native input; raw and rich modes share the same source marks. */
export function prepareNativeDuplicates(
  state: EditorState,
  nativeValues: PreparedNativeDocumentValues,
): NativeDuplicateDiagnostics {
  const previous = state.field(duplicateDiagnostics, false);
  const freshness = state.field(editorFreshness);
  if (
    nativeValues.sourceIdentity !== freshness.source ||
    nativeValues.document !== state.doc ||
    nativeValues.endings !== state.field(lineEndings) ||
    nativeValues.scopes.values.source !== readExactSource(state) ||
    nativeValues.scopes.values.kind !== state.facet(documentKind)
  )
    throw new Error('duplicate-structure-currentness');
  if (previous?.freshness === freshness) {
    issued.add(previous);
    return previous;
  }
  const prepared = enabled(state)
    ? prepareDuplicateDiagnostics(
        nativeValues.scopes,
        state.facet(conditioningContext),
        previous?.prepared ?? undefined,
      )
    : null;
  const diagnostics = prepared?.findings ?? [];
  const positions = sourcePositionsToDocument(
    state.doc.toString(),
    state.field(lineEndings),
    diagnostics.flatMap((value) => [value.range.from, value.range.to]),
  );
  const values = diagnostics.map(
    (value, index) =>
      new DuplicateDiagnosticValue(
        value,
        requireValue(positions[index * 2], 'duplicate-diagnostics'),
        requireValue(positions[index * 2 + 1], 'duplicate-diagnostics'),
      ),
  );
  const findings = RangeSet.of(
    values.map((value, index) =>
      value.range(
        requireValue(positions[index * 2], 'duplicate-diagnostics'),
        requireValue(positions[index * 2 + 1], 'duplicate-diagnostics'),
      ),
    ),
    true,
  );
  const baseDecorations = Decoration.set(
    values.map((value, index) =>
      Decoration.mark({
        class: 'sp-duplicate-segment',
        duplicateDiagnostic: value,
        attributes: {
          title: value.diagnostic.message,
          'aria-invalid': 'true',
        },
      }).range(
        requireValue(positions[index * 2], 'duplicate-diagnostics'),
        requireValue(positions[index * 2 + 1], 'duplicate-diagnostics'),
      ),
    ),
    true,
  );
  const result = ignoredProjection(
    {
      freshness,
      prepared,
      findings,
      baseDecorations,
      decorations: baseDecorations,
    },
    state,
  );
  Object.freeze(result);
  issued.add(result);
  return result;
}
/** Only genuine same-source/context owner preparations may publish indexed intent. */
export const installNativeDuplicates = StateEffect.define<NativeDuplicateDiagnostics>();
/** Current finding membership is required before ignore or genuine decoded recipe preparation. */
export function currentDuplicateDiagnostic(
  state: EditorState,
  value: DuplicateDiagnosticValue,
): boolean {
  const current = state.field(duplicateDiagnostics, false);
  if (
    !current ||
    current.freshness !== state.field(editorFreshness) ||
    duplicateIgnored(state, value.ignoreKey)
  )
    return false;
  let found = false;
  current.findings.between(value.from, value.to, (_from, _to, item) => {
    if (item === value) found = true;
  });
  return found;
}
/** Source/profile/mode/permission changes retire preparation; ignores survive in their separate mounted-session field. */
export const duplicateDiagnostics = StateField.define<NativeDuplicateDiagnostics>({
  create: empty,
  update(current, transaction) {
    const state = transaction.state;
    if (current.freshness !== state.field(editorFreshness)) current = empty();
    let project =
      transaction.startState.field(duplicateIgnores, false) !== state.field(duplicateIgnores);
    for (const effect of transaction.effects)
      if (
        effect.is(installNativeDuplicates) &&
        issued.has(effect.value) &&
        effect.value.freshness === state.field(editorFreshness)
      ) {
        current = effect.value;
        project = true;
      }
    if (project) current = ignoredProjection(current, state);
    return current;
  },
  provide: (field) => EditorView.decorations.from(field, (value) => value.decorations),
});

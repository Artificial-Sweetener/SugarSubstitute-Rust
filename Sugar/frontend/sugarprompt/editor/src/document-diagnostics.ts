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
/** Source-bound candidate errors are prepared once and rendered over native editable source in rich and raw presentation. */
import { requireValue } from '../../core/src/required.js';
import { RangeSet, RangeValue, StateEffect, StateField, type EditorState } from '@codemirror/state';
import { Decoration, EditorView, type DecorationSet } from '@codemirror/view';
import {
  unsupportedSceneDiagnostics,
  type UnsupportedSceneDiagnostic,
} from '../../services/src/index.js';
import type { PreparedDocumentSemantics, PromptDocumentKind } from '../../core/src/index.js';
import { sourcePositionsToDocument } from '../../core/src/line-endings.js';
import { documentKind } from './document-kind.js';
import { changesExactSource, lineEndings } from './exact-source.js';
import { editorFreshness } from './freshness.js';
/** Native range indexing retains the actual immutable service finding without prompt text. */
export class DocumentDiagnosticValue extends RangeValue {
  readonly diagnostic: UnsupportedSceneDiagnostic;
  /** Capture one prepared finding; native coordinates live in its containing RangeSet. */
  constructor(diagnostic: UnsupportedSceneDiagnostic) {
    super();
    this.diagnostic = diagnostic;
  }
}
/** Prepared logical-document UTF-16 errors are bound to exact source and document meaning. */
export interface DocumentDiagnostics {
  readonly source: symbol | null;
  readonly kind: PromptDocumentKind;
  readonly findings: RangeSet<DocumentDiagnosticValue>;
  readonly decorations: DecorationSet;
}
/** Empty/pending state contains no retained source or diagnostic intent. */
function empty(kind: PromptDocumentKind): DocumentDiagnostics {
  return {
    source: null,
    kind,
    findings: RangeSet.empty,
    decorations: Decoration.none,
  };
}
/** Prepare findings and bulk source/logical conversion outside native input, measurement and paint. */
export function prepareDocumentDiagnostics(
  state: EditorState,
  semantics: PreparedDocumentSemantics,
): DocumentDiagnostics {
  const current = state.field(documentDiagnostics, false);
  if (current?.source === state.field(editorFreshness).source && current.kind === semantics.kind)
    return current;
  const diagnostics = unsupportedSceneDiagnostics(semantics);
  const positions = sourcePositionsToDocument(
    state.doc.toString(),
    state.field(lineEndings),
    diagnostics.flatMap(({ range }) => [range.from, range.to]),
  );
  const values = diagnostics.map((diagnostic) => new DocumentDiagnosticValue(diagnostic));
  return {
    source: state.field(editorFreshness).source,
    kind: semantics.kind,
    findings: RangeSet.of(
      values.map((value, index) =>
        value.range(
          requireValue(positions[index * 2], 'document-diagnostics'),
          requireValue(positions[index * 2 + 1], 'document-diagnostics'),
        ),
      ),
      true,
    ),
    decorations: Decoration.set(
      values.map((value, index) =>
        Decoration.mark({
          class: 'sp-unsupported-scene',
          attributes: {
            title: value.diagnostic.message,
            'aria-invalid': 'true',
          },
        }).range(
          requireValue(positions[index * 2], 'document-diagnostics'),
          requireValue(positions[index * 2 + 1], 'document-diagnostics'),
        ),
      ),
      true,
    ),
  };
}
/** Publish only already prepared source-current errors. */
export const installDocumentDiagnostics = StateEffect.define<DocumentDiagnostics>();
/** Exact edits retire error intent immediately; obsolete source/kind publications never restore it. */
export const documentDiagnostics = StateField.define<DocumentDiagnostics>({
  create: (state) => empty(state.facet(documentKind)),
  update(current, transaction) {
    const kind = transaction.state.facet(documentKind);
    if (changesExactSource(transaction) || current.kind !== kind) current = empty(kind);
    for (const effect of transaction.effects)
      if (
        effect.is(installDocumentDiagnostics) &&
        effect.value.source === transaction.state.field(editorFreshness).source &&
        effect.value.kind === kind
      )
        current = effect.value;
    return current;
  },
  provide: (field) => [
    EditorView.decorations.from(field, (value) => value.decorations),
    EditorView.editorAttributes.compute([field, editorFreshness], (state) => ({
      'data-document-diagnostic-source':
        state.field(field).source === state.field(editorFreshness).source ? 'ready' : 'pending',
    })),
  ],
});

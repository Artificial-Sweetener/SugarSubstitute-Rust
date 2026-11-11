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
/** Non-ignorable candidate errors use the shared finite context overlay and have explanatory intent only. */
import type { EditorState } from '@codemirror/state';
import type { ContextMenuItem } from './context-menu-contracts.js';
import { documentDiagnostics, type DocumentDiagnosticValue } from './document-diagnostics.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';

/** Native target bounds use logical-document UTF-16 and retain the actual prepared finding. */
export interface DocumentMenuTarget {
  readonly from: number;
  readonly to: number;
  readonly finding: DocumentDiagnosticValue;
}
/** The disabled explanation remains inspectable with read-only permission and an active caret. */
export interface PreparedDocumentMenu extends DocumentMenuTarget {
  readonly kind: 'unsupported-scene';
  readonly freshness: EditorFreshness;
  readonly items: readonly ContextMenuItem[];
}

/** Bounded half-open hit testing consumes current indexed findings without source traversal. */
export function documentMenuTargetAt(
  state: EditorState,
  position: number,
): DocumentMenuTarget | null {
  const prepared = state.field(documentDiagnostics, false);
  if (!prepared || prepared.source !== state.field(editorFreshness).source) return null;
  let target: DocumentMenuTarget | null = null;
  prepared.findings.between(position, position, (from, to, finding) => {
    if (from <= position && position < to) target = { from, to, finding };
  });
  return target;
}
/** Source-preserving disclosure captures the current native command context. */
export function prepareDocumentMenu(
  state: EditorState,
  target: DocumentMenuTarget,
): PreparedDocumentMenu {
  return {
    ...target,
    kind: 'unsupported-scene',
    freshness: state.field(editorFreshness),
    items: Object.freeze([
      Object.freeze({
        kind: 'disabled',
        label: target.finding.diagnostic.message,
      }),
    ]),
  };
}
/** Source/kind/context retirement and replacement findings invalidate prior explanations. */
export function currentDocumentMenu(state: EditorState, menu: PreparedDocumentMenu): boolean {
  if (state.field(editorFreshness) !== menu.freshness) return false;
  const target = documentMenuTargetAt(state, menu.from);
  return target?.finding === menu.finding && target.to === menu.to;
}

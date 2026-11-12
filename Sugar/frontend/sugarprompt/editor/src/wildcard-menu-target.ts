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
/** Missing-wildcard menu eligibility consumes indexed source-current findings, never another parser or catalog lookup. */
import type { EditorState } from '@codemirror/state';
import { diagnosticIsActive } from '../../core/src/index.js';
import type { WildcardCatalog } from '../../services/src/index.js';
import type { ContextMenuItem } from './context-menu-contracts.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { richProjection } from './rich-projection.js';
import { wildcardCatalog } from './wildcard-catalog.js';
import type { WildcardDiagnosticValue } from './wildcard-diagnostic-value.js';

/** One immutable explanatory row is deliberately disabled: missing sources have no executable repair action. */
const missingItems: readonly ContextMenuItem[] = Object.freeze([
  Object.freeze({ kind: 'disabled', label: 'Wildcard not found' }),
]);

/** Logical-document UTF-16 target bounds retain the actual prepared finding identity. */
export interface WildcardMenuTarget {
  readonly from: number;
  readonly to: number;
  readonly finding: WildcardDiagnosticValue;
}

/** A finite diagnostic presentation captures source/profile and prepared catalog identities. */
export interface PreparedWildcardMenu extends WildcardMenuTarget {
  readonly kind: 'wildcard';
  readonly freshness: EditorFreshness;
  readonly catalog: WildcardCatalog | null;
  readonly items: readonly ContextMenuItem[];
}

/** Half-open diagnostic hit testing follows the reference's visible-only menu policy; the native caret suppresses active editing. */
export function wildcardMenuTargetAt(
  state: EditorState,
  position: number,
): WildcardMenuTarget | null {
  const projection = state.field(richProjection),
    caret = state.selection.main.head;
  if (
    projection.sourceIdentity !== state.field(editorFreshness).source ||
    projection.wildcardCatalog !== state.facet(wildcardCatalog)
  )
    return null;
  let target: WildcardMenuTarget | null = null;
  projection.wildcardDiagnostics.between(position, position, (from, to, finding) => {
    if (
      from <= position &&
      position < to &&
      !diagnosticIsActive(from, to, caret, finding.committedAtEnd)
    )
      target = { from, to, finding };
  });
  return target;
}

/** Capture prepared diagnostic presentation without source changes or external work. */
export function prepareWildcardMenu(
  state: EditorState,
  target: WildcardMenuTarget,
): PreparedWildcardMenu {
  return {
    ...target,
    kind: 'wildcard',
    freshness: state.field(editorFreshness),
    catalog: state.facet(wildcardCatalog),
    items: missingItems,
  };
}

/** Publication and continued visibility require the same source, catalog, range and actual finding. */
export function currentWildcardMenu(state: EditorState, menu: PreparedWildcardMenu): boolean {
  if (
    state.field(editorFreshness) !== menu.freshness ||
    state.facet(wildcardCatalog) !== menu.catalog
  )
    return false;
  const target = wildcardMenuTargetAt(state, menu.from);
  return target?.finding === menu.finding && target.to === menu.to;
}

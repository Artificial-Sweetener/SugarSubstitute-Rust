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
/** Actual spelling issue membership prepares literal menu rows and genuine canonical corrective commands. */
import type { EditorState } from '@codemirror/state';
import type { PreparedSpellingSuggestions } from '../../services/src/diagnostics/spelling-contracts.js';
import {
  spellingDiagnosticAt,
  currentSpellingDiagnostic,
  type SpellingDiagnosticValue,
} from './spelling-diagnostics.js';
import { semanticStructures } from './structure.js';
import { nativeDocumentValueAt } from './document-values.js';
import { prepareDocumentValuePaste } from './document-value-ingestion.js';
import { prepareDocumentValueEdit } from './document-value-edit-issuance.js';
import { richRendering } from './presentation.js';
import { spellingLexicon, isNativeSpellingSuggestions } from './spelling-provider.js';
import type { ContextMenuItem } from './context-menu-contracts.js';

/** One indexed word supplies logical-document UTF16 bounds and genuine service membership. */
export interface SpellingMenuTarget {
  readonly from: number;
  readonly to: number;
  readonly finding: SpellingDiagnosticValue;
}
/** The finite menu retains exact leaf identities; suggestions are literal provider output, not a new syntax owner. */
export interface PreparedSpellingMenu extends SpellingMenuTarget {
  readonly kind: 'spelling';
  readonly items: readonly ContextMenuItem[];
  readonly editLeaves: ReadonlySet<Extract<ContextMenuItem, { kind: 'source-edit' }>>;
  readonly ignoreLeaf: Extract<ContextMenuItem, { kind: 'spelling-action' }>;
  readonly addLeaf: Extract<ContextMenuItem, { kind: 'spelling-action' }> | null;
  readonly suggestions: PreparedSpellingSuggestions | null;
}
const targets = new WeakSet<SpellingMenuTarget>();
const menus = new WeakSet<PreparedSpellingMenu>();
/** Bounded source-backed hit testing delegates the same active-token and currentness owner as marks. */
export function spellingMenuTargetAt(
  state: EditorState,
  position: number,
): SpellingMenuTarget | null {
  const finding = spellingDiagnosticAt(state, position);
  if (!finding) return null;
  const target = Object.freeze({ from: finding.from, to: finding.to, finding });
  targets.add(target);
  return target;
}
/** Canonical source/selection/authorship/history are prepared before leaf issuance, never reconstructed after it. */
export function prepareSpellingMenu(
  state: EditorState,
  target: SpellingMenuTarget,
  suggestions: PreparedSpellingSuggestions | null = null,
  settlement: 'pending' | 'unavailable' = 'pending',
): PreparedSpellingMenu | null {
  if (
    !targets.has(target) ||
    !currentSpellingDiagnostic(state, target.finding) ||
    target.from !== target.finding.from ||
    target.to !== target.finding.to ||
    (suggestions &&
      (!isNativeSpellingSuggestions(suggestions) ||
        suggestions.snapshot !== target.finding.snapshot ||
        suggestions.issue !== target.finding.issue))
  )
    return null;
  const native = state.field(semanticStructures).native;
  if (!native) return null;
  const context = {
      values: native.nativeValues,
      semantics: native.documentSemantics,
      enabled: !state.readOnly,
      composing: false,
    },
    admitted = nativeDocumentValueAt(state, context, {
      from: target.from,
      to: target.to,
    });
  const items: ContextMenuItem[] = [],
    editLeaves = new Set<Extract<ContextMenuItem, { kind: 'source-edit' }>>();
  for (const text of suggestions?.suggestions ?? []) {
    let leaf: Extract<ContextMenuItem, { kind: 'source-edit' }> | null = null;
    if (admitted.status === 'ready' && admitted.target.scope === target.finding.issue.group.scope) {
      const value = admitted.target,
        range = target.finding.issue.candidate.range,
        rich = state.facet(richRendering),
        normalization = rich
          ? prepareDocumentValuePaste(value, range, text, state.facet(spellingLexicon)())
          : null;
      const position = normalization
        ? normalization.normalization.boundaryPositions[range.from + text.length]
        : range.from + text.length;
      if (position != null) {
        const result = prepareDocumentValueEdit(state, {
          context,
          target: value,
          range: normalization ? { from: 0, to: value.scope.mapping.text.length } : range,
          text: normalization ? normalization.normalization.text : text,
          publication: {
            purpose: 'spelling',
            ...(normalization ? { normalization } : {}),
          },
          selection: {
            ranges: [{ kind: 'decoded', anchor: position, head: position }],
            mainIndex: 0,
          },
        });
        if (result.status !== 'rejected')
          leaf = Object.freeze({
            kind: 'source-edit',
            purpose: 'spelling',
            label: text,
            title: 'Correct spelling',
            replacement: result.edit,
          });
      }
    }
    if (leaf) {
      editLeaves.add(leaf);
      items.push(leaf);
    } else items.push(Object.freeze({ kind: 'disabled', label: text }));
  }
  if (!items.length)
    items.push(
      Object.freeze({
        kind: 'disabled',
        label: suggestions
          ? 'No spelling suggestions'
          : settlement === 'unavailable'
            ? 'Spelling unavailable'
            : 'Loading spelling suggestions',
      }),
    );
  const ignoreLeaf = Object.freeze({
    kind: 'spelling-action' as const,
    action: 'ignore' as const,
    label: 'Ignore spelling',
    finding: target.finding,
  });
  const addLeaf = target.finding.snapshot.dictionaryAdd
    ? Object.freeze({
        kind: 'spelling-action' as const,
        action: 'add' as const,
        label: 'Add to dictionary',
        finding: target.finding,
      })
    : null;
  items.push(ignoreLeaf);
  if (addLeaf) items.push(addLeaf);
  const menu: PreparedSpellingMenu = Object.freeze({
    ...target,
    kind: 'spelling',
    items: Object.freeze(items),
    editLeaves,
    ignoreLeaf,
    addLeaf,
    suggestions,
  });
  menus.add(menu);
  return menu;
}
/** Current menu authority comes from its exact indexed finding and genuine service revision. */
export function currentSpellingMenu(state: EditorState, menu: PreparedSpellingMenu): boolean {
  return (
    menus.has(menu) &&
    currentSpellingDiagnostic(state, menu.finding) &&
    menu.from === menu.finding.from &&
    menu.to === menu.finding.to
  );
}

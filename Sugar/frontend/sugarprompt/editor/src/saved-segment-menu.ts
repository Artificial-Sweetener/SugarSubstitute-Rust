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
/** Public native configuration binds storage and model context with one observable failure boundary. */
import { Facet, StateEffect, StateField, type EditorState } from '@codemirror/state';
import { documentToSource } from '../../core/src/line-endings.js';
import { strip } from '../../core/src/prompt/text.js';
import { captureRecord } from '../../services/src/saved-segments/capture.js';
import type {
  SavedSegmentAdapter,
  SavedSegmentCatalog,
  SavedSegmentContext,
} from '../../services/src/saved-segments/contracts.js';
import {
  captureSavedSegmentContext,
  validateSavedSegmentAdapter,
} from '../../services/src/saved-segments/preparation.js';
import { lineEndings, readExactSource } from './exact-source.js';
import type { ContextMenuItem } from './context-menu-contracts.js';
export interface SavedSegmentHost {
  readonly adapter: SavedSegmentAdapter;
  readonly context: SavedSegmentContext;
  report(cause: unknown): void;
}
/** Validate injected capability and context without invoking host storage. */
export function validateSavedSegmentHost(value: SavedSegmentHost | null | undefined): void {
  if (value == null) return;
  const host = captureRecord(value, ['adapter', 'context', 'report']);
  if (typeof host.report !== 'function') throw new TypeError('invalid-saved-segment-host');
  validateSavedSegmentAdapter(host.adapter);
  captureSavedSegmentContext(host.context);
}
/** One configuration facet identifies the finite mounted storage lifetime. */
export const savedSegmentHost = Facet.define<SavedSegmentHost | null, SavedSegmentHost | null>({
  combine: (values) => values.at(-1) ?? null,
});
/** Catalog availability is prepared away from menu opening and painting. */
export interface SavedSegmentCatalogPublication {
  readonly host: SavedSegmentHost;
  readonly status: 'pending' | 'ready' | 'unavailable';
  readonly catalog: SavedSegmentCatalog | null;
}
/** Genuine catalog leaves expose membership and iteration without exposing a mutable set. */
interface SavedSegmentInsertLeaves
  extends Iterable<Extract<ContextMenuItem, { kind: 'insert-segment' }>> {
  readonly size: number;
  has(leaf: Extract<ContextMenuItem, { kind: 'insert-segment' }>): boolean;
}
/** Literal insertion presentation belongs to one accepted catalog publication. */
interface SavedSegmentInsertion {
  readonly item: ContextMenuItem;
  readonly leaves: SavedSegmentInsertLeaves;
}
/** Native publication owns its prepared presentation until host or catalog retirement. */
export interface SavedSegmentPublication extends SavedSegmentCatalogPublication {
  readonly insertion: SavedSegmentInsertion | null;
}
/** Disabled menus share immutable empty membership without retaining any catalog. */
const emptyInsertLeaves: SavedSegmentInsertLeaves = Object.freeze({
  size: 0,
  has: () => false,
  [Symbol.iterator]: () => [].values(),
});
/** Project already grouped service output once, only after native host admission. */
function prepareSavedSegmentInsertion(catalog: SavedSegmentCatalog): SavedSegmentInsertion {
  const rows: ContextMenuItem[] = [];
  const leaves = new Set<Extract<ContextMenuItem, { kind: 'insert-segment' }>>();
  for (const group of catalog.groups) {
    if (!group.segments.length) continue;
    if (rows.length) rows.push(Object.freeze({ kind: 'separator' }));
    if (catalog.groups.length > 1)
      rows.push(Object.freeze({ kind: 'heading', label: group.scope.label }));
    for (const segment of group.segments) {
      const leaf = Object.freeze({
        kind: 'insert-segment' as const,
        label: segment.label,
        segment,
      });
      leaves.add(leaf);
      rows.push(leaf);
    }
  }
  return Object.freeze({
    item: rows.length
      ? Object.freeze({
          kind: 'submenu',
          label: 'Insert saved segment',
          children: Object.freeze(rows),
        })
      : Object.freeze({
          kind: 'disabled',
          label: 'Insert saved segment',
          title: 'No saved segments apply to this context.',
        }),
    leaves: Object.freeze({
      size: leaves.size,
      has: (leaf: Extract<ContextMenuItem, { kind: 'insert-segment' }>) => leaves.has(leaf),
      [Symbol.iterator]: () => leaves.values(),
    }),
  });
}
/** Source-neutral publication never adds a native history entry. */
export const publishSavedSegments = StateEffect.define<SavedSegmentCatalogPublication>();
/** Host identity replacement retires the old catalog before a new host can publish. */
export const savedSegments = StateField.define<SavedSegmentPublication | null>({
  create: () => null,
  update(current, transaction) {
    if (current?.host !== transaction.state.facet(savedSegmentHost)) current = null;
    for (const effect of transaction.effects)
      if (
        effect.is(publishSavedSegments) &&
        effect.value.host === transaction.state.facet(savedSegmentHost)
      )
        current = Object.freeze({
          ...effect.value,
          insertion:
            effect.value.status === 'ready' && effect.value.catalog
              ? prepareSavedSegmentInsertion(effect.value.catalog)
              : null,
        });
    return current;
  },
});
/** Menu commands retain exact captured source UTF-16 replacement boundaries and genuine leaf identities. */
export interface PreparedSavedSegmentMenu {
  readonly publication: SavedSegmentPublication | null;
  readonly from: number;
  readonly to: number;
  readonly selectedText: string;
  readonly saveLeaf: Extract<ContextMenuItem, { kind: 'save-segment' }> | null;
  readonly insertLeaves: SavedSegmentInsertLeaves;
  readonly items: readonly ContextMenuItem[];
}
/** Capture the real directional selection once; an empty selection uses the clicked logical-document target. */
export function prepareSavedSegmentMenu(
  state: EditorState,
  position: number,
): PreparedSavedSegmentMenu {
  const publication = state.field(savedSegments, false) ?? null,
    catalog = publication?.catalog;
  const selection = state.selection.main,
    logical = state.doc.toString(),
    endings = state.field(lineEndings);
  const from = documentToSource(logical, endings, selection.empty ? position : selection.from),
    to = documentToSource(logical, endings, selection.empty ? position : selection.to);
  const selectedText = selection.empty ? '' : readExactSource(state).slice(from, to);
  const available = publication?.status === 'ready' && catalog !== null && catalog !== undefined;
  const reason = state.readOnly
    ? 'This prompt is read-only.'
    : !available
      ? 'Saved segment storage is unavailable.'
      : 'Select non-empty source text first.';
  const saveLeaf: PreparedSavedSegmentMenu['saveLeaf'] =
    !state.readOnly && available && strip(selectedText)
      ? Object.freeze({ kind: 'save-segment', label: 'Save selected segment…' })
      : null;
  const insertion = !state.readOnly ? publication?.insertion : null;
  const insertLeaves = insertion?.leaves ?? emptyInsertLeaves;
  const items: ContextMenuItem[] = [
    saveLeaf ?? {
      kind: 'disabled',
      label: 'Save selected segment…',
      title: reason,
    },
  ];
  if (insertion) items.push(insertion.item);
  else
    items.push({
      kind: 'disabled',
      label: 'Insert saved segment',
      title: reason,
    });
  return Object.freeze({
    publication,
    from,
    to,
    selectedText,
    saveLeaf,
    insertLeaves,
    items: Object.freeze(items),
  });
}
/** Publication identity retirement invalidates every captured catalog leaf. */
export function currentSavedSegmentMenu(
  state: EditorState,
  menu: PreparedSavedSegmentMenu,
): boolean {
  return (state.field(savedSegments, false) ?? null) === menu.publication;
}

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
/** A page of real text boxes on which the finder's highlights and real selections can be compared. */
import { matchSearch } from '../../../frontend/comfyui/ui/search/domain/SearchMatching.js';
import { buildSearchQuery } from '../../../frontend/comfyui/ui/search/domain/SearchQuery.js';
import { ensureFinderStyles } from '../../../frontend/comfyui/ui/search/presentation/FinderStyles.js';
import { SearchHighlights } from '../../../frontend/comfyui/ui/search/presentation/SearchHighlights.js';
import type {
  SearchFieldSurface,
  SearchSurface,
} from '../../../frontend/comfyui/ui/search/presentation/SearchSurface.js';
import {
  SEARCH_CURRENT_HIGHLIGHT,
  SEARCH_MATCH_HIGHLIGHT,
} from '../../../frontend/comfyui/ui/search/presentation/SearchHighlightRanges.js';

import { PROOF_MATCH, PROOF_SELECTION } from './FinderHighlightColours.js';

declare global {
  interface Window {
    finderHighlightProof: {
      /** Show every match of `needle`, the `ordinal`-th of them in `field` as the current one. */
      highlight(needle: string, field: string, ordinal: number): number;
      /** Remove the highlights and select the same occurrence in the text box for real. */
      select(needle: string, field: string, ordinal: number): void;
      clear(): void;
    };
  }
}

const style = document.createElement('style');
style.textContent = `
  body { margin:20px; background:#202020; color:#eee; font:14px/20px "Segoe UI",Arial,sans-serif; }
  .row { margin:0 0 24px; width:360px; }
  input, textarea { box-sizing:border-box; width:100%; margin:0; padding:6px 10px; border:1px solid #555; border-radius:4px;
    background:#333; color:#eee; font:inherit; outline:none; caret-color:transparent; resize:none; }
  textarea { height:92px; }
  ::selection, ::highlight(${SEARCH_CURRENT_HIGHLIGHT}), [data-sc-search="current"] { background:${PROOF_SELECTION}; color:black; }
  ::highlight(${SEARCH_MATCH_HIGHLIGHT}), [data-sc-search="match"] { background-color:${PROOF_MATCH}; }
  :root { --sc-selection-ink:black; }`;
document.head.append(style);
ensureFinderStyles(document);

const card = document.createElement('article');
document.body.append(card);
const fields = new Map<string, SearchFieldSurface>();
const values: Record<string, string> = {
  line: 'sunset_render over the harbour, a second sunset further along the line than the box is wide',
  wrapped:
    'masterpiece, best quality, a sunset over the harbour, detailed background, another sunset behind the hills, soft light',
  scrolled: Array.from(
    { length: 14 },
    (_, row) => `line ${String(row)} of text, sunset ${String(row)}`,
  ).join('\n'),
};
for (const [id, value] of Object.entries(values)) {
  const row = document.createElement('div');
  row.className = 'row';
  row.id = `row-${id}`;
  const control = document.createElement(id === 'line' ? 'input' : 'textarea');
  control.id = id;
  control.value = value;
  row.append(control);
  card.append(row);
  fields.set(id, { id, node: 'n', card, row, label: null, control, painter: null });
}
const surface: SearchSurface = {
  fields,
  read: () => [
    {
      id: 'n',
      name: 'Node',
      title: 'Node',
      type: 'Node',
      fields: [...fields.values()].map((field) => ({
        id: field.id,
        key: field.id,
        label: field.id,
        text: field.control?.value ?? null,
      })),
    },
  ],
};
const highlights = new SearchHighlights(document);

window.finderHighlightProof = {
  highlight(needle, field, ordinal) {
    const result = matchSearch(surface.read(), buildSearchQuery('text', needle));
    const active = result.matches.filter((match) => match.field === field)[ordinal] ?? null;
    highlights.render(surface, result, active);
    if (active) highlights.reveal(active);
    return result.matches.length;
  },
  select(needle, field, ordinal) {
    highlights.clear();
    const control = fields.get(field)?.control;
    if (!control) throw new Error('Unknown field');
    const result = matchSearch(surface.read(), buildSearchQuery('text', needle));
    const active = result.matches.filter((match) => match.field === field)[ordinal];
    if (!active) throw new Error('Unknown occurrence');
    control.focus({ preventScroll: true });
    control.setSelectionRange(active.start, active.start + active.length);
  },
  clear() {
    highlights.clear();
    const focused = document.activeElement;
    if (focused instanceof HTMLElement) focused.blur();
  },
};

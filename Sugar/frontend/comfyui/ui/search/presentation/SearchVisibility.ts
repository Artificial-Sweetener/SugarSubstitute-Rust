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
/** Apply transient finder filtering without changing node flags, widget values or persistent card policy. */
import type { SearchResult } from '../domain/SearchMatching.js';
import type { SearchFieldSurface } from './SearchSurface.js';
import { prepareNativePresentation } from '../../surface/NativePresentationReady.js';

/** Keep row masks separate from the masonry owner's card packing and visibility. */
export class SearchVisibility {
  readonly #fields: ReadonlyMap<string, SearchFieldSurface>;
  readonly #setCards: (nodes: ReadonlySet<string> | null, remeasure: boolean) => void;
  readonly #hidden = new Set<HTMLElement>();

  /** Bind only the identified native fields and the authoritative layout owner. */
  constructor(
    fields: ReadonlyMap<string, SearchFieldSurface>,
    setCards: (nodes: ReadonlySet<string> | null, remeasure: boolean) => void,
  ) {
    this.#fields = fields;
    this.#setCards = setCards;
  }
  /** Hide unmatched rows in Field mode and unmatched cards in Field or Node mode. */
  apply(result: SearchResult | null): void {
    const filter = result && result.query.normalized ? result : null;
    const fieldFilter = filter?.query.mode === 'field' ? filter : null;
    const changed = new Set<HTMLElement>();
    if (!fieldFilter) {
      for (const row of this.#hidden) row.removeAttribute('data-sc-search-field-hidden');
      for (const field of this.#fields.values())
        if (this.#hidden.has(field.row)) changed.add(field.card);
      this.#hidden.clear();
    }
    for (const field of this.#fields.values()) {
      const hidden = fieldFilter !== null && !fieldFilter.fields.has(field.id);
      if (hidden === this.#hidden.has(field.row)) continue;
      if (hidden) {
        field.row.setAttribute('data-sc-search-field-hidden', '');
        this.#hidden.add(field.row);
      } else {
        field.row.removeAttribute('data-sc-search-field-hidden');
        this.#hidden.delete(field.row);
      }
      changed.add(field.card);
    }
    for (const card of changed) prepareNativePresentation(card);
    this.#setCards(filter && filter.query.mode !== 'text' ? filter.nodes : null, changed.size > 0);
  }
  /** Restore the native row presentation before returning borrowed controls to the graph. */
  dispose(): void {
    this.apply(null);
  }
}

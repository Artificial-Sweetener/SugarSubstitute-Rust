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
/** Own finder intent and active-match navigation independently of mounted controls. */
import { buildSearchQuery, parseSearchInput, } from '../domain/SearchQuery.js';
import { matchSearch, } from '../domain/SearchMatching.js';
/** Retain only portable intent when a masonry surface is released or its workflow changes. */
export class SearchSession {
    #state;
    #nodes = [];
    #result;
    #index = -1;
    /** Restore saved finder intent without retaining graph objects or native widgets. */
    constructor(state) {
        this.#state = state ?? {
            open: false,
            mode: 'text',
            text: '',
            active: null,
            scrollTop: 0,
            query: buildSearchQuery('text', ''),
        };
        this.#result = matchSearch([], this.#state.query);
    }
    /** Return the portable state used by the host's workflow-scoped retention. */
    get state() {
        return this.#state;
    }
    /** Return cached results so keyboard cycling never rediscovers the graph. */
    get result() {
        return this.#result;
    }
    /** Return the active position in selectable result order. */
    get index() {
        return this.#index;
    }
    /** Remember visibility while letting the presenter clear filters on close. */
    setOpen(open) {
        this.#state = { ...this.#state, open };
    }
    /** Remember the user's masonry location before its temporary surface is released. */
    setScrollTop(scrollTop) {
        this.#state = { ...this.#state, scrollTop };
    }
    /** Parse explicit commands and choose the first new match without evaluating incomplete commands. */
    setInput(mode, text) {
        const input = parseSearchInput(mode, text);
        this.#state = { ...this.#state, mode: input.mode, text: input.text };
        if (input.pending)
            return false;
        this.#state = { ...this.#state, query: buildSearchQuery(input.mode, input.text) };
        this.#refresh(null);
        return true;
    }
    /** Replace a lifecycle-owned corpus while preserving exact or nearest same-field selection. */
    setCorpus(nodes) {
        this.#nodes = nodes;
        this.#refresh(this.#state.active);
    }
    /** Limit navigation to selectable native fields without changing semantic matching or visibility. */
    selectable(fields) {
        this.#result = {
            ...this.#result,
            matches: this.#result.matches.filter((match) => fields.has(match.field)),
        };
        this.#select(this.#state.active);
    }
    /** Wrap forward or backward through already-computed native text matches. */
    cycle(direction) {
        const count = this.#result.matches.length;
        this.#index = count ? (this.#index + direction + count) % count : -1;
        const active = this.#result.matches[this.#index] ?? null;
        this.#state = { ...this.#state, active };
        return active;
    }
    /** Compute results only at query or source-value boundaries. */
    #refresh(preferred) {
        this.#result = matchSearch(this.#nodes, this.#state.query);
        this.#select(preferred);
    }
    /** Keep navigation intent stable through value edits and native surface remounts. */
    #select(preferred) {
        let index = preferred
            ? this.#result.matches.findIndex((match) => match.field === preferred.field &&
                match.start === preferred.start &&
                match.length === preferred.length)
            : -1;
        if (index < 0 && preferred) {
            let distance = Number.POSITIVE_INFINITY;
            this.#result.matches.forEach((match, candidate) => {
                if (match.field === preferred.field && Math.abs(match.start - preferred.start) < distance) {
                    distance = Math.abs(match.start - preferred.start);
                    index = candidate;
                }
            });
        }
        this.#index = index >= 0 ? index : this.#result.matches.length ? 0 : -1;
        this.#state = { ...this.#state, active: this.#result.matches[this.#index] ?? null };
    }
}

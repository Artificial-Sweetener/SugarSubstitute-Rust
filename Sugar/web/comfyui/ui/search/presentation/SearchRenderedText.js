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
/** Highlight matches in text a card renders itself: titles, field names and shown values. */
import { findSearchRanges } from '../domain/SearchMatching.js';
/** The name the finder's stylesheet colours with `::highlight()`. */
export const SEARCH_TEXT_HIGHLIGHT = 'sc-search-match';
/**
 * Mark the characters that made a card or field match, without touching the card's own DOM.
 * - A Node query marks it in the titles and field names of the cards it kept.
 * - A Field query marks each of its words in the names of the fields it kept.
 * - Text being searched for is marked in the values a field shows as plain text, such as a
 *   chosen sampler or model. Prompts and text boxes paint their own matches.
 * A match in text that is not on screen, such as a node's type, has nothing to mark.
 */
export class SearchRenderedText {
    #document;
    /** Bind the document whose highlight registry holds the marks. */
    constructor(documentRef) {
        this.#document = documentRef;
    }
    /** Replace every mark with the ones this result calls for. */
    render(surface, result) {
        const view = this.#document.defaultView;
        const registry = view?.CSS?.highlights;
        if (!view?.Highlight || !registry)
            return;
        const { query } = result;
        const ranges = [];
        if (query.mode === 'node' && query.node)
            for (const node of result.nodes)
                this.#mark(ranges, surface.titles.get(node), [query.node]);
        for (const field of surface.fields.values()) {
            if (field.row.hidden || field.row.hasAttribute('data-sc-search-field-hidden'))
                continue;
            if (query.mode === 'node' && query.node && result.nodes.has(field.node))
                this.#mark(ranges, field.label, [query.node]);
            if (query.mode === 'field' && query.tokens.length && result.fields.has(field.id))
                this.#mark(ranges, field.label, query.tokens);
            if (query.text &&
                !field.control &&
                !field.painter &&
                (query.mode === 'text' || !query.node || result.nodes.has(field.node)))
                this.#mark(ranges, field.row, [query.text], field.label);
        }
        if (ranges.length)
            registry.set(SEARCH_TEXT_HIGHLIGHT, new view.Highlight(...ranges));
        else
            registry.delete(SEARCH_TEXT_HIGHLIGHT);
    }
    /** Remove every mark. */
    clear() {
        this.#document.defaultView?.CSS?.highlights?.delete(SEARCH_TEXT_HIGHLIGHT);
    }
    /** Collect each needle's occurrences in the text shown under `root`, outside `except`. */
    #mark(ranges, root, needles, except = null) {
        if (!root)
            return;
        const walker = this.#document.createTreeWalker(root, 4);
        for (let node = walker.nextNode(); node; node = walker.nextNode()) {
            const parent = node.parentElement;
            if (!parent || except?.contains(parent) || parent.closest('.sc-search-highlight'))
                continue;
            const text = node.nodeValue ?? '';
            for (const needle of needles)
                for (const found of findSearchRanges(text, needle)) {
                    const range = this.#document.createRange();
                    range.setStart(node, found.start);
                    range.setEnd(node, found.start + found.length);
                    ranges.push(range);
                }
        }
    }
}

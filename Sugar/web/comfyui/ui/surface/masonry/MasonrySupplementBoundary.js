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
/** Identify retained presentation cards for the shared graph-to-masonry transition. */
/** Pair an owned card with its retained native graph source without copying either presentation. */
export function identifyMasonrySupplement(card, source, id) {
    card.dataset.scMasonrySupplement = id;
    source.dataset.scMasonrySupplementSource = id;
}
/** Release the pairing when the owning graph or presentation mode ends. */
export function releaseMasonrySupplement(card, source) {
    delete card.dataset.scMasonrySupplement;
    delete source.dataset.scMasonrySupplementSource;
}
/** Include identified owned cards in the same transition list as projected node cells. */
export function masonrySupplementCards(surface) {
    return [...surface.querySelectorAll('[data-sc-masonry-supplement]')].map((cell) => ({
        id: cell.dataset.scMasonrySupplement ?? '',
        cell,
    }));
}
/** Resolve native graph frames whose controls are temporarily mounted in an owned masonry slot. */
export function masonrySupplementSources(root) {
    const sources = new Map();
    for (const source of root.querySelectorAll('[data-sc-masonry-supplement-source]')) {
        const node = source.closest('.lg-node');
        const id = source.dataset.scMasonrySupplementSource;
        if (node && id)
            sources.set(id, node);
    }
    return sources;
}

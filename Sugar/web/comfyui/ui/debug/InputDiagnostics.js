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
/**
 * Report what can keep the page from taking input, for diagnosing a page that seems frozen.
 *
 * Every SugarCubes input hold marks its surface, `inert` layers keep their attribute, and a
 * running masonry transition marks its surface; together with focus they name whatever is
 * refusing the user's clicks or keys.
 */
/** Name an element by tag and its first classes, without reading any of its content. */
function describe(element) {
    const classes = [...element.classList].slice(0, 3).join('.');
    return classes ? `${element.tagName.toLowerCase()}.${classes}` : element.tagName.toLowerCase();
}
/** Read the input state of `documentRef`. */
export function readInputDiagnostics(documentRef) {
    const blockers = [];
    const add = (selector, reason) => {
        for (const element of documentRef.querySelectorAll(selector))
            blockers.push({ element: describe(element), reason });
    };
    add('[data-sc-input-held]', 'held');
    add('[inert]', 'inert');
    add('[data-sc-masonry-transition]', 'transition');
    const active = documentRef.activeElement;
    return {
        hasFocus: documentRef.hasFocus(),
        visibility: documentRef.visibilityState,
        activeElement: active ? describe(active) : null,
        blockers,
    };
}

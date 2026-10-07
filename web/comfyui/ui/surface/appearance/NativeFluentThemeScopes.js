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
/** Share complete Fluent tokens at stable ancestors across native presentation moves. */
const THEME_ATTRIBUTE = 'data-sc-fluent-theme-root';
const OWNED_SCOPE_SELECTOR = '.sc-overrides-toolbar, .sugarcubes-workflow-masonry, .sugarcubes-cube-face';
/** Own token scope markers while card-specific styling remains with native annotations. */
export class NativeFluentThemeScopes {
    #roots = new Set();
    /** Keep graph and masonry under one token owner, with standalone owned surfaces as fallbacks. */
    retain(element) {
        const root = element.closest('#graph-canvas-container') ??
            element.closest(OWNED_SCOPE_SELECTOR) ??
            element;
        if (this.#roots.has(root))
            return;
        this.#roots.add(root);
        root.setAttribute(THEME_ATTRIBUTE, '');
    }
    /** Release detached surface scopes without retaining closed popups or retired graph roots. */
    prune() {
        for (const root of this.#roots) {
            if (root.isConnected)
                continue;
            this.#remove(root);
            this.#roots.delete(root);
        }
    }
    /** Restore host inheritance when the appearance preference is suspended or disabled. */
    clear() {
        for (const root of this.#roots)
            this.#remove(root);
        this.#roots.clear();
    }
    /** Remove only the exact marker still owned by this appearance lifetime. */
    #remove(root) {
        if (root.getAttribute(THEME_ATTRIBUTE) === '')
            root.removeAttribute(THEME_ATTRIBUTE);
    }
}

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
/** Project one live Nodes 2.0 Cube theme onto its nested native cards. */
import { deriveCubeBackdropColor } from './CubeNodeColorTheme.js';
/** Name the inherited token carrying the native Cube header color. */
export const CUBE_CARD_HEADER_TOKEN = '--sugarcubes-cube-card-header';
/** Name the inherited token carrying the native Cube body color. */
export const CUBE_CARD_BODY_TOKEN = '--sugarcubes-cube-card-body';
/** Name the token carrying the slightly darker enclosing Cube backdrop. */
export const CUBE_BACKDROP_TOKEN = '--sugarcubes-cube-backdrop';
const NATIVE_INNER_WRAPPER_SELECTOR = ':scope > [data-testid="node-inner-wrapper"]';
const NATIVE_BODY_TOKEN = '--component-node-background';
/** Track Comfy's reactive native node styles without changing graph color data. */
export class ComfyVueCubeColorScope {
    #root;
    #observer;
    #mounted = false;
    /** Bind the stable native node root whose inner wrapper owns live colors. */
    constructor(root) {
        this.#root = root;
        this.#observer = new MutationObserver((records) => {
            if (records.some((record) => this.#affectsNativeTheme(record)))
                this.refresh();
        });
    }
    /** Apply the current colors and begin tracking Comfy's reactive style updates. */
    mount() {
        if (this.#mounted)
            return;
        this.#mounted = true;
        this.refresh();
        this.#observer.observe(this.#root, {
            attributes: true,
            attributeFilter: ['style'],
            childList: true,
            subtree: true,
        });
    }
    /** Re-read the authoritative native wrapper into Cube-scoped CSS tokens. */
    refresh() {
        const wrapper = this.#root.querySelector(NATIVE_INNER_WRAPPER_SELECTOR);
        if (!wrapper) {
            this.#writeToken(CUBE_CARD_HEADER_TOKEN, '');
            this.#writeToken(CUBE_CARD_BODY_TOKEN, '');
            this.#writeToken(CUBE_BACKDROP_TOKEN, '');
            return;
        }
        const computed = getComputedStyle(wrapper);
        const header = wrapper.style.getPropertyValue('background-color').trim() || computed.backgroundColor.trim();
        const body = wrapper.style.getPropertyValue(NATIVE_BODY_TOKEN).trim() ||
            computed.getPropertyValue(NATIVE_BODY_TOKEN).trim();
        this.#writeToken(CUBE_CARD_HEADER_TOKEN, header);
        this.#writeToken(CUBE_CARD_BODY_TOKEN, body);
        this.#writeToken(CUBE_BACKDROP_TOKEN, body ? deriveCubeBackdropColor(body) : '');
    }
    /** Stop tracking and remove only the tokens owned by this scope. */
    dispose() {
        this.#observer.disconnect();
        this.#mounted = false;
        this.#root.style.removeProperty(CUBE_CARD_HEADER_TOKEN);
        this.#root.style.removeProperty(CUBE_CARD_BODY_TOKEN);
        this.#root.style.removeProperty(CUBE_BACKDROP_TOKEN);
    }
    /** Return whether one native mutation can replace or recolor the theme owner. */
    #affectsNativeTheme(record) {
        if (record.type === 'childList')
            return true;
        return (record.target instanceof HTMLElement &&
            record.target.parentElement === this.#root &&
            record.target.matches('[data-testid="node-inner-wrapper"]'));
    }
    /** Write one changed non-empty token without creating observer feedback. */
    #writeToken(name, value) {
        if (!value) {
            this.#root.style.removeProperty(name);
            return;
        }
        if (this.#root.style.getPropertyValue(name) !== value) {
            this.#root.style.setProperty(name, value);
        }
    }
}

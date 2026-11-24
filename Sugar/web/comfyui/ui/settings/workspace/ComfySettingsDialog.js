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
 * ComfyUI's settings dialog as the SugarSubstitute frame's workspace sees it. Setting pages come
 * from ComfyUI's setting store; ComfyUI's own panels (keybindings, extensions, about, account),
 * which no extension can draw, are its navigation entries keyed by `data-nav-id`, the key
 * ComfyUI itself uses to find them. Showing a panel selects ComfyUI's entry and lets ComfyUI's
 * content show through under the workspace.
 */
import { comfySettingsPages } from './ComfySettingsPages.js';
/** ComfyUI's panels in the order the workspace lists them; unknown panels follow. */
const PANEL_ORDER = [
    'keybinding',
    'extension',
    'server-config',
    'about',
    'user',
    'credits',
    'secrets',
];
/**
 * A panel's workspace id: ComfyUI's navigation id under `panel/`, as its setting pages are under
 * `root/`, so a panel never shares an id with a workspace page (both have an "about").
 */
const PANEL_PREFIX = 'panel/';
/** The dialog root's marker while the workspace presents it. */
export const HOST_ATTRIBUTE = 'data-sc-sw-host';
/** The dialog root's marker naming the ComfyUI panel that shows through. */
export const PANEL_ATTRIBUTE = 'data-sc-sw-panel';
/** One mounted settings dialog. */
export class ComfySettingsDialog {
    root;
    store;
    constructor(root, store) {
        this.root = root;
        this.store = store;
    }
    comfyPages() {
        // Page titles are ComfyUI's own, translated navigation labels where it shows one.
        return comfySettingsPages(this.store, ['SugarCubes'], (navId) => this.#icon(navId), (navId) => this.#item(navId)?.textContent?.trim() || undefined);
    }
    comfyPanels() {
        const panels = [...this.root.querySelectorAll('[data-nav-id]')]
            .map((item) => item.dataset.navId ?? '')
            .filter((id) => id && !id.startsWith('root/'));
        const rank = (id) => {
            const index = PANEL_ORDER.indexOf(id);
            return index < 0 ? PANEL_ORDER.length : index;
        };
        return [...new Set(panels)]
            .sort((a, b) => rank(a) - rank(b))
            .map((id) => {
            const icon = this.#icon(id);
            return { id: `${PANEL_PREFIX}${id}`, title: this.#label(id), ...(icon ? { icon } : {}) };
        });
    }
    showPanel(id) {
        if (id === null || !id.startsWith(PANEL_PREFIX)) {
            this.root.removeAttribute(PANEL_ATTRIBUTE);
            return;
        }
        const navId = id.slice(PANEL_PREFIX.length);
        if (this.root.getAttribute(PANEL_ATTRIBUTE) !== navId) {
            this.root.setAttribute(PANEL_ATTRIBUTE, navId);
            this.#item(navId)?.click();
        }
    }
    close() {
        // ComfyUI's dialog layout closes on Escape at its root.
        this.root.dispatchEvent(new KeyboardEvent('keydown', {
            key: 'Escape',
            code: 'Escape',
            bubbles: true,
            cancelable: true,
        }));
    }
    #item(id) {
        return ([...this.root.querySelectorAll('[data-nav-id]')].find((item) => item.dataset.navId === id) ?? null);
    }
    #label(id) {
        return this.#item(id)?.textContent?.trim() || id;
    }
    /** ComfyUI's own icon for an entry, drawn as a copy. */
    #icon(id) {
        const icon = this.#item(id)?.querySelector('i');
        if (!icon)
            return undefined;
        return () => {
            const copy = icon.cloneNode(true);
            return copy instanceof HTMLElement ? copy : icon.ownerDocument.createElement('span');
        };
    }
}

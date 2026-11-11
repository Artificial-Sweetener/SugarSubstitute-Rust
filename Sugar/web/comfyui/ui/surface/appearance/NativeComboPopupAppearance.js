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
/** Bind native dropdown presentation to its control relationship independently of input routing. */
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import { identifyMenuSurface, releaseMenuSurface } from '../../menus/MenuSurface.js';
import { NativeInlineComboSearch } from './NativeInlineComboSearch.js';
import { NativeThemeScopes } from './NativeThemeScopes.js';
const triggerSelector = '[role="combobox"]';
/** Own native popup association, theme scopes and search presentation for one appearance lifetime. */
export class NativeComboPopupAppearance {
    document;
    reportError;
    #triggers = new Set();
    #popups = new Map();
    #search;
    #presence = null;
    /** Keep host selection, filtering and keyboard behavior with Comfy's existing controls. */
    constructor(document, reportError) {
        this.document = document;
        this.reportError = reportError;
        this.#search = new NativeInlineComboSearch(document);
    }
    /** Follow identified native controls and portals without scheduling card or graph discovery. */
    enable() {
        if (this.#presence)
            return;
        this.#presence = new HostSurfaceObserver(this.document.body, `${triggerSelector}, [role="listbox"]`, (changes) => {
            try {
                this.#prune();
                for (const { element, kind } of changes) {
                    if (kind === 'removed')
                        continue;
                    if (element.matches(triggerSelector))
                        this.#retainTrigger(element);
                    else if (element.getAttribute('role') === 'listbox')
                        this.#presentPopup(element);
                }
            }
            catch (error) {
                this.clear();
                this.reportError(new Error('Native dropdown presentation failed.', { cause: error }));
            }
        }, { content: true, attributes: ['aria-controls', 'aria-expanded'] });
    }
    /** Register controls after their owning native surface has received its presentation annotations. */
    reconcile(root) {
        if (!this.#presence)
            return;
        this.#prune();
        if (root.matches(triggerSelector))
            this.#retainTrigger(root);
        for (const trigger of root.querySelectorAll(triggerSelector))
            this.#retainTrigger(trigger);
    }
    /** Restore native popup presentation and release all associations when the style is suspended. */
    clear() {
        this.#presence?.disconnect();
        this.#presence = null;
        this.#search.clear();
        for (const popup of this.#popups.keys())
            this.#releasePopup(popup);
        this.#triggers.clear();
    }
    /** Accept only controls inside identified native fields, excluding independent picker surfaces. */
    #owns(trigger) {
        return (trigger.isConnected &&
            trigger.matches(triggerSelector) &&
            Boolean(trigger.getAttribute('aria-controls')) &&
            !trigger.closest('.sc-model-field, .sc-model-popup, [role="listbox"]') &&
            trigger.closest('[data-sc-style-row], .sugarcubes-native-widget') !== null &&
            trigger.closest('[data-sc-style-node], [data-sc-style-surface]') !== null);
    }
    /** Resolve the host's exact accessibility relationship even when the popup already exists. */
    #retainTrigger(trigger) {
        if (!this.#owns(trigger))
            return;
        this.#triggers.add(trigger);
        const id = trigger.getAttribute('aria-controls');
        const popup = id ? this.document.getElementById(id) : null;
        if (popup)
            this.#presentPopup(popup);
    }
    /** Apply one presentation owner to a positively associated native listbox before its next paint. */
    #presentPopup(popup) {
        if (!popup.isConnected ||
            !popup.id ||
            popup.getAttribute('role') !== 'listbox' ||
            popup.closest('.sc-model-popup'))
            return;
        const trigger = [...this.#triggers].find((candidate) => this.#owns(candidate) && candidate.getAttribute('aria-controls') === popup.id);
        if (!trigger)
            return;
        popup.setAttribute('data-sc-style-popup', '');
        identifyMenuSurface(popup, 'choices');
        if (this.#popups.get(popup)?.trigger !== trigger) {
            this.#popups.get(popup)?.theme.clear();
            const theme = new NativeThemeScopes();
            theme.retain(popup);
            this.#popups.set(popup, { trigger, theme });
        }
        this.#search.present(trigger, popup);
    }
    /** Retire detached controls and replaced relationships without retaining obsolete native trees. */
    #prune() {
        for (const trigger of this.#triggers)
            if (!this.#owns(trigger))
                this.#triggers.delete(trigger);
        for (const [popup, { trigger }] of this.#popups)
            if (!popup.isConnected ||
                !this.#triggers.has(trigger) ||
                trigger.getAttribute('aria-controls') !== popup.id)
                this.#releasePopup(popup);
        this.#search.reconcile();
    }
    /** Remove only the menu annotations installed by this native popup owner. */
    #releasePopup(popup) {
        this.#search.release(popup);
        this.#popups.get(popup)?.theme.clear();
        popup.removeAttribute('data-sc-style-popup');
        releaseMenuSurface(popup);
        this.#popups.delete(popup);
    }
}

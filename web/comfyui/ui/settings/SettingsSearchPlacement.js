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
/** Project Comfy's existing search and close controls into a reversible top toolbar. */
import { SettingsSearchNavigation } from './SettingsSearchNavigation.js';
/** Keep native search controls stable while the settings shell changes presentation. */
export class SettingsSearchPlacement {
    #bar;
    #moves = [];
    #navigation;
    /** Preserve native input, popup, focus, keyboard and event-handler identity. */
    constructor(root) {
        const document = root.ownerDocument;
        this.#bar = document.createElement('header');
        this.#bar.className = 'sc-settings-search-bar sc-settings-owned';
        const title = document.createElement('h2');
        title.textContent = root.querySelector('header h2')?.textContent ?? 'Settings';
        this.#bar.append(title);
        root.prepend(this.#bar);
        const sidebar = root.querySelector('nav');
        const search = sidebar?.querySelector('input[placeholder]');
        this.#navigation = search ? new SettingsSearchNavigation(root, search) : null;
        let wrapper = search ?? null;
        while (wrapper?.parentElement && wrapper.parentElement !== sidebar)
            wrapper = wrapper.parentElement;
        const close = root.querySelector('button[aria-label="Close dialog"], :scope > div > div > header button');
        for (const element of [wrapper, close]) {
            if (!element?.parentNode)
                continue;
            const slot = document.createComment('SugarCubes settings presentation slot');
            element.before(slot);
            this.#moves.push({ element, slot });
            this.#relocate(element, this.#bar, null);
        }
    }
    /** Restore native slots even when a palette or mode changes with the dialog open. */
    dispose() {
        this.#navigation?.dispose();
        for (const { element, slot } of this.#moves) {
            if (slot.parentElement) {
                this.#relocate(element, slot.parentElement, slot);
                slot.remove();
            }
        }
        this.#bar.remove();
    }
    /** Reconcile native search completion without taking ownership of its result filtering. */
    reconcile() {
        return this.#navigation?.reconcile() ?? false;
    }
    /** Preserve active search and popup state during connected native presentation moves. */
    #relocate(element, target, before) {
        const focused = element.ownerDocument.activeElement;
        const move = Reflect.get(target, 'moveBefore');
        if (typeof move === 'function' && target.isConnected && element.isConnected)
            move.call(target, element, before);
        else
            target.insertBefore(element, before);
        if (focused instanceof HTMLElement &&
            element.contains(focused) &&
            focused.ownerDocument.activeElement !== focused)
            focused.focus({ preventScroll: true });
    }
}

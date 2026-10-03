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
/** Own the icon-only view affordance and its position in Comfy's action bar. */
export class WorkflowMasonryToolbarButton {
    #document;
    #button;
    #icon;
    #presence;
    #toggle;
    #disposed = false;
    /** Create one accessible button and reuse it across native toolbar replacements. */
    constructor(documentRef, toggle) {
        this.#document = documentRef;
        this.#toggle = toggle;
        this.#button = documentRef.createElement('button');
        this.#button.className = 'sugarcubes-masonry-toggle';
        this.#button.type = 'button';
        this.#button.setAttribute('aria-label', 'Toggle workflow masonry');
        this.#icon = documentRef.createElement('span');
        this.#icon.className = 'sugarcubes-masonry-toggle__icon';
        this.#icon.setAttribute('aria-hidden', 'true');
        this.#button.append(this.#icon);
        this.#button.addEventListener('click', toggle);
        this.#presence = new MutationObserver(() => this.#attach());
        this.update(false, false);
    }
    /** Observe native toolbar mounts after Comfy setup. */
    setup() {
        if (this.#disposed)
            return;
        this.#presence.observe(this.#document.body, { childList: true, subtree: true });
        this.#attach();
    }
    /** Show the destination view while exposing current state to assistive technology. */
    update(active, available) {
        const destination = active ? 'graph' : 'masonry';
        if (this.#button.dataset.view !== destination) {
            this.#button.dataset.view = destination;
            const filename = active ? 'flowchart.svg' : 'board.svg';
            const url = new URL(`../../../../icons/view/${filename}`, import.meta.url);
            this.#icon.style.setProperty('--sc-view-icon', `url("${url.href}")`);
        }
        this.#button.setAttribute('aria-pressed', String(active));
        this.#button.disabled = !available;
        this.#button.title = available ? `Switch to ${destination} view` : 'Masonry requires Nodes 2.0';
    }
    /** Release the toolbar observer and click handler with the owning host. */
    dispose() {
        this.#disposed = true;
        this.#presence.disconnect();
        this.#button.removeEventListener('click', this.#toggle);
        this.#button.remove();
    }
    /** Insert after the native queue slot without moving host DOM or depending on panel state. */
    #attach() {
        if (this.#disposed)
            return;
        const bar = this.#document.querySelector('.actionbar-container');
        const queue = bar?.querySelector('[data-testid="queue-overlay-toggle"]');
        let slot = queue?.parentElement ?? null;
        while (slot && slot.parentElement !== bar)
            slot = slot.parentElement;
        if (!bar || !queue || !slot) {
            this.#button.remove();
            return;
        }
        if (this.#button.previousElementSibling !== slot || this.#button.parentElement !== bar)
            bar.insertBefore(this.#button, slot.nextSibling);
    }
}

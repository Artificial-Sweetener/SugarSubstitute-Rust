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
/** Align the actual Comfy queue trigger with the generation cluster while retaining its native menu and DOM ownership. */
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
/** Keep native queue behavior intact while presenting source-matched geometry, artwork and count. */
export class ComfyGenerationQueueSurface {
    document;
    slot;
    #presence;
    #size;
    #glyph;
    #badge;
    #button = null;
    #label = null;
    #dark = true;
    #frameRequest = null;
    #disposed = false;
    /** Observe only the native queue trigger and accepted frame geometry, excluding owned count painting. */
    constructor(document, frame, slot) {
        this.document = document;
        this.slot = slot;
        this.#glyph = document.createElement('span');
        this.#glyph.className = 'sc-generation-queue-glyph';
        this.#glyph.setAttribute('aria-hidden', 'true');
        this.#glyph.style.setProperty('--sc-queue-glyph', `url("${new URL('../../../../orb/history.svg', import.meta.url).href}")`);
        this.#badge = document.createElement('span');
        this.#badge.className = 'sc-generation-queue-badge';
        this.#badge.setAttribute('aria-hidden', 'true');
        this.#presence = new HostSurfaceObserver(document.body, '[data-testid="queue-overlay-toggle"]', () => this.#reconcile(), {
            content: true,
            characterData: true,
            attributes: ['disabled', 'aria-pressed'],
            exclude: '.sc-generation-queue-glyph,.sc-generation-queue-badge',
        });
        const Observer = document.defaultView?.ResizeObserver;
        this.#size = Observer ? new Observer(this.#schedule) : null;
        this.#size?.observe(frame);
        this.#size?.observe(slot);
        document.defaultView?.addEventListener('resize', this.#schedule);
        this.#reconcile();
    }
    /** Reflect the same accepted theme as the adjacent generation segments. */
    setPalette(dark) {
        this.#dark = dark;
        if (this.#button)
            this.#button.dataset.scQueueTheme = dark ? 'dark' : 'light';
    }
    /** Reposition the native queue anchor after its owned generation slot changes parents. */
    refreshPosition() {
        this.#schedule();
    }
    /** Restore the native trigger's original accessible name and remove all presentation hooks. */
    dispose() {
        this.#disposed = true;
        this.#presence.disconnect();
        this.#size?.disconnect();
        this.document.defaultView?.removeEventListener('resize', this.#schedule);
        if (this.#frameRequest !== null)
            this.document.defaultView?.cancelAnimationFrame(this.#frameRequest);
        this.#release();
    }
    /** Retain the actual trigger and popover anchor through native component replacement. */
    #reconcile() {
        if (this.#disposed)
            return;
        const button = this.document.querySelector('button[data-testid="queue-overlay-toggle"]');
        if (button !== this.#button) {
            this.#release();
            this.#button = button;
            if (button) {
                this.#label = button.getAttribute('aria-label');
                button.setAttribute('data-sc-generation-queue', '');
                button.setAttribute('aria-label', 'Generation queue');
                button.append(this.#glyph, this.#badge);
                this.setPalette(this.#dark);
            }
        }
        if (button && (this.#glyph.parentElement !== button || this.#badge.parentElement !== button))
            button.append(this.#glyph, this.#badge);
        const text = [...(button?.childNodes ?? [])]
            .filter((node) => node !== this.#glyph && node !== this.#badge)
            .map((node) => node.textContent ?? '')
            .join(' ');
        const count = /^\s*(\d+)/.exec(text)?.[1] ?? '0';
        if (this.#badge.textContent !== count)
            this.#badge.textContent = count;
        this.#badge.hidden = count === '0';
        this.#position();
    }
    /** Coalesce settled geometry changes independently of previews, graph activity or popup mounts. */
    #schedule = () => {
        if (this.#disposed || this.#frameRequest !== null)
            return;
        this.#frameRequest =
            this.document.defaultView?.requestAnimationFrame(() => {
                this.#frameRequest = null;
                this.#position();
            }) ?? null;
    };
    /** Align the native popover anchor with its reserved segment without reparenting Vue nodes. */
    #position() {
        if (!this.#button || !this.slot.isConnected)
            return;
        const bounds = this.slot.getBoundingClientRect();
        for (const [name, value] of [
            ['--sc-queue-left', `${bounds.left}px`],
            ['--sc-queue-top', `${bounds.top}px`],
        ])
            if (this.#button.style.getPropertyValue(name) !== value)
                this.#button.style.setProperty(name, value);
    }
    /** Retire only owned adornments and leave native listeners, children and queue state in place. */
    #release() {
        this.#glyph.remove();
        this.#badge.remove();
        if (this.#button) {
            this.#button.removeAttribute('data-sc-generation-queue');
            this.#button.removeAttribute('data-sc-queue-theme');
            this.#button.style.removeProperty('--sc-queue-left');
            this.#button.style.removeProperty('--sc-queue-top');
            if (this.#label === null)
                this.#button.removeAttribute('aria-label');
            else
                this.#button.setAttribute('aria-label', this.#label);
        }
        this.#button = null;
        this.#label = null;
    }
}

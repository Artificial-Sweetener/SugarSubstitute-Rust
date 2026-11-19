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
/** Preview the workflow tab that something dragged over the tab bar would open. */
import { fixedOverlayOrigin } from '../../interaction/reorder/ReorderOverlayOrigin.js';
import { slotOffsets } from '../../interaction/reorder/ReorderSlots.js';
import { WORKFLOW_TAB_WIDTH } from './WorkflowTabStyles.js';
/** How far above or below the tab bar the pointer may be and still be over it. */
const REACH = 6;
/**
 * Draw a tab that does not exist yet: the title it would have inside a dashed outline, with the
 * open tabs moved aside to show where it would sit. Nothing about the open workflows changes.
 */
export class WorkflowTabGhost {
    document;
    viewport;
    bar;
    tabs;
    widths;
    #ghost = null;
    #shifted = [];
    constructor(document, 
    /** The scrolling strip that holds the tabs. */
    viewport, 
    /** The bar the strip sits in: a drop anywhere along it counts. */
    bar, tabs, widths) {
        this.document = document;
        this.viewport = viewport;
        this.bar = bar;
        this.tabs = tabs;
        this.widths = widths;
    }
    /** The position a tab dropped at this point would take, or null when it is off the bar. */
    hit(x, y) {
        const bar = this.bar.getBoundingClientRect();
        const strip = this.viewport.getBoundingClientRect();
        // The strip gives the bar's height; the bar gives its length, past the last tab.
        if (y < strip.top - REACH || y > strip.bottom + REACH || x < bar.left || x > bar.right)
            return null;
        const widths = this.widths();
        const starts = slotOffsets(widths);
        const along = x - strip.left + this.viewport.scrollLeft;
        // A tab is passed once the pointer is beyond its middle.
        const index = widths.findIndex((width, tab) => along < (starts[tab] ?? 0) + width / 2);
        return index < 0 ? widths.length : index;
    }
    /** Show the ghost at `index`, or withdraw it when `title` is null. */
    show(title, index) {
        for (const tab of this.#shifted)
            tab.style.translate = '';
        this.#shifted = [];
        if (title === null) {
            this.#ghost?.remove();
            this.#ghost = null;
            return;
        }
        const widths = this.widths();
        const width = widths.at(-1) ?? WORKFLOW_TAB_WIDTH;
        const ghost = this.#ghost ?? this.#create();
        const strip = this.viewport.getBoundingClientRect();
        const origin = fixedOverlayOrigin(this.bar);
        const start = (slotOffsets(widths)[index] ?? 0) - this.viewport.scrollLeft;
        ghost.textContent = title;
        ghost.title = title;
        ghost.style.width = `${String(width)}px`;
        ghost.style.height = `${String(strip.height)}px`;
        ghost.style.left = `${String(strip.left + start - origin.x)}px`;
        ghost.style.top = `${String(strip.top - origin.y)}px`;
        this.#shifted = this.tabs().slice(index);
        for (const tab of this.#shifted)
            tab.style.translate = `${String(width)}px 0`;
    }
    /** Remove the ghost and put every tab back. */
    dispose() {
        this.show(null, 0);
    }
    #create() {
        const ghost = this.document.createElement('div');
        ghost.className = 'sc-workflow-tab-ghost';
        ghost.setAttribute('aria-hidden', 'true');
        this.bar.append(ghost);
        this.#ghost = ghost;
        return ghost;
    }
}

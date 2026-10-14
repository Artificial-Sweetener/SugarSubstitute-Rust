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
/** Own responsive tab-strip allocation and the lifetime of its overflow affordance. */
import { WORKFLOW_TAB_WIDTH } from './WorkflowTabStyles.js';
/** Preserve the reference orb's tab-strip origin. */
export const FRAME_TAB_START = 48;
/** Match Substitute's tab tool button width. */
export const WORKFLOW_TAB_TOOL_WIDTH = 32;
/** Keep the reference item-layout margin and add-button spacer after the last tab. */
export const WORKFLOW_NEW_BUTTON_GAP = 8;
/** Shrink the complete scrolling strip and expose overflow without reserving an empty toolbar. */
export class WorkflowTabStripLayout {
    frame;
    viewport;
    tabs;
    overflow;
    resized;
    closeOverflow;
    #window;
    #observer;
    #count = -1;
    #contentWidth = 0;
    #animationFrame = null;
    #disposed = false;
    /** Observe the enclosing frame so container resizing and tab-count changes share one policy. */
    constructor(document, frame, viewport, tabs, overflow, resized, closeOverflow) {
        this.frame = frame;
        this.viewport = viewport;
        this.tabs = tabs;
        this.overflow = overflow;
        this.resized = resized;
        this.closeOverflow = closeOverflow;
        const window = document.defaultView;
        if (!window)
            throw new Error('Workflow strip layout requires a browser window.');
        this.#window = window;
        this.overflow.hidden = true;
        this.#observer =
            typeof window.ResizeObserver === 'function' ? new window.ResizeObserver(this.#request) : null;
        this.#observer?.observe(frame);
        if (!this.#observer)
            window.addEventListener('resize', this.#request);
    }
    /** Allocate natural tab and add-button width only when the collection length changes. */
    update(count) {
        if (this.#disposed || this.#count === count)
            return;
        this.#count = count;
        const tabWidth = count * WORKFLOW_TAB_WIDTH;
        this.#contentWidth = tabWidth + WORKFLOW_NEW_BUTTON_GAP + WORKFLOW_TAB_TOOL_WIDTH;
        this.tabs.style.setProperty('--sc-workflow-tabs-width', `${String(tabWidth)}px`);
        this.viewport.style.width = `${String(this.#contentWidth)}px`;
        this.#refresh();
    }
    /** Disconnect geometry observation and cancel pending work before the controls are removed. */
    dispose() {
        this.#disposed = true;
        this.#observer?.disconnect();
        this.#window.removeEventListener('resize', this.#request);
        if (this.#animationFrame !== null)
            this.#window.cancelAnimationFrame(this.#animationFrame);
        this.#animationFrame = null;
    }
    /** Coalesce browser resize deliveries without measuring on scroll or presentation refresh. */
    #request = () => {
        if (this.#disposed || this.#animationFrame !== null)
            return;
        this.#animationFrame = this.#window.requestAnimationFrame(() => {
            this.#animationFrame = null;
            this.#refresh();
        });
    };
    /** Decide overflow against the whole available row to avoid hide/show feedback at the threshold. */
    #refresh() {
        if (this.#disposed || this.#count < 0)
            return;
        const available = Math.max(0, this.frame.clientWidth - FRAME_TAB_START);
        const hidden = this.#count === 0 || this.#contentWidth <= available + 0.5;
        if (this.overflow.hidden !== hidden) {
            this.overflow.hidden = hidden;
            if (hidden)
                this.closeOverflow();
        }
        this.resized();
    }
}

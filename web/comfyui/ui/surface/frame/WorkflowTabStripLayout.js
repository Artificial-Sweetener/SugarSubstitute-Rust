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
import { WORKFLOW_TAB_WIDTH, WORKFLOW_TAB_MIN_WIDTH } from './WorkflowTabStyles.js';
/** Preserve the reference orb's tab-strip origin. */
export const FRAME_TAB_START = 48;
/** Match Substitute's tab tool button width. */
export const WORKFLOW_TAB_TOOL_WIDTH = 32;
/** Keep the fixed add action close to the end of the visible tabs. */
export const WORKFLOW_NEW_BUTTON_GAP = 3;
/** Shrink the complete scrolling strip and expose overflow without reserving an empty toolbar. */
export class WorkflowTabStripLayout {
    frame;
    viewport;
    tabs;
    controls;
    resized;
    closeOverflow;
    repositioned;
    #window;
    #observer;
    #count = -1;
    #tabWidth = WORKFLOW_TAB_WIDTH;
    #available = 0;
    #animationFrame = null;
    #disposed = false;
    #leadingInset = FRAME_TAB_START;
    /** Observe the enclosing frame so container resizing and tab-count changes share one policy. */
    constructor(document, frame, viewport, tabs, controls, resized, closeOverflow, repositioned) {
        this.frame = frame;
        this.viewport = viewport;
        this.tabs = tabs;
        this.controls = controls;
        this.resized = resized;
        this.closeOverflow = closeOverflow;
        this.repositioned = repositioned;
        const window = document.defaultView;
        if (!window)
            throw new Error('Workflow strip layout requires a browser window.');
        this.#window = window;
        for (const control of Object.values(controls))
            control.hidden = true;
        controls.previous.addEventListener('click', this.#previous);
        controls.next.addEventListener('click', this.#next);
        viewport.addEventListener('scroll', this.#updateArrows);
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
        this.#refresh();
    }
    /** Allocate the leading edge from the actual application-menu presentation. */
    setLeadingInset(inset) {
        if (this.#leadingInset === inset)
            return;
        this.#leadingInset = inset;
        this.#refresh();
    }
    /** Disconnect geometry observation and cancel pending work before the controls are removed. */
    dispose() {
        this.#disposed = true;
        this.#observer?.disconnect();
        this.controls.previous.removeEventListener('click', this.#previous);
        this.controls.next.removeEventListener('click', this.#next);
        this.viewport.removeEventListener('scroll', this.#updateArrows);
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
        const trailing = Number.parseFloat(this.#window.getComputedStyle(this.frame).paddingRight) || 0;
        const available = (this.#available = Math.max(0, this.frame.clientWidth -
            this.#leadingInset -
            trailing -
            WORKFLOW_NEW_BUTTON_GAP -
            WORKFLOW_TAB_TOOL_WIDTH));
        const hidden = this.#count === 0 || this.#count * WORKFLOW_TAB_MIN_WIDTH <= available + 0.5;
        this.viewport.toggleAttribute('data-sc-overflow', !hidden);
        const arrows = Math.max(1, Number(!this.controls.previous.hidden) + Number(!this.controls.next.hidden));
        const capacity = Math.max(0, available - (hidden ? 0 : (1 + arrows) * WORKFLOW_TAB_TOOL_WIDTH));
        this.#tabWidth =
            this.#count === 0
                ? WORKFLOW_TAB_WIDTH
                : Math.max(WORKFLOW_TAB_MIN_WIDTH, Math.min(WORKFLOW_TAB_WIDTH, capacity / this.#count));
        const content = this.#count * this.#tabWidth;
        this.tabs.style.setProperty('--sc-workflow-tab-width', `${String(this.#tabWidth)}px`);
        this.tabs.style.setProperty('--sc-workflow-tabs-width', `${String(content)}px`);
        this.viewport.style.width = `${String(Math.min(content, capacity))}px`;
        if (this.controls.overflow.hidden !== hidden) {
            this.controls.overflow.hidden = hidden;
            if (hidden)
                this.closeOverflow();
        }
        this.resized(this.#tabWidth);
        this.#updateArrows();
    }
    /** Reflect actual scroll endpoints after wheel input, selection reveal or dragging. */
    #updateArrows = () => {
        if (this.#disposed)
            return;
        if (this.controls.overflow.hidden) {
            this.controls.previous.hidden = true;
            this.controls.next.hidden = true;
            return;
        }
        const maximum = Math.max(0, this.viewport.scrollWidth - this.viewport.clientWidth);
        const previous = this.viewport.scrollLeft > 0.5;
        const next = this.viewport.scrollLeft < maximum - 0.5;
        const changed = this.controls.previous.hidden === previous || this.controls.next.hidden === next;
        this.controls.previous.hidden = !previous;
        this.controls.next.hidden = !next;
        const capacity = Math.max(0, this.#available - (1 + Number(previous) + Number(next)) * WORKFLOW_TAB_TOOL_WIDTH);
        const width = Math.min(this.#count * this.#tabWidth, capacity);
        const cssWidth = `${String(width)}px`;
        const resized = this.viewport.style.width !== cssWidth;
        if (resized)
            this.viewport.style.width = cssWidth;
        const position = Math.max(0, Math.min(this.viewport.scrollLeft, this.#count * this.#tabWidth - width));
        if (this.viewport.scrollLeft !== position)
            this.viewport.scrollLeft = position;
        if (changed || resized)
            this.repositioned();
    };
    /** Move one readable tab at a time without overshooting either end. */
    #scroll(direction) {
        const maximum = Math.max(0, this.viewport.scrollWidth - this.viewport.clientWidth);
        this.viewport.scrollLeft = Math.max(0, Math.min(maximum, this.viewport.scrollLeft + direction * this.#tabWidth));
        this.#updateArrows();
    }
    /** Reach earlier workflows using the fixed leading arrow. */
    #previous = () => this.#scroll(-1);
    /** Reach later workflows using the fixed trailing arrow. */
    #next = () => this.#scroll(1);
}

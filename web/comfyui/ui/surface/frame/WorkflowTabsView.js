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
/** Present stable Microsoft Fluent workflow tabs while Comfy retains workflow actions. */
import { createWorkflowTab, createWorkflowTablist } from '../appearance/FluentThemeRuntime.js';
import { ContextMenu } from '../../menus/ContextMenu.js';
import { WORKFLOW_TAB_WIDTH } from './WorkflowTabStyles.js';
/** Retain keyed controls, literal titles, native keyboard navigation and one-shot activity animation. */
export class WorkflowTabsView {
    document;
    actions;
    viewport;
    element = createWorkflowTablist();
    #tabs = new Map();
    #menu;
    #order = [];
    #dragged = null;
    #projecting = false;
    #selection = '';
    /** Bind interaction intents without retaining host workflow objects in presentation. */
    constructor(document, actions, viewport) {
        this.document = document;
        this.actions = actions;
        this.viewport = viewport;
        this.#menu = new ContextMenu(document);
        this.element.setAttribute('aria-label', 'Workflows');
        this.viewport.addEventListener('scroll', this.#scrolled);
        this.element.addEventListener('change', () => {
            if (this.#projecting)
                return;
            const selected = [...this.#tabs].find(([, tab]) => tab.element.id === this.element.activeid);
            if (selected && selected[0] !== this.#order.find((tab) => tab.selected)?.id)
                this.#run(() => actions.activate(selected[0]));
        });
        this.viewport.addEventListener('wheel', this.#wheel, { passive: false });
    }
    /** Update values in place and animate only a newly accepted unread transition. */
    update(tabs, unread, animate = true) {
        this.#projecting = true;
        this.#order = tabs;
        const open = new Set(tabs.map((tab) => tab.id));
        for (const [id, view] of this.#tabs)
            if (!open.has(id)) {
                view.element.remove();
                this.#tabs.delete(id);
            }
        for (const [index, tab] of tabs.entries()) {
            let view = this.#tabs.get(tab.id);
            if (!view) {
                view = this.#create(tab.id);
                this.#tabs.set(tab.id, view);
            }
            if (view.title.textContent !== tab.title)
                view.title.textContent = tab.title;
            view.element.title = tab.title;
            view.element.toggleAttribute('data-modified', tab.modified);
            const nextUnread = unread(tab.id) && !tab.selected;
            view.element.toggleAttribute('data-unread', nextUnread);
            if (nextUnread && !view.unread && animate)
                view.element.setAttribute('data-shimmer', '');
            if (!nextUnread)
                view.element.removeAttribute('data-shimmer');
            view.unread = nextUnread;
            const current = this.element.children.item(index);
            if (current !== view.element)
                this.element.insertBefore(view.element, current);
            if (tab.selected)
                this.element.activeid = view.element.id;
        }
        this.#projecting = false;
        const selected = tabs.findIndex((tab) => tab.selected);
        const selection = `${tabs[selected]?.id ?? ''}:${String(selected)}`;
        if (selection !== this.#selection) {
            this.#selection = selection;
            this.#revealSelected(selected);
        }
        this.#updateOrbAdjacency();
    }
    /** Reveal newly selected or reordered tabs without changing scroll position on activity refresh. */
    #revealSelected(index) {
        const width = this.viewport.clientWidth;
        if (index < 0 || width <= 0)
            return;
        const left = index * WORKFLOW_TAB_WIDTH;
        const right = left + WORKFLOW_TAB_WIDTH;
        if (left < this.viewport.scrollLeft)
            this.viewport.scrollLeft = left;
        else if (right > this.viewport.scrollLeft + width)
            this.viewport.scrollLeft = Math.max(0, right - width);
    }
    /** Keep selection and the orb cutout aligned after the enclosing strip changes width. */
    resize() {
        this.#revealSelected(this.#order.findIndex((tab) => tab.selected));
        this.#updateOrbAdjacency();
    }
    /** Project scroll position without rebuilding controls or changing selected workflow policy. */
    #scrolled = () => this.#updateOrbAdjacency();
    /** Preserve reference horizontal wheel scrolling across both tabs and the trailing add button. */
    #wheel = (event) => {
        if (this.viewport.scrollWidth <= this.viewport.clientWidth)
            return;
        event.preventDefault();
        this.viewport.scrollLeft += event.deltaY || event.deltaX;
    };
    /** Keep the cutout aligned to the orb when the uniform-width tab strip scrolls or reorders. */
    #updateOrbAdjacency() {
        const selected = this.#order.findIndex((tab) => tab.selected);
        this.element.toggleAttribute('data-selected', selected >= 0);
        this.element.style.setProperty('--sc-selected-offset', `${String(selected * WORKFLOW_TAB_WIDTH)}px`);
        this.element.style.setProperty('--sc-tab-scroll', `${String(this.viewport.scrollLeft)}px`);
        const index = Math.floor(Math.max(0, this.viewport.scrollLeft) / WORKFLOW_TAB_WIDTH);
        const adjacent = this.#order[index]?.id;
        for (const [id, tab] of this.#tabs) {
            tab.element.toggleAttribute('data-orb-adjacent', id === adjacent);
            if (id === adjacent)
                tab.element.style.setProperty('--sc-tab-cutout-x', `${String(-17 + this.viewport.scrollLeft - index * WORKFLOW_TAB_WIDTH)}px`);
        }
    }
    /** Release floating interaction and official Fluent focus bindings through normal disconnection. */
    dispose() {
        this.#menu.close();
        this.viewport.removeEventListener('scroll', this.#scrolled);
        this.viewport.removeEventListener('wheel', this.#wheel);
        this.element.remove();
        this.#tabs.clear();
    }
    /** Create one official tab with a reserved close slot and literal title content. */
    #create(id) {
        const element = createWorkflowTab(this.document);
        element.id = `sc-workflow-${id}`;
        element.draggable = true;
        const title = this.document.createElement('span');
        title.className = 'sc-workflow-title';
        const shimmer = this.document.createElement('span');
        shimmer.className = 'sc-workflow-shimmer';
        shimmer.slot = 'start';
        shimmer.setAttribute('aria-hidden', 'true');
        const close = this.document.createElement('fluent-button');
        close.className = 'sc-workflow-close';
        close.slot = 'end';
        close.textContent = '×';
        close.setAttribute('appearance', 'subtle');
        close.setAttribute('aria-label', 'Close workflow');
        close.addEventListener('click', (event) => {
            event.stopPropagation();
            this.#run(() => this.actions.close(id));
        });
        close.addEventListener('focusin', (event) => event.stopPropagation());
        element.addEventListener('auxclick', (event) => {
            if (event.button === 1) {
                event.preventDefault();
                this.#run(() => this.actions.close(id));
            }
        });
        element.addEventListener('contextmenu', (event) => {
            event.preventDefault();
            this.#menu.open({
                label: 'Workflow actions',
                trigger: element,
                anchor: { kind: 'pointer', x: event.clientX, y: event.clientY },
                entries: [{ label: 'Close tab', select: () => this.#run(() => this.actions.close(id)) }],
            });
        });
        element.addEventListener('dragstart', (event) => {
            this.#dragged = id;
            event.dataTransfer?.setData('text/plain', id);
            if (event.dataTransfer)
                event.dataTransfer.effectAllowed = 'move';
        });
        element.addEventListener('dragover', (event) => {
            if (this.#dragged)
                event.preventDefault();
        });
        element.addEventListener('drop', (event) => {
            event.preventDefault();
            const from = this.#order.findIndex((tab) => tab.id === this.#dragged);
            const to = this.#order.findIndex((tab) => tab.id === id);
            if (from >= 0 && to >= 0 && from !== to)
                this.#run(() => this.actions.reorder(from, to));
            this.#dragged = null;
        });
        element.addEventListener('dragend', () => {
            this.#dragged = null;
        });
        element.addEventListener('keydown', (event) => {
            if (event.altKey && ['ArrowLeft', 'ArrowRight'].includes(event.key)) {
                event.preventDefault();
                event.stopPropagation();
                const from = this.#order.findIndex((tab) => tab.id === id);
                const to = from + (event.key === 'ArrowLeft' ? -1 : 1);
                if (to >= 0 && to < this.#order.length)
                    this.#run(() => this.actions.reorder(from, to));
            }
        });
        element.addEventListener('animationend', () => element.removeAttribute('data-shimmer'));
        element.addEventListener('animationcancel', () => element.removeAttribute('data-shimmer'));
        element.append(shimmer, title, close);
        return { element, title, close, unread: false };
    }
    /** Report host action failure without resetting any workflow or canvas state. */
    #run(action) {
        try {
            action();
        }
        catch (error) {
            this.actions.report(error);
        }
    }
}

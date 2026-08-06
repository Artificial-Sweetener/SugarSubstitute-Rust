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
/** Adapt Comfy breadcrumb and inspector chrome at the root of a Cube editor. */
import { requireCubeIdentity } from '../cube/node/ComfyCubeNodeFactory.js';
/** Own DOM presentation and event interception for Cube editor workspace chrome. */
export class ComfyCubeEditorChromeAdapter {
    #document;
    #canvas;
    #contexts;
    #policy;
    #controller;
    #observer;
    #handleClick;
    #handleDoubleClick;
    #handleKeydown;
    #labelStates = new Map();
    #textStates = new Map();
    #hiddenStates = new Map();
    #scheduled = false;
    #contextKey = null;
    /** Bind the stable Comfy chrome anchors and Cube intent controller. */
    constructor(options) {
        this.#document = options.document;
        this.#canvas = options.canvas;
        this.#contexts = options.contexts;
        this.#policy = options.policy;
        this.#controller = options.controller;
        this.#observer = new MutationObserver(() => this.#schedule());
        this.#handleClick = (event) => this.#onClick(event);
        this.#handleDoubleClick = (event) => this.#onDoubleClick(event);
        this.#handleKeydown = (event) => this.#onKeydown(event);
    }
    /** Observe host remounts and intercept only actions at the active Cube root. */
    install() {
        this.#observer.observe(this.#document.body, { childList: true, subtree: true });
        this.#document.addEventListener('click', this.#handleClick, true);
        this.#document.addEventListener('dblclick', this.#handleDoubleClick, true);
        this.#document.addEventListener('keydown', this.#handleKeydown, true);
        this.refresh();
    }
    /** Release DOM observers and event capture. */
    dispose() {
        this.#observer.disconnect();
        this.#document.removeEventListener('click', this.#handleClick, true);
        this.#document.removeEventListener('dblclick', this.#handleDoubleClick, true);
        this.#document.removeEventListener('keydown', this.#handleKeydown, true);
        this.#restoreMutations();
    }
    /** Apply Cube product language to stable breadcrumb and inspector anchors. */
    refresh() {
        this.#scheduled = false;
        this.#forgetDetachedMutations();
        const context = this.#contexts.resolveEditor(this.#currentGraph());
        const contextKey = context
            ? `${String(context.node.id)}:${context.isCubeRoot ? 'root' : 'nested'}`
            : null;
        if (contextKey !== this.#contextKey) {
            this.#restoreMutations();
            this.#contextKey = contextKey;
        }
        if (!context)
            return;
        const graphId = readGraphId(context.node.subgraph);
        const breadcrumb = this.#document.querySelector(`[data-testid="subgraph-breadcrumb-item-subgraph-${cssEscape(graphId)}"]`);
        const label = breadcrumb?.querySelector('.p-breadcrumb-item-label');
        if (label)
            this.#setText(label, readCubeTitle(context.node));
        if (breadcrumb) {
            this.#label(breadcrumb, `${readCubeTitle(context.node)} Cube actions`);
        }
        if (!context.isCubeRoot)
            return;
        const exit = this.#decision('exit-container');
        const back = this.#document.querySelector('[data-testid="subgraph-breadcrumb-back"]');
        if (back)
            this.#label(back, exit.label);
        const interfaceToggle = this.#document.querySelector('[data-testid="subgraph-editor-toggle"]');
        if (interfaceToggle)
            this.#hide(interfaceToggle);
        const description = this.#decision('set-description');
        const clear = this.#decision('clear-container');
        const aliases = this.#decision('set-search-aliases');
        const menu = this.#activeMenu(graphId);
        if (menu) {
            this.#relabelMenuItem(menu, '.pi-pencil', description.label);
            this.#relabelMenuItem(menu, '.pi-trash', clear.label);
            if (!aliases.visible)
                this.#hideMenuItem(menu, '.pi-search');
        }
    }
    /** Route active breadcrumb metadata and clear items before native handlers run. */
    #onClick(event) {
        const context = this.#contexts.resolveEditor(this.#currentGraph());
        if (!context?.isCubeRoot)
            return;
        const target = event.target instanceof Element ? event.target : null;
        if (!target)
            return;
        const menu = target.closest(`[data-testid="subgraph-breadcrumb-menu-subgraph-${cssEscape(readGraphId(context.node.subgraph))}"]`);
        if (!menu)
            return;
        const item = target.closest('li');
        if (item?.querySelector('.pi-pencil')) {
            stop(event);
            this.#controller.focusMetadata();
        }
        else if (item?.querySelector('.pi-trash')) {
            stop(event);
            void this.#controller.clearActiveCube(this.#currentGraph());
        }
        else if (item?.querySelector('.pi-search')) {
            stop(event);
            this.#controller.blocked('search-aliases');
        }
    }
    /** Replace native definition rename on Cube breadcrumb double-click. */
    #onDoubleClick(event) {
        const context = this.#contexts.resolveEditor(this.#currentGraph());
        if (!context?.isCubeRoot)
            return;
        const target = event.target instanceof Element ? event.target : null;
        const breadcrumb = target?.closest(`[data-testid="subgraph-breadcrumb-item-subgraph-${cssEscape(readGraphId(context.node.subgraph))}"]`);
        if (!breadcrumb)
            return;
        stop(event);
        this.#controller.focusMetadata();
    }
    /** Route Ctrl/Cmd+S to Cube save before Comfy's global workflow keybinding. */
    #onKeydown(event) {
        if (event.defaultPrevented ||
            event.key.toLowerCase() !== 's' ||
            (!event.ctrlKey && !event.metaKey) ||
            event.altKey) {
            return;
        }
        const context = this.#contexts.resolveEditor(this.#currentGraph());
        if (!context?.isCubeRoot)
            return;
        event.preventDefault();
        event.stopImmediatePropagation();
        this.#controller.saveActiveEditor(this.#currentGraph());
    }
    /** Coalesce Vue and PrimeVue mutation bursts. */
    #schedule() {
        if (this.#scheduled)
            return;
        this.#scheduled = true;
        queueMicrotask(() => this.refresh());
    }
    /** Read the graph currently visible in either renderer. */
    #currentGraph() {
        return this.#canvas.subgraph ?? this.#canvas.graph ?? null;
    }
    /** Find only the active definition's stable PrimeVue breadcrumb menu. */
    #activeMenu(graphId) {
        return this.#document.querySelector(`[data-testid="subgraph-breadcrumb-menu-subgraph-${cssEscape(graphId)}"]`);
    }
    /** Apply accessible product language and synchronize any mounted tooltip. */
    #label(element, label) {
        if (!this.#labelStates.has(element)) {
            this.#labelStates.set(element, {
                ariaLabel: element.getAttribute('aria-label'),
                title: element.title,
            });
        }
        element.setAttribute('aria-label', label);
        element.title = label;
        const tooltipId = element.getAttribute('aria-describedby');
        const tooltipText = tooltipId
            ? this.#document.getElementById(tooltipId)?.querySelector('.p-tooltip-text')
            : null;
        if (tooltipText)
            this.#setText(tooltipText, label);
    }
    /** Relabel one icon-identified PrimeVue menu item with literal-safe text. */
    #relabelMenuItem(menu, iconSelector, label) {
        const item = menu.querySelector(iconSelector)?.closest('li');
        const text = item?.querySelector('.p-menu-item-label');
        if (text)
            this.#setText(text, label);
    }
    /** Suppress one Blueprint-only menu item at the active Cube root. */
    #hideMenuItem(menu, iconSelector) {
        const item = menu.querySelector(iconSelector)?.closest('li');
        if (item)
            this.#hide(item);
    }
    /** Hide one host element while retaining its exact presentation state. */
    #hide(element) {
        if (!this.#hiddenStates.has(element)) {
            this.#hiddenStates.set(element, {
                hidden: element.hidden,
                display: element.style.getPropertyValue('display'),
                priority: element.style.getPropertyPriority('display'),
            });
        }
        element.hidden = true;
        element.style.setProperty('display', 'none', 'important');
    }
    /** Change text while retaining the exact host value for context transitions. */
    #setText(element, value) {
        if (!this.#textStates.has(element)) {
            this.#textStates.set(element, element.textContent ?? '');
        }
        if (element.textContent !== value)
            element.textContent = value;
    }
    /** Drop restoration records for host nodes that Vue has already unmounted. */
    #forgetDetachedMutations() {
        for (const element of this.#labelStates.keys()) {
            if (!element.isConnected)
                this.#labelStates.delete(element);
        }
        for (const element of this.#textStates.keys()) {
            if (!element.isConnected)
                this.#textStates.delete(element);
        }
        for (const element of this.#hiddenStates.keys()) {
            if (!element.isConnected)
                this.#hiddenStates.delete(element);
        }
    }
    /** Resolve one product decision from the shared renderer-independent policy. */
    #decision(actionId) {
        return this.#policy.decide(actionId, this.#contexts.resolveSelection(this.#canvas.selectedItems), this.#contexts.resolveEditor(this.#currentGraph()));
    }
    /** Restore host chrome exactly when leaving Cube-owned context. */
    #restoreMutations() {
        for (const [element, state] of this.#labelStates) {
            restoreAttribute(element, 'aria-label', state.ariaLabel);
            element.title = state.title;
        }
        this.#labelStates.clear();
        for (const [element, text] of this.#textStates)
            element.textContent = text;
        this.#textStates.clear();
        for (const [element, state] of this.#hiddenStates) {
            element.hidden = state.hidden;
            if (state.display)
                element.style.setProperty('display', state.display, state.priority);
            else
                element.style.removeProperty('display');
        }
        this.#hiddenStates.clear();
    }
}
/** Read the Cube definition title from Sugar metadata. */
function readCubeTitle(node) {
    const identity = requireCubeIdentity(node);
    const alias = identity.default_alias;
    return typeof alias === 'string' && alias.trim() ? alias.trim() : node.title || 'Untitled Cube';
}
/** Read a selector-safe native graph identity. */
function readGraphId(graph) {
    const value = Reflect.get(graph, 'id');
    return typeof value === 'string' ? value : '';
}
/** Escape a dynamic id without depending on browser-specific CSS globals. */
function cssEscape(value) {
    return value.replaceAll('\\', '\\\\').replaceAll('"', '\\"');
}
/** Restore one optional host attribute exactly. */
function restoreAttribute(element, name, value) {
    if (value === null)
        element.removeAttribute(name);
    else
        element.setAttribute(name, value);
}
/** Stop one native handler after the Cube controller has accepted its intent. */
function stop(event) {
    event.preventDefault();
    event.stopImmediatePropagation();
}

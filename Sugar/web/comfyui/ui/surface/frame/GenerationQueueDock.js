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
/** Keep Comfy's docked job panel on the right of the frame, with a way to put it away. */
import { PanelReveal } from '../motion/PanelReveal.js';
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import { isRecord } from '../../types/common.js';
import { GenerationQueueDockWidth } from './GenerationQueueDockWidth.js';
/** Comfy's docked job panel. */
export const GENERATION_QUEUE_DOCK = '[data-testid="job-history-sidebar"]';
/**
 * Set on the page root while the queue is docked, for the frame's styles to read.
 *
 * A rule that asked the root whether it "has" a docked panel made the browser restyle the whole
 * page after every change anywhere in it; an attribute this owner keeps costs nothing.
 */
export const GENERATION_QUEUE_DOCKED_ATTRIBUTE = 'data-sc-queue-docked';
const SIDEBAR_TAB = 'job-history';
/**
 * Present Comfy's docked job panel as the generation queue's pinned sidebar.
 *
 * The panel is Comfy's own side panel, which the frame otherwise keeps hidden together with
 * the icon column that opens and closes it. The frame's styles place the docked panel on the
 * right, where the generation queue has always been pinned; this owner gives it the button that
 * puts it away and the edge that resizes it. Comfy's own choice of side is left alone: changing
 * it rebuilds the workspace.
 */
export class GenerationQueueDock {
    document;
    app;
    report;
    layer;
    #presence;
    #hide;
    #width;
    #reveal = new PanelReveal();
    /** The docked side panel last shown, so only a newly docked panel slides in. */
    #shown = null;
    #disposed = false;
    /** Observe only the docked panel's own mount. */
    constructor(document, app, report, 
    /** The frame's own layer, which holds the resizing edge above Comfy's panels. */
    layer = document.body) {
        this.document = document;
        this.app = app;
        this.report = report;
        this.layer = layer;
        this.#hide = document.createElement('button');
        this.#hide.type = 'button';
        this.#hide.className = 'sc-generation-queue-dock-hide';
        this.#hide.setAttribute('aria-label', 'Hide generation queue');
        this.#hide.title = 'Hide generation queue';
        this.#hide.addEventListener('click', this.#close);
        this.#width = new GenerationQueueDockWidth(document, () => document.defaultView?.localStorage ?? null, report);
        this.#presence = new HostSurfaceObserver(document.body, GENERATION_QUEUE_DOCK, () => this.#reconcile());
        this.#reconcile();
    }
    /** Remove the owned button and its listener. */
    dispose() {
        this.#disposed = true;
        this.#presence.disconnect();
        this.#hide.removeEventListener('click', this.#close);
        this.#hide.remove();
        this.#width.dispose();
        this.#reveal.dispose();
        this.#markDocked(false);
    }
    #markDocked(docked) {
        const root = this.document.documentElement;
        if (root.hasAttribute(GENERATION_QUEUE_DOCKED_ATTRIBUTE) !== docked)
            root.toggleAttribute(GENERATION_QUEUE_DOCKED_ATTRIBUTE, docked);
    }
    /** Keep the hide button and the resizing edge in the docked panel through Comfy's re-renders. */
    #reconcile() {
        if (this.#disposed)
            return;
        const panel = this.document.querySelector(GENERATION_QUEUE_DOCK);
        this.#markDocked(panel !== null);
        if (!panel) {
            this.#shown = null;
            this.#hide.remove();
            this.#width.divider.remove();
            return;
        }
        // The edge lives in the frame's own layer, over the panel's left side: Comfy re-renders its
        // panel and would drop an element placed inside it.
        const side = panel.closest('.side-bar-panel') ?? panel;
        if (this.#shown !== side) {
            this.#shown = side;
            this.#reveal.enter(side, 'right');
        }
        this.#width.divider.style.top = `${String(Math.max(0, side.getBoundingClientRect().top))}px`;
        if (this.#width.divider.parentElement !== this.layer)
            this.layer.append(this.#width.divider);
        const actions = panel.querySelector('.p-toolbar-start > .ml-auto') ??
            panel.querySelector('.p-toolbar-start');
        if (actions && this.#hide.parentElement !== actions)
            actions.append(this.#hide);
    }
    /**
     * Slide the shown panel out, then run `then`, which puts it away. Every way the queue is put
     * away comes here, so it always leaves with the same motion. With no panel shown, `then` runs at
     * once; while the panel already slides out, `then` runs when that slide ends.
     */
    leave(then) {
        const panel = this.document.querySelector(GENERATION_QUEUE_DOCK);
        const side = panel?.closest('.side-bar-panel') ?? panel;
        if (side)
            this.#reveal.exit(side, 'right', then);
        else
            then();
    }
    /** Close the panel through the same owner Comfy's own icon column uses. */
    #close = () => {
        this.leave(() => {
            const tabs = this.#store('sidebarTab');
            if (isRecord(tabs) && tabs.activeSidebarTabId === SIDEBAR_TAB)
                this.#call(tabs, 'toggleSidebarTab', SIDEBAR_TAB);
        });
    };
    /** Narrow one of Comfy's stores on each use; they are replaced as Comfy starts. */
    #store(name) {
        const manager = isRecord(this.app) ? this.app.extensionManager : null;
        return isRecord(manager) ? manager[name] : null;
    }
    /** Call one store method, reporting a failure instead of leaving the panel half moved. */
    #call(store, method, ...values) {
        const target = isRecord(store) ? store[method] : null;
        if (typeof target !== 'function')
            return undefined;
        try {
            const result = Reflect.apply(target, store, values);
            if (result instanceof Promise)
                result.catch((error) => this.report(error));
            return result;
        }
        catch (error) {
            this.report(error);
            return undefined;
        }
    }
}

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
/** Mount explicitly allocated canvas/content siblings inside the qualified Comfy workspace. */
import { CanvasWorkspaceResizeInput } from '../presentation/browser/CanvasWorkspaceResizeInput.js';
import { ComfyCanvasFrameAllocation } from './ComfyCanvasFrameAllocation.js';
import { ComfyCanvasWorkspaceFrame } from './ComfyCanvasWorkspaceFrame.js';
import { ensureCanvasWorkspaceStyles } from './CanvasWorkspaceStyles.js';
import { BrowserCanvasWorkspaceMotion } from './BrowserCanvasWorkspaceMotion.js';
/** Keep graph frame, sibling bounds, and resize input coordinated without owning workflow results. */
export class BrowserCanvasWorkspace {
    model;
    actions;
    #paintedAllocation = null;
    content;
    canvas;
    #pane;
    #backdrop;
    #divider;
    #frame;
    #workspaceFrame;
    #input;
    #motion;
    #allocation = null;
    #suspension = null;
    #disposed = false;
    /** Allocate only owned nodes, keeping Comfy's chrome and every native render layer in its original parent. */
    constructor(root, panel, model, frames, actions) {
        this.model = model;
        this.actions = actions;
        this.#frame = new ComfyCanvasFrameAllocation(root, panel);
        const document = panel.ownerDocument;
        ensureCanvasWorkspaceStyles(document);
        this.content = document.createElement('div');
        this.content.className = 'sc-output-workspace-content';
        this.#pane = document.createElement('aside');
        this.#pane.className = 'sc-output-workspace-pane';
        this.#pane.setAttribute('aria-label', 'Workflow output canvas');
        this.#pane.setAttribute('data-capture-wheel', 'true');
        this.#pane.tabIndex = -1;
        this.canvas = document.createElement('div');
        this.canvas.className = 'sc-output-workspace-view';
        this.#divider = document.createElement('div');
        this.#divider.className = 'sc-output-workspace-divider';
        this.#divider.tabIndex = 0;
        this.#divider.setAttribute('role', 'separator');
        this.#divider.setAttribute('aria-label', 'Resize output canvas');
        this.#divider.setAttribute('aria-orientation', 'vertical');
        this.#pane.append(this.canvas, this.#divider);
        this.#motion = new BrowserCanvasWorkspaceMotion(this.#pane, () => {
            this.#paintedAllocation = null;
            this.#render();
        });
        this.#backdrop = document.createElement('button');
        this.#backdrop.className = 'sc-output-workspace-backdrop';
        this.#backdrop.type = 'button';
        this.#backdrop.setAttribute('aria-label', 'Close output canvas drawer');
        this.#backdrop.addEventListener('click', actions.close);
        this.#workspaceFrame = new ComfyCanvasWorkspaceFrame(root, panel, frames, (width, height) => {
            const previous = model.read().mode;
            model.bounds(width, height);
            if (model.read().mode !== previous)
                this.#input.cancel();
            this.#render();
        });
        this.#workspaceFrame.element.append(this.content, this.#backdrop, this.#pane);
        let input = null;
        try {
            this.#input = input = new CanvasWorkspaceResizeInput(this.#divider, frames, {
                width: () => model.read().canvas.width,
                preview: (width) => {
                    model.resize(width);
                    this.#render();
                },
                commit: () => actions.editorWidth(model.read().editorWidth),
                cancel: () => {
                    model.cancelResize();
                    this.#render();
                },
            });
            this.#workspaceFrame.measure();
        }
        catch (error) {
            this.#disposed = true;
            input?.dispose();
            this.#motion.dispose();
            this.#workspaceFrame.dispose();
            this.#backdrop.removeEventListener('click', actions.close);
            this.#frame.dispose();
            this.#pane.remove();
            this.#backdrop.remove();
            this.content.remove();
            throw error;
        }
    }
    /** Read accepted open state independently of pixels retained during the exit slide. */
    get visible() {
        return !this.#disposed && this.#suspension === null && this.#allocation?.visible === true;
    }
    /** Retain the outgoing primary attachment until its exit slide completes. */
    get presented() {
        return !this.#disposed && this.#suspension === null && this.#motion.present;
    }
    /** Apply accepted view settings while preserving borrowed masonry control identity. */
    preferences(preferences, animate = false) {
        this.model.preferences(preferences);
        this.#render(animate);
    }
    /** Open/close a narrow drawer only after its explicit user action is accepted. */
    drawer(open, animate = false) {
        this.model.drawer(open);
        this.#render(animate);
        if (open && this.#allocation?.mode === 'drawer')
            this.#pane.focus({ preventScroll: true });
    }
    /** Keep content identity while the host temporarily hides graph mode or configures its workflow. */
    suspend(reason) {
        if (this.#disposed || (this.#suspension === reason && !this.#pane.dataset.canvasMotion))
            return;
        this.#suspension = reason;
        this.#paintedAllocation = null;
        this.#input.cancel();
        this.model.cancelResize();
        this.#render();
    }
    /** Relinquish only owned regions and restore the native graph's exact frame styles. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#input.dispose();
        this.#motion.dispose();
        this.#workspaceFrame.dispose();
        this.#backdrop.removeEventListener('click', this.actions.close);
        this.#frame.dispose();
        this.#pane.remove();
        this.#backdrop.remove();
        this.content.remove();
    }
    /** Apply one authoritative rectangle to masonry, its backdrop, rail, and finder through their shared parent. */
    #render(animate = false) {
        if (this.#disposed)
            return;
        const allocation = this.model.read();
        this.#allocation = allocation;
        const visible = allocation.visible && this.#suspension === null;
        const inset = this.#applyFrame();
        const painted = JSON.stringify([
            visible,
            inset,
            allocation.canvas.width,
            allocation.mode,
            allocation.minimumWidth,
            allocation.maximumWidth,
        ]);
        if (this.#paintedAllocation === painted) {
            this.actions.changed();
            return;
        }
        this.#paintedAllocation = painted;
        this.content.style.left = '0px';
        this.content.style.width = `calc(100% - ${inset}px)`;
        if (visible || !animate)
            this.#pane.style.width = `${allocation.canvas.width}px`;
        if (animate)
            void this.#motion.setVisible(visible).catch(this.actions.report);
        else
            this.#motion.restore(visible);
        this.#backdrop.hidden = !this.#motion.present || allocation.mode !== 'drawer';
        this.#backdrop.style.right = this.#pane.style.width;
        this.#divider.setAttribute('aria-valuenow', String(Math.round(allocation.canvas.width)));
        this.#divider.setAttribute('aria-valuemin', String(Math.round(allocation.minimumWidth)));
        this.#divider.setAttribute('aria-valuemax', String(Math.round(allocation.maximumWidth)));
        this.actions.changed();
    }
    /** Allocate the graph through its sole native geometry owner for the accepted presentation. */
    #applyFrame() {
        const allocation = this.model.read();
        const inset = allocation.visible && this.#suspension !== 'app' && allocation.mode === 'docked'
            ? allocation.canvas.width
            : 0;
        if (this.#suspension === 'app')
            this.#frame.suspend();
        else
            this.#frame.apply(inset);
        return inset;
    }
}

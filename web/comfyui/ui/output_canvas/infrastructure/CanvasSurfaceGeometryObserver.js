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
/** Observe one web-owned surface allocation and its conservative native clip. */
import { HostAncestryObserver } from '../../core/HostAncestryObserver.js';
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import { canvasGeometryAncestry, canvasVisibleClip } from './CanvasVisibleClip.js';
/** Coalesce layout, visibility, scrolling, and density changes within the attachment's document. */
export class CanvasSurfaceGeometryObserver {
    element;
    frames;
    changed;
    #resize;
    #ancestry;
    #presence = null;
    #stops = [];
    #ancestryStops = [];
    #ancestors = [];
    #roots = [];
    #media = null;
    #frame = null;
    #disposed = false;
    /** Bind explicit document/window geometry and release observer/listener ownership together. */
    constructor(element, frames, changed) {
        this.element = element;
        this.frames = frames;
        this.changed = changed;
        const document = element.ownerDocument;
        const window = document.defaultView;
        if (!window)
            throw new Error('Canvas document window is unavailable.');
        if (!element.hasAttribute('data-sc-canvas-geometry'))
            throw new Error('Canvas geometry surface is not identified.');
        const Resize = window.ResizeObserver;
        this.#resize = Resize ? new Resize(() => this.#schedule()) : null;
        this.#ancestry = new HostAncestryObserver(window, () => {
            this.#observeAncestry();
            this.#schedule();
        }, ['data-canvas-motion']);
        this.#observeAncestry();
        const schedule = () => this.#schedule();
        window.addEventListener('resize', schedule);
        window.addEventListener('scroll', schedule);
        document.addEventListener('scroll', schedule, true);
        document.addEventListener('visibilitychange', schedule);
        window.visualViewport?.addEventListener('resize', schedule);
        window.visualViewport?.addEventListener('scroll', schedule);
        element.addEventListener('focusin', schedule);
        element.addEventListener('focusout', schedule);
        this.#stops.push(() => window.removeEventListener('resize', schedule), () => window.removeEventListener('scroll', schedule), () => document.removeEventListener('scroll', schedule, true), () => document.removeEventListener('visibilitychange', schedule), () => window.visualViewport?.removeEventListener('resize', schedule), () => window.visualViewport?.removeEventListener('scroll', schedule), () => element.removeEventListener('focusin', schedule), () => element.removeEventListener('focusout', schedule));
        this.#density();
    }
    /** Read CSS allocation and display density without traversing the workflow graph. */
    read() {
        const bounds = this.element.getBoundingClientRect();
        const document = this.element.ownerDocument;
        const density = document.defaultView?.devicePixelRatio ?? 1;
        const valid = [bounds.left, bounds.top, bounds.right, bounds.bottom, bounds.width, bounds.height].every((value) => Number.isFinite(value) && Math.abs(value) <= 1_000_000) &&
            bounds.width >= 0 &&
            bounds.height >= 0 &&
            bounds.right >= bounds.left &&
            bounds.bottom >= bounds.top &&
            Math.abs(bounds.right - bounds.left - bounds.width) <= 0.000_001 &&
            Math.abs(bounds.bottom - bounds.top - bounds.height) <= 0.000_001 &&
            Number.isFinite(density) &&
            density > 0 &&
            density <= 64;
        const measured = valid ? canvasVisibleClip(this.element, bounds) : null;
        return Object.freeze({
            coordinateSpace: 'client',
            units: 'css-pixels',
            origin: Object.freeze({ x: valid ? bounds.left : 0, y: valid ? bounds.top : 0 }),
            viewport: Object.freeze({
                width: valid ? bounds.width : 0,
                height: valid ? bounds.height : 0,
            }),
            clip: measured?.clip ?? Object.freeze({ x: 0, y: 0, width: 0, height: 0 }),
            physicalPixelsPerCssPixel: valid ? density : 1,
            visible: measured?.visible ?? false,
            focused: this.element.contains(document.activeElement),
        });
    }
    /** Stop geometry delivery before the viewport is detached. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#resize?.disconnect();
        this.#ancestry.disconnect();
        this.#presence?.disconnect();
        this.#presence = null;
        this.#ancestors = [];
        this.#roots = [];
        this.#frame?.();
        this.#frame = null;
        this.#media?.();
        this.#media = null;
        for (const stop of this.#ancestryStops.splice(0))
            stop();
        for (const stop of this.#stops.splice(0))
            stop();
    }
    /** Observe every current clipping ancestor, including reparenting of a staged surface. */
    #observeAncestry() {
        if (this.#disposed)
            return;
        if (!this.element.isConnected) {
            this.#presence ??= new HostSurfaceObserver(this.element.ownerDocument.documentElement, '[data-sc-canvas-geometry]', (changes) => {
                if (!changes.some((change) => change.element === this.element && change.element.isConnected))
                    return;
                this.#presence?.disconnect();
                this.#presence = null;
                this.#observeAncestry();
                this.#schedule();
            });
            if (this.#ancestors.length)
                return;
        }
        else if (this.#presence) {
            this.#presence.disconnect();
            this.#presence = null;
        }
        const measured = canvasGeometryAncestry(this.element);
        const ancestors = measured?.elements ?? [this.element];
        const roots = measured?.roots ?? [];
        if (ancestors.length === this.#ancestors.length &&
            ancestors.every((node, index) => node === this.#ancestors[index]) &&
            roots.length === this.#roots.length &&
            roots.every((root, index) => root === this.#roots[index]))
            return;
        this.#ancestors = ancestors;
        this.#roots = roots;
        for (const stop of this.#ancestryStops.splice(0))
            stop();
        for (const node of ancestors) {
            if (node.localName === 'slot') {
                const changed = () => {
                    this.#observeAncestry();
                    this.#schedule();
                };
                node.addEventListener('slotchange', changed);
                this.#ancestryStops.push(() => node.removeEventListener('slotchange', changed));
            }
            const motion = (event) => {
                if (event.target === node)
                    this.#schedule();
            };
            for (const type of [
                'animationstart',
                'animationend',
                'animationcancel',
                'transitionrun',
                'transitionend',
                'transitioncancel',
            ]) {
                node.addEventListener(type, motion);
                this.#ancestryStops.push(() => node.removeEventListener(type, motion));
            }
        }
        this.#resize?.disconnect();
        for (const node of ancestors)
            this.#resize?.observe(node);
        this.#ancestry.observe(this.element, ancestors, roots);
        if (this.#moving())
            this.#schedule();
    }
    /** Observe motion already underway when this attachment or its ancestry is installed. */
    #moving() {
        return this.#ancestors.some((node) => typeof node.getAnimations === 'function' &&
            node
                .getAnimations()
                .some((animation) => animation.playState === 'running' || animation.pending));
    }
    /** Re-arm a density notification only when monitor/page density changes. */
    #density() {
        const window = this.element.ownerDocument.defaultView;
        this.#media?.();
        if (!window?.matchMedia || this.#disposed)
            return;
        const query = window.matchMedia(`(resolution: ${window.devicePixelRatio || 1}dppx)`);
        const changed = () => {
            this.#density();
            this.#schedule();
        };
        query.addEventListener('change', changed);
        this.#media = () => query.removeEventListener('change', changed);
    }
    /** Schedule one measurement for a burst of geometry events. */
    #schedule() {
        if (this.#frame || this.#disposed)
            return;
        this.#frame = this.frames.request(() => {
            this.#frame = null;
            if (!this.#disposed) {
                this.#observeAncestry();
                this.changed(this.read());
                if (!this.#disposed && this.#moving())
                    this.#schedule();
            }
        });
    }
}

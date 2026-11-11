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
/** Measure the two owned navigation bars for native-frame acrylic, independently of hit testing. */
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import { HostAncestryObserver } from '../../core/HostAncestryObserver.js';
import { canvasGeometryAncestry } from './CanvasVisibleClip.js';
const BARS = '[data-sc-canvas-backdrop]';
/** Keep material layout observation attached to one canvas and retire it with that attachment. */
export class NativeCanvasBackdrops {
    root;
    invalidated;
    #bars = [];
    #resize = null;
    #presence = null;
    #ancestry = null;
    #ancestors = [];
    #measurement = null;
    #dirty = true;
    #complete = false;
    #disposed = false;
    #observing = false;
    /** Inject the owned overlay root and immediate opacity invalidation, without camera events. */
    constructor(root, invalidated) {
        this.root = root;
        this.invalidated = invalidated;
    }
    /** Report whether the last measurement still describes every current visible bar. */
    get current() {
        return !this.#disposed && !this.#dirty && this.#complete;
    }
    /** Snapshot cached observation state without measuring layout, changing readiness or exposing elements. */
    diagnostics() {
        const measured = this.#measurement;
        return Object.freeze({
            observing: this.#observing,
            disposed: this.#disposed,
            dirty: this.#dirty,
            complete: this.#complete,
            current: this.current,
            bars: this.#bars.length,
            measured: measured
                ? Object.freeze({
                    origin: Object.freeze({ ...measured.origin }),
                    viewport: Object.freeze({ ...measured.viewport }),
                    regions: Object.freeze(measured.regions.map((region) => Object.freeze({ ...region }))),
                })
                : null,
        });
    }
    /** Observe only identified bar mounts, local styling and size; unrelated overlays are excluded. */
    observe(changed) {
        if (this.#observing || this.#disposed)
            throw new Error('Native backdrop observer is unavailable.');
        this.#observing = true;
        const Resize = this.root.ownerDocument.defaultView?.ResizeObserver;
        const notify = () => {
            if (this.#disposed || this.#unchanged())
                return;
            this.invalidate();
            changed();
        };
        this.#resize = Resize ? new Resize(notify) : null;
        const window = this.root.ownerDocument.defaultView;
        if (window)
            this.#ancestry = new HostAncestryObserver(window, notify, [
                'data-sc-theme',
                'data-sc-color-scheme',
                'data-sc-visual-style',
                'data-theme',
            ]);
        this.#presence = new HostSurfaceObserver(this.root, BARS, (changes) => {
            if (changes.some((change) => change.kind !== 'content' ||
                change.relocated ||
                !this.#bars.includes(change.element) ||
                !this.root.contains(change.element)))
                this.#synchronize();
            notify();
        }, { attributes: ['class', 'style', 'hidden'] });
        this.#synchronize();
        return () => this.dispose();
    }
    /** Ignore control styling and duplicate resize deliveries when the accepted material is exact. */
    #unchanged() {
        const previous = this.#measurement;
        if (!previous || this.#dirty)
            return false;
        const complete = this.#complete;
        const regions = this.#measure(previous.origin, previous.viewport);
        const same = previous.signature === JSON.stringify([this.#complete, regions]);
        this.#complete = complete;
        return same;
    }
    /** Withdraw transparency before a queued layout measurement can reach the native renderer. */
    invalidate() {
        if (this.#disposed || this.#dirty)
            return;
        this.#dirty = true;
        this.invalidated();
    }
    /** Read actual client bounds and material lengths, retaining no pixels or image URLs. */
    read(origin, viewport) {
        if (this.#observeAncestry())
            this.#dirty = true;
        const previous = this.#measurement;
        if (!this.#dirty &&
            previous &&
            origin.x === previous.origin.x &&
            origin.y === previous.origin.y &&
            viewport.width === previous.viewport.width &&
            viewport.height === previous.viewport.height)
            return previous.regions;
        const regions = this.#measure(origin, viewport);
        this.#dirty = false;
        this.#measurement = {
            origin,
            viewport,
            signature: JSON.stringify([this.#complete, regions]),
            regions,
        };
        return regions;
    }
    /** Resolve bounded material inputs only when layout or the observed CSS ancestry changes. */
    #measure(origin, viewport) {
        const window = this.root.ownerDocument.defaultView;
        const regions = [];
        this.#complete = !this.#disposed && window !== null && this.#bars.length <= 2;
        if (!this.#complete || !window)
            return Object.freeze([]);
        for (const bar of this.#bars) {
            const style = window.getComputedStyle(bar);
            const bounds = bar.getBoundingClientRect();
            if (!bar.isConnected ||
                bar.hidden ||
                style.display === 'none' ||
                style.visibility === 'hidden' ||
                style.opacity === '0' ||
                bounds.width === 0 ||
                bounds.height === 0)
                continue;
            const x = bounds.left - origin.x;
            const y = bounds.top - origin.y;
            const radius = pixels(style.borderTopLeftRadius || style.borderRadius);
            const sigma = pixels(style.getPropertyValue('--p-toast-blur').trim() || '10px');
            const corners = [
                style.borderTopRightRadius,
                style.borderBottomLeftRadius,
                style.borderBottomRightRadius,
            ];
            if (![x, y, bounds.width, bounds.height].every(Number.isFinite) ||
                x < 0 ||
                y < 0 ||
                bounds.width <= 0 ||
                bounds.height <= 0 ||
                x + bounds.width > viewport.width ||
                y + bounds.height > viewport.height ||
                radius === null ||
                sigma === null ||
                corners.some((corner) => corner && pixels(corner) !== radius)) {
                this.#complete = false;
                return Object.freeze([]);
            }
            regions.push(Object.freeze({
                x,
                y,
                width: bounds.width,
                height: bounds.height,
                cornerRadius: Math.min(radius, bounds.width / 2, bounds.height / 2),
                sigma: Math.min(sigma, 64),
            }));
        }
        return Object.freeze(regions);
    }
    /** Release observer callbacks before the owning overlay is detached. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#resize?.disconnect();
        this.#presence?.disconnect();
        this.#ancestry?.disconnect();
        this.#resize = null;
        this.#presence = null;
        this.#ancestry = null;
        this.#measurement = null;
        this.#ancestors = [];
        this.#bars = [];
        this.#complete = false;
    }
    /** Replace the bounded inventory only after identified bars mount or retire. */
    #synchronize() {
        const bars = [...this.root.querySelectorAll(BARS)].slice(0, 3);
        const same = bars.length === this.#bars.length && bars.every((bar, index) => bar === this.#bars[index]);
        this.#bars = bars;
        this.#observeAncestry();
        if (same)
            return;
        this.#resize?.disconnect();
        for (const bar of this.#bars.slice(0, 2))
            this.#resize?.observe(bar);
    }
    /** Rebind only when the bars' composed CSS ancestry changes, including overlay reparenting. */
    #observeAncestry() {
        const ancestors = new Set();
        for (const bar of this.#bars.slice(0, 2)) {
            const ancestry = canvasGeometryAncestry(bar);
            for (const element of ancestry?.elements ?? [])
                ancestors.add(element);
        }
        const next = [...ancestors];
        if (next.length === this.#ancestors.length &&
            next.every((element, index) => element === this.#ancestors[index]))
            return false;
        this.#ancestors = next;
        this.#ancestry?.observeAttributes(next);
        return true;
    }
}
/** Accept only finite nonnegative circular CSS-pixel lengths used by the actual bar style. */
function pixels(value) {
    if (!/^(?:\d+(?:\.\d+)?|\.\d+)(?:px)?$/.test(value.trim()))
        return null;
    const parsed = Number.parseFloat(value);
    return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
}

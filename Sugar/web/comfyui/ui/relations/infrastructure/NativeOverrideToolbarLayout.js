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
/** Publish native Run docking and intrinsic size changes without observing override animation. */
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
/** Observe only native Run geometry and retain one subscription across chrome replacements. */
export class NativeOverrideToolbarLayout {
    #bar = null;
    #native = null;
    #floating = false;
    #presence = null;
    #size = null;
    #changed = null;
    /** Bind layout notification without owning toolbar placement or persistence. */
    subscribe(changed) {
        this.#changed = changed;
        return () => {
            this.#changed = null;
        };
    }
    /** Follow the actual native Run panel while excluding the owned dock slot. */
    mount(bar) {
        if (this.#bar === bar)
            return;
        if (this.#bar !== bar) {
            this.#presence?.disconnect();
            this.#presence = null;
            this.#bar = bar;
            if (bar)
                this.#presence = new HostSurfaceObserver(bar, '.actionbar', () => this.#reconcile(), {
                    attributes: ['class'],
                });
        }
        this.#reconcile();
    }
    /** Release native observation before host disposal. */
    dispose() {
        this.#presence?.disconnect();
        this.#size?.disconnect();
        this.#presence = null;
        this.#size = null;
        this.#bar = null;
        this.#native = null;
        this.#changed = null;
    }
    /** Notify on native mode changes and measure only the native panel's independent size. */
    #reconcile() {
        const native = this.#bar?.querySelector('.actionbar') ?? null;
        const position = native?.ownerDocument.defaultView?.getComputedStyle(native).position;
        const floating = position === 'fixed' || position === 'absolute';
        const changed = this.#native !== native || floating !== this.#floating;
        if (this.#native !== native) {
            this.#size?.disconnect();
            this.#size = null;
            const Observer = native?.ownerDocument.defaultView?.ResizeObserver;
            if (Observer && native) {
                this.#size = new Observer(() => this.#changed?.());
                this.#size.observe(native);
                const flow = native.parentElement;
                // The native wrapper also owns Run's temporary dock target, independently of our slot.
                if (flow && flow !== this.#bar)
                    this.#size.observe(flow);
            }
        }
        this.#native = native;
        this.#floating = floating;
        if (changed)
            this.#changed?.();
    }
}

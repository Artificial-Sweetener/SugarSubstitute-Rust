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
/** Fade only the stack's material aperture, retaining controls and the surrounding body wash. */
const OPACITY = '--sc-stack-wash-opacity';
const WIDTH = '--sc-stack-material-width';
const DURATION_MS = 220;
/**
 * Retarget the Python presentation's 220 ms InOutCubic frame from its currently painted value.
 *
 * The aperture's width and its wash opacity travel together, as Substitute's material region
 * followed the stack widget's animated geometry: the body beside the rail keeps its full wash
 * while the rail grows or shrinks into it.
 */
export class CubeStackMaterialMotion {
    #container;
    #reportError;
    #animation = null;
    #target = null;
    /** Bind the common workspace ancestor that supplies the backdrop's aperture properties. */
    constructor(container, reportError) {
        this.#container = container;
        this.#reportError = reportError;
    }
    /** Keep density changes independent of row reconciliation and honor the current motion preference. */
    set(compact, width, animated = true) {
        const target = { opacity: compact ? 0 : 1, width };
        const previous = this.#target;
        if (previous?.opacity === target.opacity && previous.width === target.width)
            return;
        const element = this.#container;
        const view = element.ownerDocument.defaultView;
        const start = this.#paintedFrame(previous ?? target);
        this.#target = target;
        this.#animation?.cancel();
        this.#animation = null;
        element.style.setProperty(OPACITY, String(target.opacity));
        element.style.setProperty(WIDTH, `${String(target.width)}px`);
        if (previous === null ||
            !animated ||
            (start.opacity === target.opacity && start.width === target.width) ||
            typeof element.animate !== 'function' ||
            view?.matchMedia?.('(prefers-reduced-motion: reduce)').matches)
            return;
        const frame = (progress) => ({
            [OPACITY]: String(start.opacity + (target.opacity - start.opacity) * progress),
            [WIDTH]: `${String(start.width + (target.width - start.width) * progress)}px`,
        });
        // Each half has linear x and cubic y, reproducing Qt InOutCubic without a Bezier approximation.
        const animation = element.animate([
            {
                ...frame(0),
                offset: 0,
                easing: 'cubic-bezier(0.3333333333333333,0,0.6666666666666666,0)',
            },
            {
                ...frame(0.5),
                offset: 0.5,
                easing: 'cubic-bezier(0.3333333333333333,1,0.6666666666666666,1)',
            },
            { ...frame(1), offset: 1 },
        ], { duration: DURATION_MS });
        this.#animation = animation;
        void Promise.allSettled([animation.finished]).then((results) => {
            if (this.#animation !== animation)
                return;
            const result = results[0];
            if (result?.status === 'rejected')
                this.#reportError(new Error('Cube stack material animation failed.', { cause: result.reason }));
            animation.cancel();
            this.#animation = null;
        });
    }
    /** Remove the aperture when the rail leaves its workspace lifetime. */
    dispose() {
        this.#animation?.cancel();
        this.#animation = null;
        this.#container.style.removeProperty(OPACITY);
        this.#container.style.removeProperty(WIDTH);
    }
    /** Read the live interpolated aperture, falling back to the last target where styles are absent. */
    #paintedFrame(fallback) {
        const computed = this.#container.ownerDocument.defaultView?.getComputedStyle(this.#container);
        const opacity = Number.parseFloat(computed?.getPropertyValue(OPACITY) ?? '');
        const width = Number.parseFloat(computed?.getPropertyValue(WIDTH) ?? '');
        return {
            opacity: Number.isFinite(opacity) ? opacity : fallback.opacity,
            width: Number.isFinite(width) ? width : fallback.width,
        };
    }
}

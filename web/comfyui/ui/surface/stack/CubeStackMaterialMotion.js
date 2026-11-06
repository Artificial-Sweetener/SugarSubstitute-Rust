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
/** Retarget the Python presentation's 220 ms InOutCubic wash from its currently painted value. */
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
        const element = this.#container;
        const materialWidth = `${String(width)}px`;
        if (element.style.getPropertyValue(WIDTH) !== materialWidth)
            element.style.setProperty(WIDTH, materialWidth);
        const target = compact ? 0 : 1;
        if (target === this.#target)
            return;
        const view = element.ownerDocument.defaultView;
        const start = Number(view?.getComputedStyle(element).getPropertyValue(OPACITY) || 1);
        const initial = this.#target === null;
        this.#target = target;
        this.#animation?.cancel();
        this.#animation = null;
        element.style.setProperty(OPACITY, String(target));
        if (initial ||
            !animated ||
            start === target ||
            typeof element.animate !== 'function' ||
            view?.matchMedia?.('(prefers-reduced-motion: reduce)').matches)
            return;
        // Each half has linear x and cubic y, reproducing Qt InOutCubic without a Bezier approximation.
        const animation = element.animate([
            {
                [OPACITY]: String(start),
                offset: 0,
                easing: 'cubic-bezier(0.3333333333333333,0,0.6666666666666666,0)',
            },
            {
                [OPACITY]: String((start + target) / 2),
                offset: 0.5,
                easing: 'cubic-bezier(0.3333333333333333,1,0.6666666666666666,1)',
            },
            { [OPACITY]: String(target), offset: 1 },
        ], { duration: 220 });
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
}

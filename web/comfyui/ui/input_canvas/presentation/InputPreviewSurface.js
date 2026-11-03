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
/** Own persistent preview geometry and passive plugin presentation independently of source acquisition. */
import { decodeCanvasRaster } from '../../core/embedded/CanvasRuntime.js';
import { maskPreviewPixels } from './MaskPreviewWash.js';
/** Keep the same raster and frame mounted while source identity, interpretation and availability change. */
export class InputPreviewSurface {
    element;
    canvas;
    #label;
    #frame;
    #placeholder;
    #message;
    #crop;
    #passive;
    #painted = false;
    #color = null;
    #linked = false;
    #rendered = null;
    /** Use native button semantics only when entry selection belongs to the preview. */
    constructor(document, passive, select) {
        this.#passive = passive;
        this.element = this.#container(document, passive, select);
        this.#label = document.createElement('span');
        this.#label.className = 'sc-input-editor__label';
        this.#frame = document.createElement('div');
        this.#frame.className = 'sc-input-editor__frame';
        this.#frame.style.aspectRatio = '1 / 1';
        this.canvas = document.createElement('canvas');
        this.canvas.className = 'sc-input-editor__preview';
        this.canvas.hidden = true;
        this.#placeholder = document.createElement('div');
        this.#placeholder.className = 'sc-input-editor__placeholder';
        this.#placeholder.setAttribute('role', 'status');
        const icon = document.createElement('span');
        icon.textContent = 'ⓘ';
        icon.setAttribute('aria-hidden', 'true');
        icon.className = 'sc-input-editor__placeholder-icon';
        this.#message = document.createElement('span');
        this.#message.className = 'sc-input-editor__message';
        this.#placeholder.append(icon, this.#message);
        this.#frame.append(this.canvas, this.#placeholder);
        this.#crop = document.createElement('div');
        this.#crop.className = 'sc-input-editor__crop';
        this.#crop.hidden = true;
        this.#crop.setAttribute('aria-hidden', 'true');
        this.#frame.append(this.#crop);
        this.element.append(this.#label, this.#frame);
        this.loading();
    }
    /** Replace only mode-dependent semantics, retaining the raster and its reserved geometry. */
    update(label, selected, busy, passive, select) {
        if (passive !== this.#passive) {
            const previous = this.element;
            this.element = this.#container(previous.ownerDocument, passive, select);
            this.element.dataset.inputEntry = previous.dataset.inputEntry ?? '';
            this.element.append(this.#label, this.#frame);
            previous.replaceWith(this.element);
            this.#passive = passive;
            this.#applyRegion();
        }
        this.#label.textContent = label;
        this.canvas.setAttribute('aria-label', label + ' preview');
        this.element.dataset.selected = String(selected);
        if (this.element instanceof HTMLButtonElement) {
            this.element.setAttribute('aria-pressed', String(selected));
            this.element.disabled = busy;
        }
    }
    /** Colour the row's rail with its region colour and show a hover linked from another view. */
    region(color, linked) {
        this.#color = color;
        this.#linked = linked;
        this.#applyRegion();
    }
    /** Supply the persistent content box to the display-resolution observer. */
    get frameElement() {
        return this.#frame;
    }
    /** Reserve the full canvas aspect ratio even when no target pixels can be captured. */
    frame(width, height) {
        if (width > 0 && height > 0)
            this.#frame.style.aspectRatio = `${width} / ${height}`;
    }
    /** Retain the last frame during replacement; use a placeholder only before the first frame exists. */
    loading() {
        if (!this.#painted)
            this.unavailable('Loading…');
        this.#frame.setAttribute('aria-busy', 'true');
    }
    /** Replace stale pixels with concise feedback inside the same persistent frame. */
    unavailable(message) {
        this.#crop.hidden = true;
        this.#painted = false;
        this.canvas.hidden = true;
        this.#placeholder.hidden = false;
        this.#message.textContent = message;
        this.#frame.dataset.content = 'unavailable';
        this.#frame.setAttribute('aria-busy', 'false');
    }
    /** Show empty associated-image context as solid black without changing the reserved preview geometry. */
    black() {
        this.#crop.hidden = true;
        this.#painted = true;
        this.canvas.hidden = true;
        this.#placeholder.hidden = true;
        this.#message.textContent = '';
        this.#frame.dataset.content = 'black';
        this.#frame.setAttribute('aria-busy', 'false');
    }
    /** Paint interpreted pixels with one optional associated-image wash, retaining the mounted preview surface. */
    paint(pixels, frame, background = null, placement = null, outline = false, viewport = null) {
        const raster = background ?? pixels, canvas = this.canvas;
        if (this.#rendered?.pixels !== pixels ||
            this.#rendered.background !== background ||
            this.#rendered.placement !== placement) {
            this.#paintPixels(pixels, background, placement);
            this.#rendered = { pixels, background, placement };
        }
        Object.assign(canvas.style, viewport
            ? {
                left: `${(-viewport.bounds.left * 100) / viewport.bounds.width}%`,
                top: `${(-viewport.bounds.top * 100) / viewport.bounds.height}%`,
                width: `${(viewport.frame.width * 100) / viewport.bounds.width}%`,
                height: `${(viewport.frame.height * 100) / viewport.bounds.height}%`,
                objectFit: 'fill',
            }
            : { left: '0%', top: '0%', width: '100%', height: '100%', objectFit: 'contain' });
        this.frame(frame?.width ?? raster.width, frame?.height ?? raster.height);
        this.#crop.hidden = !placement || !outline;
        if (placement && outline) {
            const { bounds, frame: source } = placement;
            Object.assign(this.#crop.style, {
                left: `${(bounds.left * 100) / source.width}%`,
                top: `${(bounds.top * 100) / source.height}%`,
                width: `${(bounds.width * 100) / source.width}%`,
                height: `${(bounds.height * 100) / source.height}%`,
            });
        }
        this.#shown();
    }
    /** Upload changed display pixels once while viewport-only changes retain the same canvas backing store. */
    #paintPixels(pixels, background, placement) {
        const raster = background ?? pixels, canvas = this.canvas;
        const context = canvas.getContext('2d');
        if (!context)
            throw new Error('Source preview rendering is unavailable.');
        const image = context.createImageData(raster.width, raster.height);
        if (background)
            image.data.set(maskPreviewPixels(pixels, background, placement));
        else {
            const bytes = decodeCanvasRaster(pixels);
            if (pixels.format === 'rgba8')
                image.data.set(bytes);
            else
                for (let index = 0; index < bytes.length; index++) {
                    const offset = index * 4, value = bytes[index] ?? 0;
                    image.data[offset] = image.data[offset + 1] = image.data[offset + 2] = value;
                    image.data[offset + 3] = 255;
                }
        }
        if (canvas.width !== raster.width)
            canvas.width = raster.width;
        if (canvas.height !== raster.height)
            canvas.height = raster.height;
        context.putImageData(image, 0, 0);
    }
    /** Reveal completed pixels after one atomic paint and retain them during replacement work. */
    #shown() {
        this.#painted = true;
        this.canvas.hidden = false;
        this.#placeholder.hidden = true;
        this.#message.textContent = '';
        this.#frame.dataset.content = 'pixels';
        this.#frame.setAttribute('aria-busy', 'false');
    }
    /** Apply region identity to the current container, which passive-mode changes replace. */
    #applyRegion() {
        if (this.#color)
            this.element.style.setProperty('--sc-mask-color', this.#color);
        else
            this.element.style.removeProperty('--sc-mask-color');
        this.element.dataset.linkedHover = String(this.#linked);
    }
    /** Keep plugin previews outside the focus and command paths. */
    #container(document, passive, select) {
        const element = document.createElement(passive ? 'div' : 'button');
        element.className = 'sc-input-editor__row';
        element.dataset.passive = String(passive);
        if (element instanceof HTMLButtonElement) {
            element.type = 'button';
            element.addEventListener('click', select);
        }
        return element;
    }
}

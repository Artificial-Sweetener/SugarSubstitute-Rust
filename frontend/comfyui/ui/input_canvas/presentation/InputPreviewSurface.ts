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
import { decodeCanvasRaster, type CanvasPixelRaster } from '../../core/embedded/CanvasRuntime.js';

/** Keep the same raster and frame mounted while source identity, interpretation and availability change. */
export class InputPreviewSurface {
  element: HTMLElement;
  readonly canvas: HTMLCanvasElement;
  readonly #label: HTMLElement;
  readonly #frame: HTMLElement;
  readonly #placeholder: HTMLElement;
  readonly #message: HTMLElement;
  #passive: boolean;
  #painted = false;

  /** Use native button semantics only when entry selection belongs to the preview. */
  constructor(document: Document, passive: boolean, select: () => void) {
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
    this.element.append(this.#label, this.#frame);
    this.loading();
  }

  /** Replace only mode-dependent semantics, retaining the raster and its reserved geometry. */
  update(
    label: string,
    selected: boolean,
    busy: boolean,
    passive: boolean,
    select: () => void,
  ): void {
    if (passive !== this.#passive) {
      const previous = this.element;
      this.element = this.#container(previous.ownerDocument, passive, select);
      this.element.dataset.inputEntry = previous.dataset.inputEntry ?? '';
      this.element.append(this.#label, this.#frame);
      previous.replaceWith(this.element);
      this.#passive = passive;
    }
    this.#label.textContent = label;
    this.canvas.setAttribute('aria-label', label + ' preview');
    this.element.dataset.selected = String(selected);
    if (this.element instanceof HTMLButtonElement) {
      this.element.setAttribute('aria-pressed', String(selected));
      this.element.disabled = busy;
    }
  }

  /** Supply the persistent content box to the display-resolution observer. */
  get frameElement(): HTMLElement {
    return this.#frame;
  }

  /** Reserve the full canvas aspect ratio even when no target pixels can be captured. */
  frame(width: number, height: number): void {
    if (width > 0 && height > 0) this.#frame.style.aspectRatio = `${width} / ${height}`;
  }

  /** Retain the last frame during replacement; use a placeholder only before the first frame exists. */
  loading(): void {
    if (!this.#painted) this.unavailable('Loading…');
    this.#frame.setAttribute('aria-busy', 'true');
  }

  /** Replace stale pixels with concise feedback inside the same persistent frame. */
  unavailable(message: string): void {
    this.#painted = false;
    this.canvas.hidden = true;
    this.#placeholder.hidden = false;
    this.#message.textContent = message;
    this.#frame.setAttribute('aria-busy', 'false');
  }

  /** Paint interpreted pixels atomically without removing the containing preview surface. */
  paint(
    pixels: CanvasPixelRaster,
    frame: { readonly width: number; readonly height: number } | null,
  ): void {
    const bytes = decodeCanvasRaster(pixels),
      canvas = this.canvas;
    const context = canvas.getContext('2d');
    if (!context) throw new Error('Source preview rendering is unavailable.');
    const image = context.createImageData(pixels.width, pixels.height);
    if (pixels.format === 'rgba8') image.data.set(bytes);
    else
      for (let index = 0; index < bytes.length; index++) {
        const offset = index * 4,
          value = bytes[index] ?? 0;
        image.data[offset] = image.data[offset + 1] = image.data[offset + 2] = value;
        image.data[offset + 3] = 255;
      }
    if (canvas.width !== pixels.width) canvas.width = pixels.width;
    if (canvas.height !== pixels.height) canvas.height = pixels.height;
    context.putImageData(image, 0, 0);
    this.frame(frame?.width ?? pixels.width, frame?.height ?? pixels.height);
    this.#painted = true;
    canvas.hidden = false;
    this.#placeholder.hidden = true;
    this.#message.textContent = '';
    this.#frame.setAttribute('aria-busy', 'false');
  }

  /** Keep plugin previews outside the focus and command paths. */
  #container(document: Document, passive: boolean, select: () => void): HTMLElement {
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

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
/** Add out-of-flow native preview actions without replacing Comfy's controls, images, or popup identity. */
import type { CanvasRevealTarget } from '../domain/CanvasReveal.js';

export interface ComfyImageRevealDomOptions {
  target(image: HTMLImageElement): CanvasRevealTarget | null;
  reveal(target: CanvasRevealTarget): void;
  context(image: HTMLImageElement): void;
}

/** Retain action identity for unchanged thumbnails and reflow only this mounted preview. */
export class ComfyImageRevealDom {
  readonly #buttons = new Map<
    HTMLImageElement,
    { button: HTMLButtonElement; target: CanvasRevealTarget }
  >();
  readonly #resize: ResizeObserver | null;
  #frame: number | null = null;

  /** Observe native geometry independently of graph inventory and render clocks. */
  constructor(
    readonly root: HTMLElement,
    private readonly options: ComfyImageRevealDomOptions,
  ) {
    this.#resize =
      typeof ResizeObserver === 'function' ? new ResizeObserver(() => this.#schedule()) : null;
    this.#resize?.observe(root);
    root.addEventListener('contextmenu', this.#context, true);
    this.refresh();
  }

  /** Reconcile only changed media and preserve all surviving native and injected control identities. */
  refresh(): void {
    const images = new Set(this.root.querySelectorAll<HTMLImageElement>('img'));
    for (const [image, binding] of this.#buttons) {
      const target = images.has(image) ? this.options.target(image) : null;
      if (
        !target ||
        target.itemId !== binding.target.itemId ||
        target.generation !== binding.target.generation
      ) {
        binding.button.remove();
        this.#buttons.delete(image);
      }
    }
    for (const image of images) {
      if (this.#buttons.has(image)) continue;
      const target = this.options.target(image);
      if (!target) continue;
      const button = createImageRevealButton(this.root.ownerDocument, () =>
        this.options.reveal(target),
      );
      this.#buttons.set(image, { button, target });
      this.root.append(button);
    }
    this.#schedule();
  }

  /** Release scoped geometry and actions without modifying native preview layout. */
  dispose(): void {
    this.root.removeEventListener('contextmenu', this.#context, true);
    this.#resize?.disconnect();
    if (this.#frame !== null)
      this.root.ownerDocument.defaultView?.cancelAnimationFrame(this.#frame);
    for (const { button } of this.#buttons.values()) button.remove();
    this.#buttons.clear();
  }

  /** Capture exact gallery media before Comfy constructs its normal node context menu. */
  readonly #context = (event: MouseEvent): void => {
    const element = event.target instanceof Element ? event.target : null;
    const image =
      element instanceof HTMLImageElement
        ? element
        : (element?.closest('button')?.querySelector('img') ?? this.root.querySelector('img'));
    if (image instanceof HTMLImageElement) this.options.context(image);
  };

  /** Coalesce local image replacement and resize notifications into one settled geometry update. */
  #schedule(): void {
    if (this.#frame !== null) return;
    const window = this.root.ownerDocument.defaultView;
    if (!window) return;
    this.#frame = window.requestAnimationFrame(() => {
      this.#frame = null;
      this.#layout();
    });
  }

  /** Place sibling overlay buttons over native cells, never nested inside Comfy's gallery buttons. */
  #layout(): void {
    const root = this.root.getBoundingClientRect();
    const scale = this.root.offsetWidth ? root.width / this.root.offsetWidth : 1;
    if (!scale) return;
    for (const [image, { button }] of this.#buttons) {
      const holder = image.closest<HTMLElement>('button') ?? image.parentElement;
      if (!holder) continue;
      const rect = holder.getBoundingClientRect();
      const actions = holder.querySelector<HTMLElement>('.actions');
      const controls = actions?.offsetWidth ?? 0;
      button.style.left = `${Math.max(0, (rect.right - root.left) / scale - 32 - 8 - (controls ? controls + 8 : 0))}px`;
      button.style.top = `${Math.max(0, (rect.top - root.top) / scale + 8)}px`;
    }
  }
}

/** Build an accessible action using the installed host icon font and explicit event isolation. */
export function createImageRevealButton(document: Document, reveal: () => void): HTMLButtonElement {
  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'sc-image-reveal';
  button.dataset.scImageReveal = '';
  button.setAttribute('aria-label', 'Show in output canvas');
  button.title = 'Show in output canvas';
  const icon = document.createElement('i');
  icon.className = 'pi pi-window-maximize';
  icon.setAttribute('aria-hidden', 'true');
  button.append(icon);
  button.addEventListener('pointerdown', (event) => event.stopPropagation());
  button.addEventListener('click', (event) => {
    event.preventDefault();
    event.stopPropagation();
    reveal();
  });
  return button;
}

/** Install scoped hover/focus styles without affecting native preview geometry or controls. */
export function ensureImageRevealStyles(document: Document): void {
  if (document.getElementById('sc-image-reveal-styles')) return;
  const style = document.createElement('style');
  style.id = 'sc-image-reveal-styles';
  style.textContent = `.sc-image-reveal { position:absolute; z-index:4; display:grid; place-items:center; width:32px; height:32px; padding:0; border:0; border-radius:8px; background:var(--fg-color,#f2f2f2); color:var(--bg-color,#151515); box-shadow:0 2px 8px #0005; cursor:pointer; opacity:0; pointer-events:none; transition:opacity 120ms ease; }
  .image-preview:hover > .sc-image-reveal,.image-preview:focus-within > .sc-image-reveal,.sugarcubes-cube-face__preview-media figure:hover > .sc-image-reveal,.sugarcubes-cube-face__preview-media figure:focus-within > .sc-image-reveal,.sc-image-reveal:focus-visible { opacity:1;pointer-events:auto; }
  .sc-image-reveal:focus-visible { outline:2px solid var(--p-primary-color,#409eff);outline-offset:2px; }
  .sc-image-reveal-legacy {position:fixed;opacity:1;pointer-events:auto;z-index:1100;}`;
  document.head.append(style);
}

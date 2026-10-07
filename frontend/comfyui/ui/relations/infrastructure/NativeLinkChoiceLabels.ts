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
/** Adapt closed native link labels to available width while preserving full popup text and identity. */
import { NativeLinkLabelLayout } from './NativeLinkLabelLayout.js';

/** Own native label decorations while shared layout batches their measurement. */
export class NativeLinkChoiceLabels {
  readonly #resize: ResizeObserver | null;
  readonly #mutations: MutationObserver;
  #labels: readonly string[] = [];
  #selected: HTMLElement | null = null;
  #disposed = false;
  #context: CanvasRenderingContext2D | null = null;

  /** Observe native select replacement and row resizing without replacing the host control. */
  constructor(
    private readonly root: HTMLElement,
    private readonly layout: NativeLinkLabelLayout,
    private readonly measure?: (text: string, font: string) => number,
  ) {
    this.#resize = typeof ResizeObserver === 'function' ? new ResizeObserver(this.#schedule) : null;
    this.#resize?.observe(root);
    this.#mutations = new MutationObserver(this.#schedule);
    this.#mutations.observe(root, { childList: true, subtree: true, characterData: true });
  }

  /** Derive one stable width preference from the complete candidate list. */
  update(labels: readonly string[]): void {
    this.#labels = labels;
    this.#schedule();
  }

  /** Release observers and presentation annotations with the native mount. */
  dispose(): void {
    this.#disposed = true;
    this.layout.release(this.#measure);
    this.#resize?.disconnect();
    this.#mutations.disconnect();
    this.#restore();
    this.root.style.removeProperty('--sc-link-preferred-width');
  }

  /** Coalesce native replacement and resizing into one presentation pass per frame. */
  readonly #schedule = (): void => {
    if (!this.#disposed) this.layout.invalidate(this.#measure);
  };

  /** Surrender the route prefix before allowing the native text box to elide the cube name. */
  readonly #measure = (): (() => void) | null => {
    const selected = this.root.querySelector<HTMLElement>(
      '[data-testid="widget-select-default-trigger"] > span',
    );
    if (!selected) return null;
    const font = selected.hasAttribute('data-sc-link-display')
      ? selected.style.getPropertyValue('--sc-link-font')
      : getComputedStyle(selected).font;
    const available = selected.clientWidth;
    const preferred = `${Math.ceil(Math.max(60, ...this.#labels.map((label) => this.#width(label, font) + 44)))}px`;
    const text = selected.textContent ?? '';
    const slash = text.lastIndexOf('/');
    const display =
      text.startsWith('🔗 ') && slash > 3 && this.#width(text, font) > available
        ? `🔗 ${text.slice(slash + 1).trim()}`
        : null;
    return () => {
      if (this.#disposed) return;
      if (this.#selected !== selected) this.#restore();
      this.#selected = selected;
      if (this.root.style.getPropertyValue('--sc-link-preferred-width') !== preferred)
        this.root.style.setProperty('--sc-link-preferred-width', preferred);
      if (selected.title !== text) selected.title = text;
      if (display !== null) {
        if (selected.style.getPropertyValue('--sc-link-font') !== font)
          selected.style.setProperty('--sc-link-font', font);
        if (selected.dataset.scLinkDisplay !== display) selected.dataset.scLinkDisplay = display;
      } else if (selected.hasAttribute('data-sc-link-display')) {
        selected.removeAttribute('data-sc-link-display');
        selected.style.removeProperty('--sc-link-font');
      }
    };
  };

  /** Measure with the native font rather than treating route length as pixel width. */
  #width(text: string, font: string): number {
    if (this.measure) return this.measure(text, font);
    this.#context ??= this.root.ownerDocument.createElement('canvas').getContext('2d');
    if (!this.#context) throw new Error('Native link label font measurement is unavailable.');
    this.#context.font = font;
    return this.#context.measureText(text).width;
  }

  /** Restore decorations when Comfy replaces its selected label element. */
  #restore(): void {
    this.#selected?.removeAttribute('data-sc-link-display');
    this.#selected?.style.removeProperty('--sc-link-font');
    this.#selected?.removeAttribute('title');
    this.#selected = null;
  }
}

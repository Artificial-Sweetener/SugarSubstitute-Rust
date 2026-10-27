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
/** Reserve reversible scroll space without replacing the author's padding declarations. */

/** Own one scroll container's reserved space through theme, compact-layout and overflow changes. */
export class ScrollbarGutter {
  #width = 0;
  #height = 0;
  #context = '';

  /** Keep original layout declarations on their native element throughout the reservation lifetime. */
  constructor(private readonly element: HTMLElement) {}

  /** Re-read unreserved padding only when presentation changes, avoiding resize feedback loops. */
  update(width: number, height: number, context: string): void {
    if (width === this.#width && height === this.#height && context === this.#context) return;
    this.clear();
    this.#context = context;
    this.#width = width;
    this.#height = height;
    if (width === 0 && height === 0) return;
    const style = this.element.ownerDocument.defaultView?.getComputedStyle(this.element);
    this.element.style.setProperty(
      '--sc-scrollbar-base-padding-right',
      style?.paddingRight || '0px',
    );
    this.element.style.setProperty(
      '--sc-scrollbar-base-padding-bottom',
      style?.paddingBottom || '0px',
    );
    this.element.style.setProperty('--sc-scrollbar-gutter-width', `${String(width)}px`);
    this.element.style.setProperty('--sc-scrollbar-gutter-height', `${String(height)}px`);
    this.element.setAttribute('data-sc-scroll-gutter', '');
  }

  /** Restore the author's current padding rather than a stale copy from an earlier theme. */
  clear(): void {
    this.element.removeAttribute('data-sc-scroll-gutter');
    for (const property of [
      'base-padding-right',
      'base-padding-bottom',
      'gutter-width',
      'gutter-height',
    ])
      this.element.style.removeProperty(`--sc-scrollbar-${property}`);
    this.#width = 0;
    this.#height = 0;
    this.#context = '';
  }
}

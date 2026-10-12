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
/** Allocate the installed Comfy graph frame while retaining native chrome and coordinate origins. */

interface SavedStyle {
  readonly element: HTMLElement;
  readonly property: string;
  readonly value: string;
  readonly priority: string;
}

/** Move the graph origin and its complete render layers together, leaving native navigation in place. */
export class ComfyCanvasFrameAllocation {
  readonly #chrome: HTMLElement;
  readonly #styles: readonly SavedStyle[];
  #inset = 0;
  #disposed = false;

  /** Qualify a direct native chrome ancestor and the public graph canvas before changing host geometry. */
  constructor(
    private readonly root: HTMLElement,
    panel: HTMLElement,
  ) {
    const chrome = [...root.children].find((child) => child.contains(panel));
    const canvas = root.querySelector<HTMLCanvasElement>(':scope > #graph-canvas');
    if (
      root.id !== 'graph-canvas-container' ||
      !panel.matches('.graph-canvas-panel') ||
      !(chrome instanceof HTMLElement) ||
      !canvas
    )
      throw new Error('The Comfy graph workspace layout is unsupported.');
    this.#chrome = chrome;
    this.#styles = [
      ...['width', 'margin-left', 'overflow', 'clip-path'].map((property) => ({
        element: root,
        property,
      })),
      ...['left', 'width'].map((property) => ({ element: chrome, property })),
    ].map(({ element, property }) => ({
      element,
      property,
      value: element.style.getPropertyValue(property),
      priority: element.style.getPropertyPriority(property),
    }));
  }

  /** Resize actual canvas/Nodes layers through Comfy's canvas-element ResizeObserver, preserving public conversion origins. */
  apply(inset: number): void {
    if (!Number.isFinite(inset) || inset < 0)
      throw new RangeError('Graph workspace inset is invalid.');
    if (this.#disposed || this.#inset === inset) return;
    this.#inset = inset;
    if (inset === 0) {
      this.#restore();
      return;
    }
    this.root.style.setProperty('width', `calc(100% - ${inset}px)`);
    this.root.style.setProperty('margin-left', `${inset}px`);
    // The native chrome extends to the original frame; clip to that frame instead of the reduced graph box.
    this.root.style.setProperty('overflow', 'visible');
    this.root.style.setProperty('clip-path', `inset(0px 0px 0px -${inset}px)`);
    this.#chrome.style.setProperty('left', `-${inset}px`);
    this.#chrome.style.setProperty('width', `calc(100% + ${inset}px)`);
  }

  /** Restore exact inline values and priorities after graph-workspace suspension or replacement. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#restore();
  }

  /** Relinquish only properties this adapter owns, including pre-existing extension styles. */
  #restore(): void {
    for (const style of this.#styles) {
      if (style.value) style.element.style.setProperty(style.property, style.value, style.priority);
      else style.element.style.removeProperty(style.property);
    }
  }
}

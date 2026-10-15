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
/** Allocate the native graph viewport beneath chrome while retaining its mounted render layers. */

interface SavedStyle {
  readonly element: HTMLElement;
  readonly property: string;
  readonly value: string;
  readonly priority: string;
}

/** Own graph bounds for native rendering and graph-pinned surfaces while leaving chrome stationary. */
export class ComfyCanvasFrameAllocation {
  readonly #chrome: HTMLElement;
  readonly #frame: HTMLElement;
  readonly #styles: readonly SavedStyle[];
  #allocation: string | null = null;
  #disposed = false;

  /** Qualify a direct native chrome ancestor and the public graph canvas before changing host geometry. */
  constructor(
    private readonly root: HTMLElement,
    private readonly panel: HTMLElement,
  ) {
    const chrome = [...root.children].find((child) => child.contains(panel));
    const canvas = root.querySelector<HTMLCanvasElement>(':scope > #graph-canvas');
    const frame = root.parentElement;
    if (
      root.id !== 'graph-canvas-container' ||
      !panel.matches('.graph-canvas-panel') ||
      !(chrome instanceof HTMLElement) ||
      !canvas ||
      !frame?.matches('.comfyui-body')
    )
      throw new Error('The Comfy graph workspace layout is unsupported.');
    this.#chrome = chrome;
    this.#frame = frame;
    this.#styles = [
      { element: frame, property: 'overflow' },
      { element: panel, property: '--sc-output-workspace-reserved-width' },
      ...['width', 'height', 'margin-left', 'margin-top', 'overflow', 'clip-path'].map(
        (property) => ({
          element: root,
          property,
        }),
      ),
      ...['left', 'width', 'top', 'height'].map((property) => ({ element: chrome, property })),
    ].map(({ element, property }) => ({
      element,
      property,
      value: element.style.getPropertyValue(property),
      priority: element.style.getPropertyPriority(property),
    }));
  }

  /** Resize actual canvas/Nodes layers to the usable panel through Comfy's canvas ResizeObserver. */
  apply(inset: number): void {
    if (!Number.isFinite(inset) || inset < 0)
      throw new RangeError('Graph workspace inset is invalid.');
    if (this.#disposed) return;
    const frame = this.#frame.getBoundingClientRect();
    const panel = this.panel.getBoundingClientRect();
    const measured = frame.height > 0 && panel.height > 0;
    const top = measured ? Math.max(0, Math.min(frame.height, panel.top - frame.top)) : 0;
    const bottom = measured
      ? Math.max(0, Math.min(frame.height - top, frame.bottom - panel.bottom))
      : 0;
    const measuredWidth = frame.width > 0 && panel.width > 0;
    const left = measuredWidth ? Math.max(0, Math.min(frame.width, panel.left - frame.left)) : 0;
    const right = measuredWidth
      ? Math.max(0, Math.min(frame.width - left, frame.right - panel.right))
      : 0;
    const horizontal = left + right + inset;
    const vertical = top + bottom;
    const allocation = JSON.stringify([inset, left, right, top, bottom]);
    if (this.#allocation === allocation) return;
    this.#allocation = allocation;
    if (horizontal === 0 && vertical === 0) {
      this.#restore();
      return;
    }
    this.#set(this.root, 'width', horizontal ? `calc(100% - ${horizontal}px)` : null);
    this.#set(this.panel, '--sc-output-workspace-reserved-width', inset ? `${inset}px` : null);
    this.#set(this.root, 'margin-left', horizontal ? `${left}px` : null);
    this.#set(this.root, 'margin-top', vertical ? `${top}px` : null);
    this.#set(this.root, 'height', vertical ? `calc(100% - ${vertical}px)` : null);
    // Hidden overflow permits focus-driven scrolling; the qualified outer frame must remain stationary.
    this.#set(this.#frame, 'overflow', 'clip');
    this.#set(this.root, 'overflow', 'visible');
    this.#set(
      this.root,
      'clip-path',
      `inset(${-top}px ${-(right + inset)}px ${-bottom}px ${-left}px)`,
    );
    this.#set(this.#chrome, 'left', horizontal ? `${-left}px` : null);
    this.#set(this.#chrome, 'width', horizontal ? `calc(100% + ${horizontal}px)` : null);
    this.#set(this.#chrome, 'top', vertical ? `${-top}px` : null);
    this.#set(this.#chrome, 'height', vertical ? `calc(100% + ${vertical}px)` : null);
  }

  /** Relinquish viewport allocation while Comfy owns an alternate application surface. */
  suspend(): void {
    this.#allocation = null;
    this.#restore();
  }

  /** Restore exact inline values and priorities after graph-workspace suspension or replacement. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#restore();
  }

  /** Relinquish only properties this adapter owns, including pre-existing extension styles. */
  #restore(): void {
    for (const style of this.#styles) this.#set(style.element, style.property, null);
  }

  /** Write changed geometry only, restoring exact borrowed values and priorities on release. */
  #set(element: HTMLElement, property: string, value: string | null): void {
    const original = this.#styles.find(
      (style) => style.element === element && style.property === property,
    );
    const next = value ?? original?.value ?? '';
    const priority = value === null ? (original?.priority ?? '') : '';
    if (
      element.style.getPropertyValue(property) === next &&
      element.style.getPropertyPriority(property) === priority
    )
      return;
    if (next) element.style.setProperty(property, next, priority);
    else element.style.removeProperty(property);
  }
}

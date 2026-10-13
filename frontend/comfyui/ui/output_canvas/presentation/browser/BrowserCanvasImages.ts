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
/** Present admitted image handles with one coalesced transform write and no decoded copies. */

import type { CanvasFrameClock } from '../../application/CanvasCameraTransition.js';
import type { CanvasInspectionTarget } from '../../application/CanvasInspectionStore.js';
import type { CanvasAnchor } from '../../domain/CanvasCamera.js';
import type { CanvasImageFrame } from '../../domain/CanvasImageResource.js';

export interface BrowserCanvasWipe {
  readonly split: number;
  readonly orientation: 'vertical' | 'horizontal';
}

/** Own only presentation nodes; the image-resource owner retains release authority. */
export class BrowserCanvasImages {
  readonly #primaryLayer: HTMLDivElement;
  readonly #secondaryLayer: HTMLDivElement;
  #primary: CanvasImageFrame<HTMLImageElement> | null = null;
  #secondary: CanvasImageFrame<HTMLImageElement> | null = null;
  #target: CanvasInspectionTarget | null = null;
  #wipe: BrowserCanvasWipe | null = null;
  #frame: (() => void) | null = null;
  #disposed = false;
  #unpainted = false;

  /** Create scoped clipping layers beneath a separately mounted shared overlay. */
  constructor(
    element: HTMLElement,
    private readonly clock: CanvasFrameClock,
  ) {
    const document = element.ownerDocument;
    this.#primaryLayer = document.createElement('div');
    this.#secondaryLayer = document.createElement('div');
    this.#primaryLayer.className = 'sc-output-image-layer';
    this.#secondaryLayer.className = 'sc-output-image-layer sc-output-secondary';
    element.append(this.#primaryLayer, this.#secondaryLayer);
  }

  /** Attach existing decoded elements without setting another source or decoding them again. */
  images(
    primary: CanvasImageFrame<HTMLImageElement> | null,
    secondary: CanvasImageFrame<HTMLImageElement> | null,
  ): void {
    if (this.#disposed) return;
    if (primary === this.#primary && secondary === this.#secondary) return;
    if (primary?.handle !== this.#primary?.handle) {
      this.#primaryLayer.replaceChildren(...(primary ? [primary.handle] : []));
      if (primary) primary.handle.className = 'sc-output-image';
    }
    if (secondary?.handle !== this.#secondary?.handle) {
      this.#secondaryLayer.replaceChildren(...(secondary ? [secondary.handle] : []));
      if (secondary) secondary.handle.className = 'sc-output-image';
    }
    this.#primary = primary;
    this.#secondary = secondary;
    this.#unpainted = true;
    this.#schedule();
  }

  /** Coalesce camera input without rebroadcasting catalog or source-tab state. */
  camera(target: CanvasInspectionTarget | null): void {
    if (this.#disposed) return;
    this.#target = target;
    if (!target) {
      this.#frame?.();
      this.#frame = null;
      this.#primaryLayer.hidden = true;
      this.#secondaryLayer.hidden = true;
    } else if (this.#unpainted) {
      this.#paint();
      this.#unpainted = false;
    } else this.#schedule();
  }

  /** Change clipping independently of image loading and camera inspection. */
  wipe(wipe: BrowserCanvasWipe | null): void {
    if (this.#disposed) return;
    if (wipe?.split === this.#wipe?.split && wipe?.orientation === this.#wipe?.orientation) return;
    this.#wipe = wipe;
    this.#schedule();
  }

  /** Require gestures to begin on the clipped, displayed image footprint. */
  contains(point: CanvasAnchor): boolean {
    const target = this.#target;
    if (!target || (!this.#primary && !this.#secondary)) return false;
    const { camera, geometry } = target;
    return (
      point.x >= Math.max(0, camera.x) &&
      point.y >= Math.max(0, camera.y) &&
      point.x <=
        Math.min(geometry.viewport.width, camera.x + geometry.image.width * camera.scale) &&
      point.y <= Math.min(geometry.viewport.height, camera.y + geometry.image.height * camera.scale)
    );
  }

  /** Expose owned rendering work without claiming control over the browser's frame scheduler. */
  get pendingFrames(): number {
    return this.#frame ? 1 : 0;
  }

  /** Remove presentation and scheduled work without releasing another owner's image handles. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#frame?.();
    this.#frame = null;
    this.#target = null;
    this.#primary = null;
    this.#secondary = null;
    this.#primaryLayer.remove();
    this.#secondaryLayer.remove();
  }

  /** Schedule at most one paint; an idle viewer owns no recurring callbacks. */
  #schedule(): void {
    if (this.#frame || !this.#target) return;
    this.#frame = this.clock.request(() => {
      this.#frame = null;
      if (!this.#disposed) this.#paint();
    });
  }

  /** Stretch B into the primary image's registration rectangle and clip in viewport space. */
  #paint(): void {
    const target = this.#target;
    if (!target) return;
    const { camera, geometry } = target;
    for (const [layer, frame] of [
      [this.#primaryLayer, this.#primary],
      [this.#secondaryLayer, this.#secondary],
    ] as const) {
      layer.hidden = !frame;
      if (!frame) continue;
      const image = frame.handle;
      image.style.width = `${geometry.image.width}px`;
      image.style.height = `${geometry.image.height}px`;
      image.style.transformOrigin = '0 0';
      image.style.transform = `translate(${camera.x}px, ${camera.y}px) scale(${camera.scale})`;
      const physical = camera.scale * geometry.density;
      const magnification =
        physical *
        Math.min(
          geometry.image.width / frame.dimensions.width,
          geometry.image.height / frame.dimensions.height,
        );
      image.style.imageRendering = magnification >= 2 ? 'pixelated' : 'auto';
    }
    const wipe = this.#wipe;
    this.#secondaryLayer.style.clipPath = !wipe
      ? ''
      : wipe.orientation === 'vertical'
        ? `inset(0 ${(1 - wipe.split) * 100}% 0 0)`
        : `inset(0 0 ${(1 - wipe.split) * 100}% 0)`;
  }
}

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
/** Animate short camera transitions through a cancellable frame port with no idle loop. */

import {
  anchoredCanvasCamera,
  type CanvasAnchor,
  type CanvasCamera,
  type CanvasCameraGeometry,
} from '../domain/CanvasCamera.js';

export interface CanvasFrameClock {
  now(): number;
  request(callback: (time: number) => void): () => void;
}

/** Rebase interrupted navigation from the displayed camera rather than an obsolete destination. */
export class CanvasCameraTransition {
  #cancel: (() => void) | null = null;
  #displayed: CanvasCamera | null = null;
  #lastWheel = -Infinity;
  #sequence = 0;

  /** Inject a frame clock so interpolation can be proved without browser scheduling. */
  constructor(
    private readonly clock: CanvasFrameClock,
    private readonly display: (camera: CanvasCamera) => void,
  ) {}

  /** Read the actually displayed transform for new input and resize. */
  read(): CanvasCamera | null {
    return this.#displayed;
  }

  /** Replace geometry/selection immediately while retiring an obsolete animation. */
  set(camera: CanvasCamera): void {
    this.cancel();
    this.#publish(camera);
  }

  /** Apply anchored scale and smoothly settle destination constraints. */
  move(
    target: CanvasCamera,
    geometry: CanvasCameraGeometry,
    anchor: CanvasAnchor | null,
    reducedMotion: boolean,
    wheel = false,
  ): void {
    const start = this.#displayed;
    const time = this.clock.now();
    const interval = time - this.#lastWheel;
    if (wheel) this.#lastWheel = time;
    const duration =
      reducedMotion || (wheel && interval <= 25) ? 0 : wheel && interval <= 80 ? 20 : 45;
    this.cancel();
    if (!start || duration === 0) {
      this.#publish(target);
      return;
    }
    const sequence = this.#sequence;
    const destination = anchor ? anchoredCanvasCamera(start, target.scale, anchor, geometry) : null;
    const frame = (now: number): void => {
      if (sequence !== this.#sequence) return;
      this.#cancel = null;
      const elapsed = Math.max(0, Math.min(1, (now - time) / duration));
      if (elapsed === 1) {
        this.#publish(target);
        return;
      }
      const progress = 1 - Math.pow(1 - elapsed, 3);
      const scale = start.scale * Math.pow(target.scale / start.scale, progress);
      const anchored = anchor ? anchoredCanvasCamera(start, scale, anchor, geometry) : null;
      const centeredX = (geometry.viewport.width - geometry.image.width * scale) / 2;
      const centeredY = (geometry.viewport.height - geometry.image.height * scale) / 2;
      const panStartX =
        start.x - (geometry.viewport.width - geometry.image.width * start.scale) / 2;
      const panStartY =
        start.y - (geometry.viewport.height - geometry.image.height * start.scale) / 2;
      const panEndX =
        target.x - (geometry.viewport.width - geometry.image.width * target.scale) / 2;
      const panEndY =
        target.y - (geometry.viewport.height - geometry.image.height * target.scale) / 2;
      this.#publish(
        Object.freeze({
          scale,
          mode: 'custom',
          x:
            anchored && destination
              ? anchored.x + (target.x - destination.x) * progress
              : centeredX + panStartX * (1 - progress) + panEndX * progress,
          y:
            anchored && destination
              ? anchored.y + (target.y - destination.y) * progress
              : centeredY + panStartY * (1 - progress) + panEndY * progress,
        }),
      );
      if (sequence === this.#sequence) this.#cancel = this.clock.request(frame);
    };
    this.#cancel = this.clock.request(frame);
  }

  /** Invalidate delayed callbacks and release the only pending frame. */
  cancel(): void {
    this.#sequence++;
    this.#cancel?.();
    this.#cancel = null;
  }

  /** Release frame and camera references at attachment teardown. */
  dispose(): void {
    this.cancel();
    this.#displayed = null;
  }

  /** Publish only accepted frames, retaining no destination render loop. */
  #publish(camera: CanvasCamera): void {
    this.#displayed = camera;
    this.display(camera);
  }
}

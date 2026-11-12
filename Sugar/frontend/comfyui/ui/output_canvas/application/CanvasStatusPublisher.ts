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
/** Bound camera/preview status publication without polling idle or hidden views. */

import type { ExecutionClock } from '../../execution/domain/WorkflowExecutionEvent.js';

/** Publish discrete navigation immediately and continuous visual changes at most ten times per second. */
export class CanvasStatusPublisher {
  #last = -Infinity;
  #cancel: (() => void) | null = null;
  #disposed = false;

  /** Inject monotonic time and one snapshot-producing collaborator. */
  constructor(
    private readonly clock: ExecutionClock,
    private readonly publish: () => void,
  ) {}

  /** Coalesce pending status while preserving immediate selection/error transitions. */
  changed(immediate = false): void {
    if (this.#disposed) return;
    const wait = this.#last + 100 - this.clock.now();
    if (immediate || wait <= 0) {
      this.#cancel?.();
      this.#cancel = null;
      this.#last = this.clock.now();
      this.publish();
    } else if (!this.#cancel) {
      this.#cancel = this.clock.later(wait, () => {
        this.#cancel = null;
        if (this.#disposed) return;
        this.#last = this.clock.now();
        this.publish();
      });
    }
  }

  /** Cancel delayed status before destroying an attachment. */
  dispose(): void {
    this.#disposed = true;
    this.#cancel?.();
    this.#cancel = null;
  }
}

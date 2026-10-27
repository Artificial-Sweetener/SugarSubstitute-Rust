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
/** Bind optional native observation to one authenticated server lifetime. */
import type {
  CanvasChangeObservation,
  CanvasChangeSubscription,
  CanvasObservationStatus,
} from '../contracts/CanvasChangeObservation.js';

/** Adapt a synchronous metadata observer without adding another native polling owner. */
export function nativeCanvasChanges<Change>(
  subscribe: (listener: (change: Change) => void) => (() => void) | null,
): CanvasChangeObservation<Change> {
  return {
    subscribe: (listener) => {
      const stop = subscribe(listener);
      return {
        ready: Promise.resolve(stop ? 'observing' : 'unavailable'),
        stop: stop ?? (() => {}),
      };
    },
  };
}

/** Invalidate late native callbacks before stopping or replacing their subscription. */
export class CanvasChangeRelay<Change> {
  #lease: CanvasChangeSubscription | null = null;
  #epoch = 0;
  #disposed = false;
  /** Keep native capability and wire envelope translation with their existing adapter owners. */
  constructor(
    private readonly observation: CanvasChangeObservation<Change> | null,
    private readonly publish: (change: Change) => void,
  ) {}
  /** Negotiate actual observation support independently of source or mask capture support. */
  async watch(enabled: boolean): Promise<CanvasObservationStatus> {
    if (this.#disposed) return 'unavailable';
    if (!enabled) {
      this.#release();
      return this.observation ? 'observing' : 'unsupported';
    }
    if (!this.observation) return 'unsupported';
    if (!this.#lease) {
      const epoch = ++this.#epoch;
      this.#lease = this.observation.subscribe((change) => {
        if (!this.#disposed && epoch === this.#epoch) this.publish(change);
      });
    }
    return await this.#lease.ready;
  }
  /** Release native resources without allowing callbacks into a retired navigation. */
  dispose(): void {
    this.#disposed = true;
    this.#release();
  }
  /** Fence first so a native unsubscribe callback cannot publish during cleanup. */
  #release(): void {
    this.#epoch++;
    this.#lease?.stop();
    this.#lease = null;
  }
}

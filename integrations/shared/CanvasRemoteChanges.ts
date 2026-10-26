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
/** Share one negotiated remote watch without assuming that capture implies native observation. */
import type {
  CanvasChangeObservation,
  CanvasChangeSubscription,
  CanvasObservationStatus,
} from '../contracts/CanvasChangeObservation.js';

/** Borrow one authenticated transport and its validated watch acknowledgement. */
export interface CanvasRemoteChangesPort<Change> {
  watch(enabled: boolean): Promise<boolean>;
  listen(listener: (change: Change) => void): () => void;
  report(error: unknown): void;
}

/** Fence obsolete acknowledgements, coalesce listeners and release a pending watch on teardown. */
export class CanvasRemoteChanges<Change> implements CanvasChangeObservation<Change> {
  readonly #listeners = new Set<(change: Change) => void>();
  #ready: Promise<CanvasObservationStatus> | null = null;
  #stop: (() => void) | null = null;
  #epoch = 0;
  #accepted = false;
  #buffer: Change[] = [];
  #overflow = false;
  #watchSent = false;
  #disposed = false;
  /** Keep protocol-specific requests and native event envelopes outside this lifetime owner. */
  constructor(private readonly port: CanvasRemoteChangesPort<Change>) {}

  /** Register before starting the watch so synchronous native notifications cannot escape the lease. */
  subscribe(listener: (change: Change) => void): CanvasChangeSubscription {
    if (this.#disposed) return { ready: Promise.resolve('unavailable'), stop: () => {} };
    this.#listeners.add(listener);
    const ready = this.#ready ?? this.#start();
    let stopped = false;
    return {
      ready,
      stop: () => {
        if (stopped) return;
        stopped = true;
        this.#listeners.delete(listener);
        if (!this.#listeners.size) this.#release();
      },
    };
  }

  /** Stop accepting events before releasing the session's transport. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#listeners.clear();
    this.#release();
  }

  /** Retain bounded early notifications without losing changes to different masks or documents. */
  #start(): Promise<CanvasObservationStatus> {
    const epoch = ++this.#epoch;
    this.#stop = this.port.listen((change) => {
      if (!this.#owns(epoch)) return;
      if (this.#accepted) this.#publish(change);
      else if (this.#buffer.length < 64) this.#buffer.push(change);
      else this.#overflow = true;
    });
    this.#watchSent = true;
    const ready = this.port.watch(true).then(
      (supported): CanvasObservationStatus => {
        if (!this.#owns(epoch)) return 'unavailable';
        const buffered = this.#buffer;
        this.#buffer = [];
        if (supported && this.#overflow) {
          this.#stop?.();
          this.#stop = null;
          this.#stopWatch();
          this.port.report(
            new Error(
              'Canvas changes exceeded the pending observation budget. Refresh the canvas sources.',
            ),
          );
          return 'unavailable';
        }
        this.#accepted = supported;
        if (supported)
          for (const change of buffered) {
            if (!this.#owns(epoch)) break;
            this.#publish(change);
          }
        if (!supported) {
          this.#stop?.();
          this.#stop = null;
        }
        return supported ? 'observing' : 'unsupported';
      },
      (error: unknown): CanvasObservationStatus => {
        if (this.#owns(epoch)) {
          this.#buffer = [];
          this.#stop?.();
          this.#stop = null;
          this.#stopWatch();
          this.port.report(error);
        }
        return 'unavailable';
      },
    );
    this.#ready = ready;
    return ready;
  }

  /** Notify only consumers still mounted in the current observation lifetime. */
  #publish(change: Change): void {
    for (const listener of this.#listeners) listener(change);
  }
  /** Re-read ownership after consumer callbacks may dispose or replace the observation lifetime. */
  #owns(epoch: number): boolean {
    return !this.#disposed && epoch === this.#epoch;
  }

  /** Send the matching stop even when its start acknowledgement has not arrived yet. */
  #release(): void {
    if (!this.#ready) return;
    this.#epoch++;
    this.#ready = null;
    this.#accepted = false;
    this.#buffer = [];
    this.#overflow = false;
    this.#stop?.();
    this.#stop = null;
    this.#stopWatch();
  }
  /** Send exactly one stop for each admitted native watch, including an overflowing start. */
  #stopWatch(): void {
    if (!this.#watchSent) return;
    this.#watchSent = false;
    void this.port.watch(false).catch((error: unknown) => {
      if (!this.#disposed) this.port.report(error);
    });
  }
}

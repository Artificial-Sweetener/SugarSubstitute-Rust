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
/** One finite latest-wins lane settles retired work even when an external provider ignores cancellation. */

/** Expected retirement never masquerades as a successful external result. */
export type LatestOutcome<T> =
  | { readonly status: 'ready'; readonly value: T }
  | { readonly status: 'cancelled' | 'disposed' };

/** An active request owns cancellation and settlement, without retaining a service observer or document. */
interface ActiveRequest {
  readonly cancellation: AbortController;
  readonly retire: (status: 'cancelled' | 'disposed') => void;
}

/** Supersede one active external request; late results/errors are consumed without publication into a retired lifetime. */
export class LatestRequest {
  private active: ActiveRequest | undefined;
  private generation = 0;
  private disposed = false;

  /** Start only the latest invocation, including reentrant submissions from host abort listeners. */
  run<T>(work: (signal: AbortSignal) => Promise<T>): Promise<LatestOutcome<T>> {
    if (this.disposed) return Promise.resolve({ status: 'disposed' });
    const generation = ++this.generation;
    this.retire('cancelled');
    if (this.disposed) return Promise.resolve({ status: 'disposed' });
    if (generation !== this.generation) return Promise.resolve({ status: 'cancelled' });
    return new Promise((resolve, reject) => {
      const active: ActiveRequest = {
        cancellation: new AbortController(),
        retire: (status) => resolve({ status }),
      };
      this.active = active;
      /** Only the currently owned request may settle with external data. */
      const complete = (value: T): void => {
        if (this.active !== active) return;
        this.active = undefined;
        resolve({ status: 'ready', value });
      };
      /** Unexpected current failures remain observable; retired failures cannot poison a newer request. */
      const fail = (cause: unknown): void => {
        if (this.active !== active) return;
        this.active = undefined;
        // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors -- Preserve the external host's original thrown value across this asynchronous boundary.
        reject(cause);
      };
      try {
        void work(active.cancellation.signal).then(complete, fail);
      } catch (cause) {
        fail(cause);
      }
    });
  }

  /** Retire the current operation before invoking host cancellation callbacks. */
  cancel(): void {
    ++this.generation;
    this.retire('cancelled');
  }

  /** Settle owned work immediately and prohibit new external invocation after disposal. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    ++this.generation;
    this.retire('disposed');
  }

  /** Release the lane before aborting, so synchronous host reentrancy cannot revive the old request. */
  private retire(status: 'cancelled' | 'disposed'): void {
    const active = this.active;
    this.active = undefined;
    active?.retire(status);
    active?.cancellation.abort();
  }
}

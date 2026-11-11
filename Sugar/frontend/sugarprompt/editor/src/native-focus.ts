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
/** Finite readiness owns measurement requests, a bounded deadline and failure cleanup for one native view. */
import { EditorView } from '@codemirror/view';

/** Injectable clocks let owner tests exercise deadline settlement without elapsed-time sleeps. */
export interface FocusClock {
  /** Allocate one finite readiness deadline. */
  set(callback: () => void, milliseconds: number): unknown;
  /** Release the exact owned deadline on all terminal paths. */
  clear(handle: unknown): void;
}
/** Default timers retain no geometry or deferred focus operation. */
const nativeClock: FocusClock = {
  set: (callback, milliseconds) => setTimeout(callback, milliseconds),
  clear: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
};
/** One readiness lifetime owns all pending measurement and deadline work. */
interface FocusRequest {
  readonly promise: Promise<boolean>;
  readonly settle: (ready: boolean) => void;
  deadline: unknown;
  passes: number;
  readonly viewport: 'reveal-selection' | 'preserve-viewport';
  readonly signal: AbortSignal | undefined;
  readonly abort: () => void;
}
/** Current native selection and geometry remain CodeMirror-owned. */
export class NativeFocus {
  private readonly view: EditorView;
  private readonly clock: FocusClock;
  private pending: FocusRequest | undefined;
  private disposed = false;
  /** Bind one per-view lifetime and its finite timer adapter. */
  constructor(view: EditorView, clock: FocusClock = nativeClock) {
    this.view = view;
    this.clock = clock;
  }
  /** Coalesce matching viewport/cancellation identity; an abort retires only its exact request and spatial restoration preserves the native scroll anchor. */
  focus(
    viewport: FocusRequest['viewport'] = 'reveal-selection',
    signal?: AbortSignal,
  ): Promise<boolean> {
    if (viewport !== 'reveal-selection' && viewport !== 'preserve-viewport')
      throw new TypeError('Invalid focus viewport intent');
    if (this.disposed || signal?.aborted) return Promise.resolve(false);
    if (
      this.pending &&
      this.view.hasFocus &&
      this.pending.viewport === viewport &&
      this.pending.signal === signal
    )
      return this.pending.promise;
    this.retire();
    let settle!: (ready: boolean) => void;
    const promise = new Promise<boolean>((resolve) => {
      settle = resolve;
    });
    const request: FocusRequest = {
      promise,
      settle,
      deadline: undefined,
      passes: 0,
      viewport,
      signal,
      abort: () => this.finish(request, false),
    };
    this.pending = request;
    try {
      request.deadline = this.clock.set(() => this.finish(request, false), 1000);
      if (!this.current(request)) {
        this.clock.clear(request.deadline);
        return promise;
      }
      signal?.addEventListener('abort', request.abort, { once: true });
      if (signal?.aborted) this.finish(request, false);
      if (!this.current(request)) return promise;
      const active = this.view.root.activeElement;
      if (viewport === 'reveal-selection')
        this.view.dispatch({
          effects: EditorView.scrollIntoView(this.view.state.selection.main.head),
        });
      if (!this.current(request)) return promise;
      const destination = this.view.root.activeElement;
      if (destination !== active && destination && !this.view.dom.contains(destination)) {
        this.finish(request, false);
        return promise;
      }
      this.view.focus();
      if (this.current(request)) this.measure(request);
    } catch (cause) {
      this.fail(request, cause);
    }
    return promise;
  }
  /** Only the current mounted request can schedule or publish readiness. */
  private current(request: FocusRequest): boolean {
    return !this.disposed && this.pending === request;
  }
  /** At most eight native read/write passes include final reveal verification. */
  private measure(request: FocusRequest, previousHeight?: number, revealed = false): void {
    if (!this.current(request)) return;
    if (++request.passes > 8) {
      this.finish(request, false);
      return;
    }
    try {
      this.view.requestMeasure({
        key: this,
        /** Read geometry only during an admitted native measurement. */
        read: () => {
          try {
            return this.current(request)
              ? {
                  focused: this.view.hasFocus,
                  height: this.view.scrollDOM.clientHeight,
                }
              : null;
          } catch (cause) {
            this.fail(request, cause);
          }
        },
        /** Recheck host intent after layout publication before revealing current native selection. */
        write: (measured) =>
          queueMicrotask(() => {
            if (!this.current(request)) return;
            try {
              if (!measured?.focused || !this.view.hasFocus) {
                this.finish(request, false);
                return;
              }
              if (previousHeight !== measured.height) this.measure(request, measured.height);
              else if (revealed || request.viewport === 'preserve-viewport')
                this.finish(request, true);
              else {
                this.view.dispatch({
                  effects: EditorView.scrollIntoView(this.view.state.selection.main.head),
                });
                if (this.current(request)) this.measure(request, measured.height, true);
              }
            } catch (cause) {
              this.fail(request, cause);
            }
          }),
      });
    } catch (cause) {
      this.fail(request, cause);
    }
  }
  /** Cleanup cannot replace an invariant failure, including a thrown undefined value. */
  private fail(request: FocusRequest, cause: unknown): never {
    try {
      this.finish(request, false);
    } catch (cleanup) {
      throw new AggregateError([cause, cleanup], 'Native focus failure cleanup failed');
    }
    throw cause;
  }
  /** Release deadline ownership before settling once; stale native callbacks remain inert. */
  private finish(request: FocusRequest, ready: boolean): void {
    if (this.pending !== request) return;
    this.pending = undefined;
    const failures: unknown[] = [];
    try {
      try {
        request.signal?.removeEventListener('abort', request.abort);
      } catch (cause) {
        failures.push(cause);
      }
      try {
        this.clock.clear(request.deadline);
      } catch (cause) {
        failures.push(cause);
      }
    } finally {
      request.settle(ready);
    }
    if (failures.length > 1) throw new AggregateError(failures, 'Native focus cleanup failed');
    if (failures.length === 1) throw failures[0];
  }
  /** Baseline, supersession and focus departure release all owned readiness work. */
  retire(): void {
    if (this.pending) this.finish(this.pending, false);
  }
  /** Teardown settles work before native queues disappear. */
  dispose(): void {
    this.disposed = true;
    this.retire();
  }
}

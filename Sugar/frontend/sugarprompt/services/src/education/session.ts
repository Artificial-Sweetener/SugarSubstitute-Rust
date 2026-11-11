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
/** A finite one-time education claim publishes only acknowledged, current-lifetime presentation. */
import { requireValue } from '../../../core/src/required.js';
import { implicitWeight } from '../../../core/src/index.js';
import type {
  EducationClaim,
  EducationOutcome,
  EducationSnapshot,
  NestedParenthesisTip,
  ParenthesisEducationAdapter,
} from './contracts.js';
/** Own one coalesced acknowledgement request; the host remains the durable seen-flag authority. */
export class ParenthesisEducationSession {
  private readonly adapter: ParenthesisEducationAdapter | undefined;
  private readonly observe: (snapshot: EducationSnapshot) => void;
  private current: EducationSnapshot;
  private cancellation: AbortController | undefined;
  private active: Promise<EducationOutcome> | undefined;
  private disposed = false;
  /** Bind one stable host adapter and an observer for this finite component lifetime. */
  constructor(
    adapter: ParenthesisEducationAdapter | undefined,
    observe: (snapshot: EducationSnapshot) => void = () => {},
  ) {
    this.adapter = adapter;
    this.observe = observe;
    this.current = Object.freeze({
      status: adapter ? 'idle' : 'unavailable',
      tip: null,
    });
  }
  /** Observe immutable prepared presentation without exposing a persistence queue or source buffer. */
  snapshot(): EducationSnapshot {
    return this.current;
  }
  /** Only authored nesting qualifies; concurrent conversions share the first claim and its exact multiplier. */
  authored(nestingDepth: number): Promise<EducationOutcome> {
    if (!Number.isSafeInteger(nestingDepth) || nestingDepth < 1)
      throw new RangeError('Invalid authored nesting depth');
    if (this.disposed) return Promise.resolve({ status: 'disposed' });
    if (nestingDepth < 2 || this.current.status === 'visible' || this.current.status === 'seen')
      return Promise.resolve({ status: 'suppressed' });
    if (this.active) return this.active;
    if (!this.adapter) return Promise.resolve({ status: 'unavailable' });
    const tip = Object.freeze({
      nestingDepth,
      weight: implicitWeight(nestingDepth),
    });
    const cancellation = new AbortController();
    this.cancellation = cancellation;
    this.publish(Object.freeze({ status: 'claiming', tip: null }));
    if (this.disposed || this.cancellation !== cancellation)
      return Promise.resolve({
        status: this.disposed ? 'disposed' : 'cancelled',
      });
    this.active = this.prepare(tip, cancellation);
    return this.active;
  }
  /** Closing an acknowledged tip preserves its seen state and never changes editor history. */
  dismiss(): void {
    if (!this.disposed && this.current.status === 'visible')
      this.publish(Object.freeze({ status: 'seen', tip: null }));
  }
  /** Baseline, capability or read-only replacement closes presentation and retires unacknowledged work. */
  retire(): void {
    if (this.disposed) return;
    this.cancellation?.abort();
    this.cancellation = undefined;
    this.active = undefined;
    if (this.current.status === 'visible') this.dismiss();
    else if (this.current.status === 'claiming')
      this.publish(Object.freeze({ status: 'idle', tip: null }));
  }
  /** Abort and settle owned work; released callbacks cannot publish into a dead component. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.cancellation?.abort();
    this.cancellation = undefined;
    this.active = undefined;
    this.current = Object.freeze({ status: 'disposed', tip: null });
  }
  /** Validate the external acknowledgement before presenting; unexpected host failures remain observable. */
  private async prepare(
    tip: NestedParenthesisTip,
    cancellation: AbortController,
  ): Promise<EducationOutcome> {
    try {
      const result = await this.request(cancellation.signal);
      if (this.disposed) return { status: 'disposed' };
      if (cancellation.signal.aborted || this.cancellation !== cancellation)
        return { status: 'cancelled' };
      if (
        !result ||
        typeof result !== 'object' ||
        Object.keys(result).join() !== 'status' ||
        !['claimed', 'seen', 'unavailable'].includes(result.status)
      )
        throw new TypeError('Invalid education claim outcome');
      if (result.status === 'claimed') {
        this.publish(Object.freeze({ status: 'visible', tip }));
        return { status: 'shown' };
      }
      this.publish(Object.freeze({ status: result.status, tip: null }));
      return {
        status: result.status === 'seen' ? 'suppressed' : 'unavailable',
      };
    } catch (cause) {
      if (this.disposed) return { status: 'disposed' };
      if (cancellation.signal.aborted) return { status: 'cancelled' };
      this.publish(Object.freeze({ status: 'failed', tip: null }));
      throw cause;
    } finally {
      if (this.cancellation === cancellation) {
        this.active = undefined;
        this.cancellation = undefined;
      }
    }
  }
  /** Settle owned claims even if a faulty host ignores abort; late results release their listener without publishing. */
  private request(signal: AbortSignal): Promise<EducationClaim | null> {
    if (signal.aborted) return Promise.resolve(null);
    return new Promise((resolve, reject) => {
      /** Cancellation ends this request independently of the host's completion. */
      const abort = (): void => {
        resolve(null);
      };
      signal.addEventListener('abort', abort, { once: true });
      /** Accept boundary data only for later closed-shape validation. */
      const complete = (result: EducationClaim): void => {
        signal.removeEventListener('abort', abort);
        resolve(result);
      };
      /** Preserve internal causes until the recovering/reporting Vue boundary. */
      const failure = (cause: unknown): void => {
        signal.removeEventListener('abort', abort);
        if (signal.aborted) resolve(null);
        // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors -- Preserve the external host's original thrown value across this asynchronous boundary.
        else reject(cause);
      };
      try {
        void requireValue(this.adapter, 'session').claim(signal).then(complete, failure);
      } catch (cause) {
        failure(cause);
      }
    });
  }
  /** A single finite owner publishes prepared state; prompt content is never part of these observations. */
  private publish(snapshot: EducationSnapshot): void {
    if (!this.disposed) {
      this.current = snapshot;
      this.observe(snapshot);
    }
  }
}

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
/** One mounted search lifetime issues current host work and cancels superseded reveals. */
import type {
  SearchContext,
  SearchRequest,
  SearchRevealAdapter,
  SearchHostIntent,
} from './contracts.js';
import { captureSearchContext, captureSearchRevealAdapter } from './capture.js';

/** Service orchestration stores identities only; host callbacks may synchronously issue newer intent. */
export class SearchSession {
  private request: SearchRequest | null = null;
  private pending: AbortController | null = null;
  private disposed = false;
  private generation = 0;
  private readonly refresh: (request: SearchRequest) => void;
  private readonly intent: (intent: SearchHostIntent) => void;

  /** Observe one finite component's recomputation and host visibility boundary. */
  constructor(
    refresh: (request: SearchRequest) => void,
    intent: (intent: SearchHostIntent) => void,
  ) {
    this.refresh = refresh;
    this.intent = intent;
  }

  /** Current request remains immutable until source or query/field identity changes. */
  current(): SearchRequest | null {
    return this.request;
  }

  /** Only a genuine issued nonnull request can match the current source/query lifetime. */
  accepts(request: SearchRequest): boolean {
    return !this.disposed && this.request !== null && request === this.request;
  }

  /** Detach the captured controller before abort listeners can install newer work. */
  private abortPending(): void {
    const pending = this.pending;
    this.pending = null;
    pending?.abort();
  }

  /** Newer synchronous intent wins at every cancellation and host visibility boundary. */
  setContext(context: SearchContext | null, revision: number): SearchRequest | null {
    if (this.disposed) throw new Error('Search session disposed');
    const generation = ++this.generation;
    const captured = captureSearchContext(
      context,
      () => !this.disposed && generation === this.generation,
    );
    if (captured.status === 'stale') return this.request;
    this.abortPending();
    if (this.disposed || generation !== this.generation) return this.request;
    const previous = this.request;
    this.request = captured.value
      ? Object.freeze({
          fieldId: captured.value.fieldId,
          queryId: captured.value.queryId,
          sourceRevision: revision,
          identity: Symbol(),
        })
      : null;
    const current = this.request;
    if (current) {
      if (previous && previous.fieldId !== current.fieldId)
        this.intent(Object.freeze({ kind: 'clear', fieldId: previous.fieldId }));
      if (!this.accepts(current)) return this.request;
      this.intent(Object.freeze({ kind: 'filter', request: current }));
      if (this.accepts(current))
        queueMicrotask(() => {
          if (this.accepts(current)) this.refresh(current);
        });
    } else if (previous) this.intent(Object.freeze({ kind: 'clear', fieldId: previous.fieldId }));
    return this.request;
  }

  /** Source/context retirement creates a fresh ticket; cancellation reentry may supersede that request. */
  retire(revision: number): void {
    if (this.request) this.setContext(this.request, revision);
  }

  /** Reveal waits on the host layout boundary; detached cancellation settles ignored aborts. */
  async reveal(
    request: SearchRequest,
    matchId: string,
    adapter: SearchRevealAdapter | null,
  ): Promise<'ready' | 'unavailable' | 'cancelled'> {
    if (!this.accepts(request)) return 'cancelled';
    const generation = ++this.generation;
    this.abortPending();
    if (generation !== this.generation || !this.accepts(request)) return 'cancelled';
    const controller = new AbortController();
    this.pending = controller;
    const current = (): boolean =>
      generation === this.generation &&
      this.accepts(request) &&
      this.pending === controller &&
      !controller.signal.aborted;
    let abort!: () => void;
    const cancelled = new Promise<'cancelled'>((resolve) => {
      abort = () => resolve('cancelled');
      controller.signal.addEventListener('abort', abort, { once: true });
    });
    const intent = Object.freeze({ kind: 'reveal' as const, request, matchId });
    try {
      this.intent(intent);
      if (!current()) return 'cancelled';
      const captured = captureSearchRevealAdapter(adapter, current);
      if (captured.status === 'stale') return 'cancelled';
      const outcome = captured.value
        ? await Promise.race([captured.value.reveal(intent, controller.signal), cancelled])
        : 'ready';
      if (!current()) return 'cancelled';
      if (outcome !== 'ready' && outcome !== 'unavailable' && outcome !== 'cancelled')
        throw new TypeError('Invalid search reveal outcome');
      return outcome;
    } finally {
      controller.signal.removeEventListener('abort', abort);
      if (this.pending === controller) this.pending = null;
    }
  }

  /** Cancelling old work preserves any newer controller created by its synchronous abort listener. */
  cancel(): void {
    this.generation++;
    this.abortPending();
  }

  /** Retire admission before invoking external abort listeners so teardown cannot revive work. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.request = null;
    this.generation++;
    this.abortPending();
  }
}

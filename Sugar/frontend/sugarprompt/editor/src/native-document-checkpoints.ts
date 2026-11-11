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
/** One mounted editor authenticates document handles and owns finite measured capture lifetimes. */
import { requireValue } from '../../core/src/required.js';
import { Transaction } from '@codemirror/state';
import { isolateHistory } from '@codemirror/commands';
import type { EditorView } from '@codemirror/view';
import type { FocusClock } from './native-focus.js';
import {
  nativeCompositionStarted,
  nativeInputSettlement,
  type CommandInputSettlement,
} from './input-settlement.js';
import {
  nativeDocumentAuthority,
  documentAuthorityMatches,
  type NativeDocumentAuthority,
  documentSeedSource,
  type NativeDocumentSeed,
} from './document-state.js';
import type {
  DocumentCheckpoint,
  DocumentCaptureOutcome,
  DocumentCheckpointRejection,
} from './document-checkpoint.js';
import { documentCheckpointBrand } from './document-checkpoint.js';
/** Read genuine cancellation after observable signal accessors, without accepting a stale caller snapshot. */
const platformAborted = requireValue(
  requireValue(
    Object.getOwnPropertyDescriptor(AbortSignal.prototype, 'aborted'),
    'native-document-checkpoints',
  ).get,
  'native-document-checkpoints',
) as (this: AbortSignal) => boolean;
/** Resolve current native authority rather than retaining an obsolete view or state. */
export interface DocumentCheckpointOwner {
  readonly view: () => EditorView;
  readonly alive: () => boolean;
}
/** A prepared native document becomes a public handle only after every owned release and final freshness check. */
type CaptureCompletion =
  | Exclude<
      DocumentCaptureOutcome,
      {
        readonly status: 'captured';
      }
    >
  | {
      readonly status: 'ready';
      readonly authority: NativeDocumentAuthority;
    };
/** One capture owns its command reservation, abort listener, deadline and terminal outcome. */
interface CaptureRequest {
  readonly generation: number;
  readonly view: EditorView;
  readonly resolve: (outcome: DocumentCaptureOutcome) => void;
  readonly reject: (cause: unknown) => void;
  readonly signal: AbortSignal | undefined;
  readonly abort: () => void;
  initializing: boolean;
  input?: CommandInputSettlement;
  deadline?: {
    readonly handle: unknown;
  };
  listener: boolean;
  terminal?: CaptureCompletion;
  released: boolean;
  failed: boolean;
  cause?: unknown;
}
/** Native one-second readiness bound matches the existing measured focus lifetime. */
const clock: FocusClock = {
  set: (callback, milliseconds) => setTimeout(callback, milliseconds),
  clear: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
};
/** Same-editor authentication retains native fields only until explicit release or controller disposal. */
export class NativeDocumentCheckpoints {
  private readonly owner: DocumentCheckpointOwner;
  private readonly clock: FocusClock;
  private readonly seeds = new Map<DocumentCheckpoint, NativeDocumentSeed>();
  private readonly issued = new WeakSet<DocumentCheckpoint>();
  private pending: CaptureRequest | undefined;
  private disposed = false;
  private generation = 0;
  /** Capture the mounted lifetime and its injectable finite clock, never an EditorState. */
  constructor(owner: DocumentCheckpointOwner, captureClock: FocusClock = clock) {
    this.owner = owner;
    this.clock = captureClock;
  }
  /** Authenticate a handle without invoking caller properties or modifying current native authority. */
  inspect(checkpoint: DocumentCheckpoint): NativeDocumentSeed | DocumentCheckpointRejection {
    if (this.disposed || !this.owner.alive() || this.disposed) return 'disposed';
    if (!this.issued.has(checkpoint)) return 'foreign-checkpoint';
    return this.seeds.get(checkpoint) ?? 'released-checkpoint';
  }
  /** Host/source intent retires only the pending capture; already captured document handles remain usable. */
  supersede(): void {
    this.generation++;
    if (this.pending) this.finish(this.pending, { status: 'rejected', reason: 'superseded' });
  }
  /** Revoke every retained document before native teardown and settle outstanding measurement work. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.generation++;
    this.seeds.clear();
    if (this.pending) this.finish(this.pending, { status: 'rejected', reason: 'disposed' });
  }
  /** Settle accepted native input at a measured command boundary, close its history group and retain immutable fields. */
  async capture(signal?: AbortSignal): Promise<DocumentCaptureOutcome> {
    if (this.disposed || !this.owner.alive())
      return Promise.resolve({ status: 'rejected', reason: 'disposed' });
    if (signal?.aborted) return Promise.resolve({ status: 'rejected', reason: 'cancelled' });
    const generation = ++this.generation;
    if (this.pending) this.finish(this.pending, { status: 'rejected', reason: 'superseded' });
    if (generation !== this.generation)
      return Promise.resolve({ status: 'rejected', reason: 'superseded' });
    const view = this.owner.view();
    if (nativeCompositionStarted(view))
      return Promise.resolve({ status: 'rejected', reason: 'composing' });
    if (this.disposed || !this.owner.alive())
      return Promise.resolve({ status: 'rejected', reason: 'disposed' });
    if (generation !== this.generation || this.owner.view() !== view)
      return Promise.resolve({ status: 'rejected', reason: 'superseded' });
    let resolve!: CaptureRequest['resolve'], reject!: CaptureRequest['reject'];
    const promise = new Promise<DocumentCaptureOutcome>((accept, fail) => {
      resolve = accept;
      reject = fail;
    });
    void promise.catch(() => {});
    const request: CaptureRequest = {
      generation,
      view,
      resolve,
      reject,
      signal,
      abort: () => this.finish(request, { status: 'rejected', reason: 'cancelled' }),
      initializing: true,
      listener: false,
      failed: false,
      released: false,
    };
    this.pending = request;
    try {
      const state = view.state;
      const authority = nativeDocumentAuthority(state);
      request.input = state.facet(nativeInputSettlement).reserveCommand(view);
      if (!this.current(request)) {
        // eslint-disable-next-line @typescript-eslint/return-await -- Finish synchronous capture initialization before its terminal promise can settle.
        return promise;
      }
      request.deadline = {
        handle: this.clock.set(
          () => this.finish(request, { status: 'rejected', reason: 'deadline' }),
          1000,
        ),
      };
      if (!this.current(request)) {
        // eslint-disable-next-line @typescript-eslint/return-await -- Finish synchronous capture initialization before its terminal promise can settle.
        return promise;
      }
      request.listener = Boolean(signal);
      signal?.addEventListener('abort', request.abort, { once: true });
      if (signal?.aborted) request.abort();
      if (!this.current(request)) {
        // eslint-disable-next-line @typescript-eslint/return-await -- Finish synchronous capture initialization before its terminal promise can settle.
        return promise;
      }
      view.requestMeasure({
        key: request,
        read: () => {
          try {
            return this.current(request) && documentAuthorityMatches(view.state, authority);
          } catch (cause) {
            this.fail(request, cause);
            return false;
          }
        },
        /** Native input settlement and transactions resume only after CodeMirror releases its measurement write phase. */
        write: (ready) => {
          try {
            queueMicrotask(() => {
              try {
                if (!this.current(request)) return;
                if (!ready || !documentAuthorityMatches(view.state, authority)) {
                  this.finish(request, {
                    status: 'rejected',
                    reason: 'superseded',
                  });
                  return;
                }
                const composing = nativeCompositionStarted(view);
                if (!this.current(request)) return;
                if (composing) {
                  this.finish(request, {
                    status: 'rejected',
                    reason: 'composing',
                  });
                  return;
                }
                if (!documentAuthorityMatches(view.state, authority)) {
                  this.finish(request, {
                    status: 'rejected',
                    reason: 'superseded',
                  });
                  return;
                }
                requireValue(request.input, 'native-document-checkpoints').finish();
                if (!this.current(request)) return;
                const settled = nativeDocumentAuthority(view.state);
                if (nativeCompositionStarted(view)) {
                  this.finish(request, {
                    status: 'rejected',
                    reason: 'composing',
                  });
                  return;
                }
                if (!this.current(request)) return;
                if (!documentAuthorityMatches(view.state, settled)) {
                  this.finish(request, {
                    status: 'rejected',
                    reason: 'superseded',
                  });
                  return;
                }
                const transaction = view.state.update({
                  annotations: [isolateHistory.of('after'), Transaction.addToHistory.of(false)],
                });
                view.dispatch(transaction);
                if (!this.current(request)) return;
                if (
                  !documentAuthorityMatches(view.state, nativeDocumentAuthority(transaction.state))
                ) {
                  this.finish(request, {
                    status: 'rejected',
                    reason: 'superseded',
                  });
                  return;
                }
                this.finish(request, {
                  status: 'ready',
                  authority: nativeDocumentAuthority(view.state),
                });
              } catch (cause) {
                this.fail(request, cause);
              }
            });
          } catch (cause) {
            this.fail(request, cause);
          }
        },
      });
    } catch (cause) {
      this.fail(request, cause);
    } finally {
      request.initializing = false;
      if (request.terminal || request.failed) this.releaseRequest(request);
    }
    return promise;
  }
  /** Handle methods close over authentication only; deleting a seed releases every retained native field. */
  private retain(seed: NativeDocumentSeed): DocumentCheckpoint {
    const owner = new WeakRef(this);
    const checkpoint: DocumentCheckpoint = Object.freeze({
      [documentCheckpointBrand]: true as const,
      get source() {
        const current = owner.deref()?.inspect(checkpoint);
        if (!current || typeof current === 'string')
          throw new Error('Document checkpoint unavailable');
        return documentSeedSource(current);
      },
      release: () => {
        owner.deref()?.seeds.delete(checkpoint);
      },
    });
    this.issued.add(checkpoint);
    this.seeds.set(checkpoint, seed);
    return checkpoint;
  }
  /** Recheck mounted authority after every external collaborator or native callback. */
  private current(request: CaptureRequest): boolean {
    if (this.pending !== request || request.terminal || request.failed) return false;
    if (this.disposed || !this.owner.alive()) {
      this.finish(request, { status: 'rejected', reason: 'disposed' });
      return false;
    }
    if (this.owner.view() !== request.view) {
      this.finish(request, { status: 'rejected', reason: 'superseded' });
      return false;
    }
    return this.pending === request && !request.terminal && !request.failed;
  }
  /** Unexpected failures retain their exact primary, including undefined. */
  private fail(request: CaptureRequest, cause: unknown): void {
    if (request.failed || request.released) return;
    request.failed = true;
    request.cause = cause;
    if (this.pending === request) this.pending = undefined;
    if (!request.initializing) this.releaseRequest(request);
  }
  /** Retire identity immediately while initialization still owns any resource allocation in progress. */
  private finish(request: CaptureRequest, outcome: CaptureCompletion): void {
    if (request.terminal || request.failed || request.released) return;
    request.terminal = outcome;
    if (this.pending === request) this.pending = undefined;
    if (!request.initializing) this.releaseRequest(request);
  }
  /** Attempt every independent release before settling; successful capture is revoked if cleanup fails. */
  private releaseRequest(request: CaptureRequest): void {
    if (request.released) return;
    request.released = true;
    const terminal = request.terminal;
    delete request.terminal;
    const input = request.input,
      deadline = request.deadline,
      listener = request.listener;
    delete request.input;
    delete request.deadline;
    request.listener = false;
    const causes: unknown[] = request.failed ? [request.cause] : [];
    delete request.cause;
    for (const release of [
      () => input?.cancel(),
      () => {
        if (deadline) this.clock.clear(deadline.handle);
      },
      () => {
        if (listener)
          requireValue(request.signal, 'native-document-checkpoints').removeEventListener(
            'abort',
            request.abort,
          );
      },
    ]) {
      try {
        release();
      } catch (cause) {
        causes.push(cause);
      }
    }
    let result: DocumentCaptureOutcome | undefined;
    if (!causes.length) {
      try {
        if (terminal?.status !== 'ready') result = terminal;
        else if (this.disposed || !this.owner.alive() || this.disposed)
          result = { status: 'rejected', reason: 'disposed' };
        else {
          const view = this.owner.view();
          if (this.disposed) result = { status: 'rejected', reason: 'disposed' };
          else {
            const aborted = request.signal?.aborted,
              composing = nativeCompositionStarted(view),
              cancelled = request.signal ? platformAborted.call(request.signal) : false,
              matches = documentAuthorityMatches(view.state, terminal.authority),
              captured = terminal.authority.document;
            if (this.disposed) result = { status: 'rejected', reason: 'disposed' };
            else if (this.generation !== request.generation || view !== request.view || !matches)
              result = { status: 'rejected', reason: 'superseded' };
            else if (aborted || cancelled) result = { status: 'rejected', reason: 'cancelled' };
            else if (composing) result = { status: 'rejected', reason: 'composing' };
            else
              result = {
                status: 'captured',
                checkpoint: this.retain(captured),
              };
          }
        }
      } catch (cause) {
        causes.push(cause);
      }
    }
    if (causes.length) {
      request.reject(
        causes.length === 1
          ? causes[0]
          : new AggregateError(causes, 'Document capture and cleanup failed'),
      );
    } else request.resolve(requireValue(result, 'native-document-checkpoints'));
  }
}

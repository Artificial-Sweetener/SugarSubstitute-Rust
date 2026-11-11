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
/** Coordinate fenced desktop capture/materialization over one bounded correlated protocol lifetime. */
import {
  isSessionMessage,
  isDesktopCheckpoint,
  utf8Bytes,
  sameSessionFence,
  sessionAdmission,
  sessionMessageError,
  type SessionErrorCode,
} from '../../core/embedded/SessionRuntime.js';
import type {
  SessionCapabilities,
  SessionMessage,
  SessionOperation,
  SessionScope,
  SessionBinding,
} from '../../../../../integrations/contracts/SessionProtocol.generated.js';

import {
  DesktopSessionCancellation,
  DesktopSessionRefusal,
  type DesktopSessionRuntime,
  type DesktopSessionTransport,
} from '../domain/DesktopSession.js';
import { DesktopSessionTransfer, desktopSessionChunks } from './DesktopSessionTransfer.js';

interface ActiveOperation {
  readonly requestId: string;
  readonly cancellation: DesktopSessionCancellation;
  /** The host withdrew this request and already received its cancelled reply. */
  hostCancelled: boolean;
  /** A terminal reply for this request has been sent to the host. */
  answered: boolean;
}
export interface DesktopSessionClientOptions {
  readonly transport: DesktopSessionTransport;
  readonly runtime: DesktopSessionRuntime;
  readonly nextId: () => string;
  readonly digest: (text: string) => Promise<string>;
  readonly report: (error: unknown) => void;
}

const capabilities: SessionCapabilities = Object.freeze({
  capture: true,
  materialize: true,
  chunkTransfer: true,
  artifactResolution: false,
});

/** Treat restored output state as presentation import; this protocol never queues inference. */
export class DesktopSessionClient {
  readonly #transfer: DesktopSessionTransfer;
  readonly #stops: (() => void)[] = [];
  #active: ActiveOperation | null = null;
  #disposed = false;
  #readySignature = '';
  #readySending = false;
  readonly #readyWaiters = new Set<{
    readonly resolve: () => void;
    readonly reject: (error: unknown) => void;
  }>();
  #pendingReady: SessionOperation | null = null;
  #awaitingAcknowledgement: {
    readonly requestId: string;
    readonly checkpointId: string;
    readonly revision: number;
    readonly bindings: readonly SessionBinding[];
    readonly phase: 'durable' | 'restored';
  } | null = null;

  /** Keep transport, hashing, identity generation and semantic materialization explicitly injected. */
  constructor(private readonly options: DesktopSessionClientOptions) {
    this.#transfer = new DesktopSessionTransfer(options.digest);
  }

  /** Listen before the first Ready so synchronous host capture cannot be lost. */
  install(): void {
    if (this.#disposed || this.#stops.length) return;
    if (this.options.runtime.bindObservationDelivery)
      this.#stops.push(this.options.runtime.bindObservationDelivery(() => this.#flushReady()));
    this.#stops.push(
      this.options.transport.subscribe((value) => {
        void this.receive(value).catch(this.options.report);
      }),
      this.options.runtime.subscribe(() => this.ready()),
    );
    this.ready();
  }

  /** Republish only a complete immutable observation, distinct from host checkpoint revision. */
  ready(): void {
    if (this.#disposed) return;
    const state = this.options.runtime.readiness();
    const operation: SessionOperation = {
      kind: 'ready',
      capabilities,
      committed: state.committed,
      workflowPersistence: state.workflowPersistence,
      fence: this.options.runtime.fence(),
    };
    const signature = JSON.stringify(operation);
    if (signature === this.#readySignature) return;
    this.#readySignature = signature;
    this.#pendingReady = operation;
    if (!this.#readySending) void this.#sendReady().catch(this.options.report);
  }

  /** Admit origin/scope/shape and full fences before retaining chunks or invoking semantic owners. */
  async receive(value: unknown): Promise<void> {
    if (this.#disposed) return;
    const { transport, runtime } = this.options;
    const error = sessionMessageError(value, transport.scope, transport.origin);
    if (error || !isSessionMessage(value)) return;
    const state = runtime.readiness();
    const admission = sessionAdmission(
      value,
      capabilities,
      state.committed,
      state.workflowPersistence,
      runtime.fence(),
    );
    if (admission) return this.#error(value.requestId, admission);
    const operation = value.operation;
    if (operation.kind === 'cancel') {
      this.#transfer.cancel(operation.targetRequestId);
      if (this.#active?.requestId === operation.targetRequestId) {
        this.#active.hostCancelled = true;
        this.#active.cancellation.cancel();
      }
      return this.#error(operation.targetRequestId, 'cancelled');
    }
    if (operation.kind === 'acknowledged') {
      const expected = this.#awaitingAcknowledgement;
      if (
        !expected ||
        value.requestId !== expected.requestId ||
        operation.phase !== expected.phase ||
        operation.checkpointId !== expected.checkpointId ||
        operation.revision !== expected.revision ||
        JSON.stringify(operation.bindings) !== JSON.stringify(expected.bindings)
      )
        return;
      runtime.acknowledged(operation.checkpointId, operation.revision);
      this.#awaitingAcknowledgement = null;
      if (operation.phase === 'durable')
        await this.#send(value.requestId, { ...operation, bindings: expected.bindings });
      return;
    }
    try {
      if (operation.kind === 'begin') this.#transfer.begin(value.requestId, operation.transfer);
      else if (operation.kind === 'chunk')
        this.#transfer.chunk(
          value.requestId,
          operation.transferId,
          operation.index,
          operation.text,
        );
      else if (operation.kind === 'commit')
        this.#transfer.commit(value.requestId, operation.transferId);
      else if (operation.kind === 'capture' || operation.kind === 'materialize')
        await this.#execute(value);
    } catch (cause: unknown) {
      this.#transfer.cancel(value.requestId);
      // A capture refused because Comfy is still loading or the workspace moved on is retried by the
      // host on its next schedule; neither is a failure the user needs to see.
      const captureRetried =
        operation.kind === 'capture' &&
        cause instanceof DesktopSessionRefusal &&
        (cause.code === 'not-committed' || cause.code === 'stale');
      if (!captureRetried) this.options.report(cause);
      await this.#error(
        value.requestId,
        cause instanceof DesktopSessionRefusal ? cause.code : 'invalid',
      );
    }
  }

  /** Cancel all owned request work before subscriptions and retained transfer bytes are released. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#active?.cancellation.cancel();
    this.#active = null;
    this.#awaitingAcknowledgement = null;
    this.#transfer.dispose();
    for (const waiter of this.#readyWaiters)
      waiter.reject(new DesktopSessionRefusal('cancelled', 'Desktop Ready delivery was retired.'));
    this.#readyWaiters.clear();
    for (const stop of this.#stops.splice(0)) stop();
  }

  /** Serialize one application operation while allowing its own chunk and cancel messages to arrive. */
  async #execute(message: SessionMessage): Promise<void> {
    if (this.#active) return this.#error(message.requestId, 'unavailable');
    const operation = message.operation;
    if (operation.kind !== 'capture' && operation.kind !== 'materialize') return;
    const active: ActiveOperation = {
      requestId: message.requestId,
      cancellation: new DesktopSessionCancellation(),
      hostCancelled: false,
      answered: false,
    };
    this.#active = active;
    try {
      if (operation.kind === 'capture') {
        const checkpoint = await this.options.runtime.capture(
          operation.checkpointId,
          operation.revision,
          active.cancellation,
        );
        if (!this.#current(active)) return;
        if (!sameSessionFence(operation.fence, this.options.runtime.fence()))
          return await this.#refuse(active, 'stale');
        if (
          !isDesktopCheckpoint(checkpoint) ||
          checkpoint.id !== operation.checkpointId ||
          checkpoint.revision !== operation.revision ||
          !sameScope(checkpoint.scope, this.options.transport.scope)
        )
          throw new TypeError('Captured checkpoint does not match the admitted request.');
        const text = JSON.stringify(checkpoint);
        const chunks = desktopSessionChunks(text);
        const digest = await this.options.digest(text);
        if (!this.#current(active)) return;
        if (!sameSessionFence(operation.fence, this.options.runtime.fence()))
          return await this.#refuse(active, 'stale');
        const transferId = this.options.nextId();
        await this.#send(message.requestId, {
          kind: 'begin',
          transfer: {
            id: transferId,
            checkpointId: checkpoint.id,
            revision: checkpoint.revision,
            bytes: utf8Bytes(text),
            chunks: chunks.length,
            digest,
            direction: 'capture',
          },
        });
        for (const [index, text] of chunks.entries()) {
          if (!this.#current(active)) return;
          if (!sameSessionFence(operation.fence, this.options.runtime.fence()))
            return await this.#refuse(active, 'stale');
          await this.#send(message.requestId, { kind: 'chunk', transferId, index, text });
        }
        if (!this.#current(active)) return;
        this.#awaitingAcknowledgement = {
          requestId: message.requestId,
          checkpointId: checkpoint.id,
          revision: checkpoint.revision,
          bindings: operation.fence.bindings,
          phase: 'durable',
        };
        await this.#send(message.requestId, { kind: 'commit', transferId });
        await this.#send(message.requestId, {
          kind: 'acknowledged',
          checkpointId: checkpoint.id,
          revision: checkpoint.revision,
          phase: 'captured',
          bindings: operation.fence.bindings,
        });
        active.answered = true;
      } else {
        const checkpoint = await this.#transfer.take(message.requestId, operation.transferId);
        if (!this.#current(active)) return;
        if (!sameScope(checkpoint.scope, this.options.transport.scope))
          return await this.#refuse(active, 'stale');
        const bindings = await this.options.runtime.materialize(checkpoint, active.cancellation);
        if (!this.#current(active)) return;
        this.#awaitingAcknowledgement = {
          requestId: message.requestId,
          checkpointId: checkpoint.id,
          revision: checkpoint.revision,
          bindings,
          phase: 'restored',
        };
        await this.#send(message.requestId, {
          kind: 'acknowledged',
          checkpointId: checkpoint.id,
          revision: checkpoint.revision,
          phase: 'materialized',
          bindings,
        });
        active.answered = true;
      }
    } catch (cause: unknown) {
      // A failure after cancellation is the cancellation's own outcome, such as an aborted asset
      // read, never a separate fault to show the user.
      if (active.cancellation.cancelled() || this.#disposed) return;
      throw cause;
    } finally {
      if (this.#active === active) {
        this.#active = null;
        // Work this page cancelled itself, for example when a graph load supersedes a restore, still
        // ends the host's request so it never waits for an answer that will not come.
        if (
          active.cancellation.cancelled() &&
          !active.hostCancelled &&
          !active.answered &&
          !this.#disposed
        )
          await this.#error(active.requestId, 'cancelled');
        this.ready();
      }
    }
  }

  /** Send one terminal refusal for the active request. */
  async #refuse(active: ActiveOperation, code: SessionErrorCode): Promise<void> {
    active.answered = true;
    await this.#error(active.requestId, code);
  }

  /** Refuse delayed completion after page teardown or request cancellation. */
  #current(active: ActiveOperation): boolean {
    return !this.#disposed && this.#active === active && !active.cancellation.cancelled();
  }

  /** Translate machine-readable refusal without introducing product copy into the protocol. */
  async #error(requestId: string, code: SessionErrorCode): Promise<void> {
    if (!this.#disposed)
      await this.#send(requestId, {
        kind: 'error',
        failure: {
          code,
          retryable: ['stale', 'unavailable', 'not-committed'].includes(code),
          entryId: null,
          outputId: null,
        },
      });
  }

  /** Use only the bootstrap's independently admitted scope and origin. */
  async #send(requestId: string, operation: SessionOperation): Promise<void> {
    await this.options.transport.send({
      protocol: 'sugarcubes.desktop-session',
      version: 1,
      requestId,
      scope: this.options.transport.scope,
      origin: this.options.transport.origin,
      operation,
    });
  }

  /** Coalesce observations while native admission supplies bounded per-message backpressure. */
  async #sendReady(): Promise<void> {
    this.#readySending = true;
    try {
      while (!this.#disposed && this.#pendingReady) {
        const operation = this.#pendingReady;
        this.#pendingReady = null;
        await this.#send(this.options.nextId(), operation);
      }
    } catch (error: unknown) {
      this.#readySignature = '';
      this.#pendingReady = null;
      for (const waiter of this.#readyWaiters) waiter.reject(error);
      this.#readyWaiters.clear();
      throw error;
    } finally {
      this.#readySending = false;
      for (const waiter of this.#readyWaiters) waiter.resolve();
      this.#readyWaiters.clear();
    }
  }

  /** Flush every coalesced complete binding observation before the runtime requests pre-ack retained facts. */
  #flushReady(): Promise<void> {
    if (this.#disposed)
      return Promise.reject(
        new DesktopSessionRefusal('cancelled', 'Desktop Ready delivery was retired.'),
      );
    this.ready();
    if (!this.#readySending && !this.#pendingReady) return Promise.resolve();
    return new Promise<void>((resolve, reject) => {
      this.#readyWaiters.add({ resolve, reject });
    });
  }
}

/** Compare scoped identities by value, independent of JSON member ordering. */
function sameScope(left: SessionScope, right: SessionScope): boolean {
  return (
    left.installation === right.installation &&
    left.profile === right.profile &&
    left.workspace === right.workspace &&
    left.backend === right.backend
  );
}

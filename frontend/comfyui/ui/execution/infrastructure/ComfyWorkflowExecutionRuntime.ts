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
/** Compose the shared execution lifetime while keeping host registration and cube queue policy thin. */

import { WorkflowSessionStore } from '../application/WorkflowSessionStore.js';
import { WorkflowResultStore } from '../application/WorkflowResultStore.js';
import { WorkflowSeedHistory } from '../application/WorkflowSeedHistory.js';
import { WorkflowOutputActivity } from '../application/WorkflowOutputActivity.js';
import { WorkflowRunRegistry } from '../application/WorkflowRunRegistry.js';
import { WorkflowProgressStore } from '../application/WorkflowProgressStore.js';
import { WorkflowSubmissionContexts } from '../application/WorkflowSubmissionContexts.js';
import { WorkflowExecutionIngestor } from '../application/WorkflowExecutionIngestor.js';
import { WorkflowResultLifecycle } from '../application/WorkflowResultLifecycle.js';
import { ComfyWorkflowSessionAdapter } from './ComfyWorkflowSessionAdapter.js';
import { ComfyWorkflowEventAdapter } from './ComfyWorkflowEventAdapter.js';
import { ComfyWorkflowHistoryAdapter } from './ComfyWorkflowHistoryAdapter.js';
import { ComfyPromptSubmissionObserver } from './ComfyPromptSubmissionObserver.js';
import { ComfyWorkflowPreviewDelivery } from './ComfyWorkflowPreviewDelivery.js';
import { ComfyWorkflowOutputProjection } from './ComfyWorkflowOutputProjection.js';
import { createComfyWorkflowLoadObserver } from './ComfyWorkflowLoadObserver.js';
import { createComfyPromptConstructionObserver } from './ComfyPromptConstructionObserver.js';
import { restoreComfyWorkflowSnapshot } from './ComfyWorkflowSnapshotRestore.js';
import type { ExecutionClock } from '../domain/WorkflowExecutionEvent.js';
import {
  ComfyCubeLivePreview,
  type CubePreviewUrlPort,
  type CubePreviewVisibilityPort,
} from './ComfyCubeLivePreview.js';
import { workflowSessionKey } from './WorkflowSessionKey.js';
import { isRecord } from '../../types/common.js';
import type { WorkflowSessionId } from '../domain/ExecutionIdentity.js';
import type { WorkflowSubmissionContext } from '../domain/WorkflowSubmission.js';

export interface ComfyWorkflowExecutionRuntimeOptions {
  readonly app: unknown;
  readonly api: unknown;
  readonly logger: Pick<Console, 'error' | 'warn'>;
  readonly nextKey: () => string;
  readonly clock: ExecutionClock;
  readonly previewUrls: CubePreviewUrlPort;
  readonly previewVisibility: CubePreviewVisibilityPort;
}

/** Supply one result/session authority to all views and one observer to the existing queue bridge. */
export class ComfyWorkflowExecutionRuntime {
  readonly sessions = new WorkflowSessionStore();
  readonly results = new WorkflowResultStore();
  readonly seeds = new WorkflowSeedHistory();
  readonly activity = new WorkflowOutputActivity();
  readonly progress = new WorkflowProgressStore((id) => this.#runs.read(id));
  readonly previews: ComfyWorkflowPreviewDelivery;
  readonly outputs: ComfyWorkflowOutputProjection;
  readonly submissionObserver: ComfyPromptSubmissionObserver;
  readonly #cubePreviews: ComfyCubeLivePreview;
  readonly #runs = new WorkflowRunRegistry();
  readonly #contexts = new WorkflowSubmissionContexts();
  readonly #ingestor: WorkflowExecutionIngestor;
  readonly #connections = new Set<(connected: boolean) => void>();
  readonly #stopSeeds: () => void;
  #adapter: ComfyWorkflowSessionAdapter | null = null;
  #events: ComfyWorkflowEventAdapter | null = null;
  #history: ComfyWorkflowHistoryAdapter | null = null;
  #loads: ReturnType<typeof createComfyWorkflowLoadObserver> | null = null;
  #construction: ReturnType<typeof createComfyPromptConstructionObserver> | null = null;
  #lifecycle: WorkflowResultLifecycle | null = null;
  #installed = false;
  #disposed = false;
  #connected = true;

  /** Construct pure owners immediately and defer public-host capability access until extension setup. */
  constructor(private readonly options: ComfyWorkflowExecutionRuntimeOptions) {
    this.#stopSeeds = this.sessions.subscribe((snapshot) => {
      this.seeds.synchronize(snapshot);
      this.activity.synchronize(snapshot);
    });
    this.#ingestor = new WorkflowExecutionIngestor({
      runs: this.#runs,
      results: this.results,
      clock: options.clock,
      logger: options.logger,
      imageAccepted: (id) => this.activity.record(id),
      recover: (id) => {
        void this.#history?.reconcile(id);
      },
    });
    this.submissionObserver = new ComfyPromptSubmissionObserver({
      contexts: this.#contexts,
      logger: options.logger,
      seeds: this.seeds,
      submitting: (context) => this.#ingestor.submitting(context),
      accepted: (context, id) => {
        this.#ingestor.accepted(context, id);
        this.progress.refresh();
      },
    });
    this.previews = new ComfyWorkflowPreviewDelivery(this.results, options.logger);
    this.#cubePreviews = new ComfyCubeLivePreview(
      this.sessions,
      this.results,
      this.previews,
      options.previewUrls,
      () => options.clock.now(),
      options.previewVisibility,
    );
    this.outputs = new ComfyWorkflowOutputProjection(
      this.#cubePreviews,
      this.sessions,
      this.results,
    );
  }

  /** Observe host loading and execution before the queue bridge begins accepting requests. */
  install(): void {
    if (this.#disposed) throw new Error('Workflow execution runtime is disposed.');
    if (this.#installed) return;
    try {
      const adapter = new ComfyWorkflowSessionAdapter({
        app: this.options.app,
        sessions: this.sessions,
        nextKey: this.options.nextKey,
        logger: this.options.logger,
      });
      this.#adapter = adapter;
      const history = new ComfyWorkflowHistoryAdapter({
        api: this.options.api,
        runs: this.#runs,
        results: this.results,
        ingestor: this.#ingestor,
        logger: this.options.logger,
      });
      this.#history = history;
      this.#loads = createComfyWorkflowLoadObserver(
        this.options.app,
        this.sessions,
        adapter,
        this.options.logger,
      );
      this.#construction = createComfyPromptConstructionObserver(
        this.options.app,
        this.sessions,
        this.#contexts,
        this.options.logger,
      );
      this.#lifecycle = new WorkflowResultLifecycle({
        sessions: this.sessions,
        results: this.results,
        ingestor: this.#ingestor,
        cancelRecovery: (id) => history.cancelSession(id),
        restore: (context) => restoreComfyWorkflowSnapshot(context, adapter, this.results),
      });
      this.#events = new ComfyWorkflowEventAdapter({
        api: this.options.api,
        ingestor: this.#ingestor,
        progress: this.progress,
        logger: this.options.logger,
        preview: (source, blob) => this.previews.deliver(source, blob),
        connection: (connected) => this.#setConnected(connected),
        reconnect: () => history.reconnect(),
      });
      this.#lifecycle.install();
      this.#loads.install();
      this.#construction.install();
      adapter.install();
      this.#events.install();
      this.#installed = true;
    } catch (error: unknown) {
      this.dispose();
      throw error;
    }
  }

  /** Read connection state independently of whichever workflow or image is mounted. */
  get connected(): boolean {
    return this.#connected;
  }

  /** Read submission intent through the shared run registry, including outstanding queue receipts. */
  latestSubmissionOrder(session: WorkflowSessionId): number {
    return this.#runs.latestOrder(session);
  }

  /** Read immutable construction ownership without inferring identity from the currently active graph. */
  submissionContext(payload: unknown): WorkflowSubmissionContext | null {
    return isRecord(payload) ? this.#contexts.read(payload) : null;
  }

  /** Refuse live-graph preparation when an observed construction now names another workflow or load. */
  assertCurrentPrompt(payload: unknown): void {
    const captured = isRecord(payload) ? this.#contexts.read(payload) : null;
    if (!captured) return;
    const active = this.sessions.read().active;
    if (
      active?.sessionId !== captured.session.sessionId ||
      active.generation !== captured.session.generation
    )
      throw new Error('The workflow changed while preparing this run. Select it and run again.');
  }

  /** Subscribe a view to connection transitions with explicit surface-owned teardown. */
  subscribeConnection(listener: (connected: boolean) => void): () => void {
    this.#connections.add(listener);
    return () => {
      this.#connections.delete(listener);
    };
  }

  /** Stop all incoming adapters before releasing result metadata and pending transport. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#installed = false;
    this.#events?.dispose();
    this.progress.dispose();
    this.#construction?.dispose();
    this.#loads?.dispose();
    this.#adapter?.dispose();
    this.#lifecycle?.dispose();
    this.#history?.dispose();
    this.#cubePreviews.dispose();
    this.previews.dispose();
    this.#ingestor.dispose();
    this.#contexts.dispose();
    this.#runs.dispose();
    this.results.dispose();
    this.#stopSeeds();
    this.seeds.dispose();
    this.activity.dispose();
    this.sessions.dispose();
    this.#connections.clear();
  }

  /** Publish connection changes without clearing another workflow's durable results. */
  #setConnected(connected: boolean): void {
    if (this.#connected === connected) return;
    this.#connected = connected;
    for (const listener of this.#connections) listener(connected);
  }
}

/** Supply browser identity and timer capabilities at the infrastructure composition boundary. */
export function createComfyWorkflowExecutionRuntime(
  options: Pick<ComfyWorkflowExecutionRuntimeOptions, 'app' | 'api' | 'logger'>,
): ComfyWorkflowExecutionRuntime {
  return new ComfyWorkflowExecutionRuntime({
    ...options,
    previewUrls: {
      create: (blob) => URL.createObjectURL(blob),
      revoke: (url) => URL.revokeObjectURL(url),
    },
    previewVisibility: {
      read: () => !document.hidden,
      subscribe(listener) {
        document.addEventListener('visibilitychange', listener);
        return () => document.removeEventListener('visibilitychange', listener);
      },
    },
    nextKey: workflowSessionKey,
    clock: {
      now: () => Date.now(),
      later(delay, callback) {
        const handle = setTimeout(callback, delay);
        return () => clearTimeout(handle);
      },
    },
  });
}

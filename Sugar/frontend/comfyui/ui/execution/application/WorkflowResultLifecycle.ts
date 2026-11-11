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
/** Coordinate result invalidation and restoration with explicit workflow-session lifecycles. */

import type { WorkflowSessionContext, WorkflowSessionSnapshot } from '../domain/WorkflowSession.js';
import type { WorkflowSessionId } from '../domain/ExecutionIdentity.js';
import type { WorkflowSessionStore } from './WorkflowSessionStore.js';
import type { WorkflowResultStore } from './WorkflowResultStore.js';
import type { WorkflowExecutionIngestor } from './WorkflowExecutionIngestor.js';

export interface WorkflowResultLifecycleOptions {
  readonly sessions: WorkflowSessionStore;
  readonly results: WorkflowResultStore;
  readonly ingestor: WorkflowExecutionIngestor;
  cancelRecovery(sessionId: WorkflowSessionId): void;
  restore(context: WorkflowSessionContext): void;
}

/** Preserve inactive finals while retiring obsolete generations and their outstanding transport. */
export class WorkflowResultLifecycle {
  readonly #restored = new Map<WorkflowSessionId, number>();
  #unsubscribe: (() => void) | null = null;

  /** Bind state owners and adapter callbacks without importing browser or host values. */
  constructor(private readonly options: WorkflowResultLifecycleOptions) {}

  /** Observe named load, tab, close and rename transitions exactly once. */
  install(): void {
    if (this.#unsubscribe) return;
    this.#unsubscribe = this.options.sessions.subscribe((snapshot) => this.#synchronize(snapshot));
    this.#synchronize(this.options.sessions.read());
  }

  /** Stop orchestration before result and transport owners are disposed. */
  dispose(): void {
    this.#unsubscribe?.();
    this.#unsubscribe = null;
    this.#restored.clear();
  }

  /** Apply generation and closure invalidation before attempting demonstrably workflow-owned restoration. */
  #synchronize(snapshot: WorkflowSessionSnapshot): void {
    for (const context of snapshot.open) {
      const previous = this.options.results.read(context.sessionId);
      if (previous && previous.context.generation !== context.generation) {
        this.#retire(context.sessionId);
        this.#restored.delete(context.sessionId);
      }
    }
    for (const id of this.options.results.synchronize(snapshot.open)) {
      this.#retire(id);
      this.#restored.delete(id);
    }
    if (snapshot.loading || !snapshot.active) return;
    for (const context of snapshot.open) {
      if (this.#restored.get(context.sessionId) === context.generation) continue;
      this.options.restore(context);
      this.#restored.set(context.sessionId, context.generation);
    }
  }

  /** Drop late routing metadata and cancel only this workflow's pending exact-history requests. */
  #retire(sessionId: WorkflowSessionId): void {
    this.options.ingestor.closeSession(sessionId);
    this.options.cancelRecovery(sessionId);
  }
}

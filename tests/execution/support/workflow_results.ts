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
/** Supply explicit workflow attribution and controllable external boundaries for execution proofs. */

import { WorkflowResultStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowResultStore.js';
import { WorkflowRunRegistry } from '../../../frontend/comfyui/ui/execution/application/WorkflowRunRegistry.js';
import { WorkflowExecutionIngestor } from '../../../frontend/comfyui/ui/execution/application/WorkflowExecutionIngestor.js';
import {
  outputSourceId,
  promptId,
  workflowSessionId,
  type PromptId,
} from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type { WorkflowSubmissionContext } from '../../../frontend/comfyui/ui/execution/domain/WorkflowSubmission.js';
import type {
  WorkflowFinalEvent,
  ExecutionClock,
} from '../../../frontend/comfyui/ui/execution/domain/WorkflowExecutionEvent.js';
import { decodeComfyMedia } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyMediaDecoder.js';

/** Advance deadlines deterministically without depending on browser timer precision. */
export class WorkflowTestClock implements ExecutionClock {
  #now = 0;
  readonly #tasks = new Map<() => void, number>();
  /** Read elapsed test milliseconds. */
  now(): number {
    return this.#now;
  }
  /** Register one cancellable deadline. */
  later(delay: number, callback: () => void): () => void {
    this.#tasks.set(callback, this.#now + delay);
    return () => {
      this.#tasks.delete(callback);
    };
  }
  /** Run elapsed deadlines, including subsequent deadlines scheduled by their callbacks. */
  advance(milliseconds: number): void {
    this.#now += milliseconds;
    for (;;) {
      const task = [...this.#tasks].find(([, deadline]) => deadline <= this.#now);
      if (!task) return;
      this.#tasks.delete(task[0]);
      task[0]();
    }
  }
  /** Count retained timers for lifecycle assertions. */
  get pending(): number {
    return this.#tasks.size;
  }
}

/** Build colliding graph paths with distinct workflow binding identity. */
export function workflowSubmission(key = 'a', generation = 0): WorkflowSubmissionContext {
  return {
    session: { sessionId: workflowSessionId(key), title: 'Workflow', path: '', generation },
    sources: [
      {
        id: outputSourceId('42:23'),
        executionPath: '42:23',
        label: 'Preview',
        breadcrumb: ['Nested'],
        order: 0,
      },
    ],
  };
}

/** Decode a representative host event before passing it into typed application ownership. */
export function workflowFinal(id: PromptId, filename = 'final.png'): WorkflowFinalEvent {
  const media = decodeComfyMedia({ images: [{ filename }] });
  if (!media) throw new Error('Invalid execution fixture media');
  return { promptId: id, executionPaths: ['42:23'], ...media, merge: false };
}

/** Assemble real execution owners around an observable recovery boundary. */
export function workflowResultsFixture() {
  const contexts = [workflowSubmission('a'), workflowSubmission('b')];
  const results = new WorkflowResultStore();
  results.synchronize(contexts.map((context) => context.session));
  const runs = new WorkflowRunRegistry();
  const clock = new WorkflowTestClock();
  const recoveries: PromptId[] = [];
  const warnings: unknown[] = [];
  const ingestor = new WorkflowExecutionIngestor({
    runs,
    results,
    clock,
    logger: {
      warn: (...args: unknown[]) => {
        warnings.push(args);
      },
    },
    recover: (id) => {
      recoveries.push(id);
    },
  });
  return {
    results,
    runs,
    clock,
    recoveries,
    warnings,
    ingestor,
    a: workflowSubmission('a'),
    b: workflowSubmission('b'),
    id: promptId('one'),
  };
}

/** Model public event subscriptions with exact listener identities and no DOM dependency. */
export class WorkflowEventApi {
  readonly listeners = new Map<string, Set<(event: unknown) => void>>();
  failAdd: string | null = null;
  failRemove: string | null = null;
  /** Retain the listener before optionally simulating a host registration failure. */
  addEventListener(name: string, listener: (event: unknown) => void): void {
    const listeners = this.listeners.get(name) ?? new Set();
    listeners.add(listener);
    this.listeners.set(name, listeners);
    if (name === this.failAdd) throw new Error('Host registration failed');
  }
  /** Remove one exact listener or simulate an external unsubscription failure. */
  removeEventListener(name: string, listener: (event: unknown) => void): void {
    if (name === this.failRemove) throw new Error('Host unsubscription failed');
    this.listeners.get(name)?.delete(listener);
  }
  /** Deliver a Comfy-style public event to current listeners. */
  emit(name: string, detail: unknown): void {
    for (const listener of this.listeners.get(name) ?? []) listener({ detail });
  }
}

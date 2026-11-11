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
/** Project accepted job progress without borrowing the currently displayed graph. */
import type { PromptId } from '../domain/ExecutionIdentity.js';
import type { WorkflowRun } from '../domain/WorkflowRun.js';
import type { WorkflowSessionContext } from '../domain/WorkflowSession.js';

export interface WorkflowProgressSnapshot {
  readonly visible: boolean;
  readonly workflow: number;
  readonly sampler: number;
}

const idle: WorkflowProgressSnapshot = Object.freeze({ visible: false, workflow: 0, sampler: 0 });

/** Retain one backend execution while the existing run registry owns workflow attribution. */
export class WorkflowProgressStore {
  readonly #completed = new Set<string>();
  readonly #listeners = new Set<() => void>();
  #active: PromptId | null = null;
  #sampler = 0;
  #node = '';

  /** Resolve immutable submitted node paths from the authoritative accepted-run owner. */
  constructor(private readonly run: (id: PromptId) => WorkflowRun | null) {}

  /** Show only the running workflow generation, including acceptance arriving after its first event. */
  read(context: WorkflowSessionContext | null): WorkflowProgressSnapshot {
    const run = this.#active ? this.run(this.#active) : null;
    if (
      !context ||
      !run ||
      run.session.sessionId !== context.sessionId ||
      run.session.generation !== context.generation ||
      ['complete', 'cancelled', 'error'].includes(run.status)
    )
      return idle;
    const paths = new Set(run.sources.map((source) => source.executionPath));
    const completed = [...this.#completed].filter((path) => paths.has(path)).length;
    return Object.freeze({
      visible: true,
      workflow: paths.size ? (completed / paths.size) * 100 : 0,
      sampler: this.#sampler,
    });
  }

  /** Replace the single executing job without retaining stale progress from its predecessor. */
  started(id: PromptId): void {
    this.#active = id;
    this.#completed.clear();
    this.#sampler = 0;
    this.#node = '';
    this.refresh();
  }

  /** Count exact completed and cached execution paths once. */
  completed(id: PromptId, paths: readonly string[]): void {
    if (id !== this.#active) return;
    for (const path of paths) this.#completed.add(path);
    this.refresh();
  }

  /** Apply validated node progress, resetting naturally when a new sampler starts. */
  sampler(id: PromptId, node: string, value: number, maximum: number): void {
    if (id !== this.#active || !Number.isFinite(value) || !Number.isFinite(maximum) || maximum <= 0)
      return;
    this.#node = node;
    this.#sampler = Math.max(0, Math.min(100, (value / maximum) * 100));
    this.refresh();
  }

  /** Clear a finished sampler while retaining whole-workflow progress. */
  nodeFinished(id: PromptId, paths: readonly string[]): void {
    if (id !== this.#active) return;
    if (paths.includes(this.#node)) this.#sampler = 0;
    this.completed(id, paths);
  }

  /** Ignore a delayed terminal notification belonging to another job. */
  terminal(id: PromptId): void {
    if (id === this.#active) this.reset();
  }

  /** Retire the displayed execution on completion or connection loss. */
  reset(): void {
    this.#active = null;
    this.#completed.clear();
    this.#sampler = 0;
    this.#node = '';
    this.refresh();
  }

  /** Publish acceptance and progress updates without graph discovery or tab relayout. */
  refresh(): void {
    for (const listener of this.#listeners) listener();
  }

  /** Release exactly one presentation subscription. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Release retained paths and observers with the execution runtime. */
  dispose(): void {
    this.#listeners.clear();
    this.reset();
  }
}

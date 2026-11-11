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
/** Retain accepted seed values separately from armed values and undoable workflow state. */
import type { WorkflowSessionContext, WorkflowSessionSnapshot } from '../domain/WorkflowSession.js';
import type { WorkflowSessionId } from '../domain/ExecutionIdentity.js';
import type { SeedInput, SubmittedSeed } from '../domain/WorkflowSeed.js';

interface SeedHistory {
  readonly generation: number;
  readonly values: Map<string, { readonly order: number; readonly value: number }>;
}

/** Scope last-used values to an open workflow generation and submission order. */
export class WorkflowSeedHistory {
  readonly #histories = new Map<WorkflowSessionId, SeedHistory>();

  /** Retire closed or replaced workflows while preserving history across undo and tab activation. */
  synchronize(snapshot: WorkflowSessionSnapshot): void {
    const open = new Map(snapshot.open.map((context) => [context.sessionId, context.generation]));
    for (const [id, history] of this.#histories)
      if (open.get(id) !== history.generation) this.#histories.delete(id);
    for (const context of snapshot.open)
      if (!this.#histories.has(context.sessionId))
        this.#histories.set(context.sessionId, {
          generation: context.generation,
          values: new Map(),
        });
  }

  /** Admit accepted values only to the original open generation; late responses cannot regress history. */
  record(context: WorkflowSessionContext, order: number, values: readonly SubmittedSeed[]): void {
    const history = this.#histories.get(context.sessionId);
    if (!history || history.generation !== context.generation) return;
    for (const seed of values) {
      const key = JSON.stringify([seed.path, seed.name]);
      const previous = history.values.get(key);
      if (Number.isSafeInteger(seed.value) && (!previous || previous.order < order))
        history.values.set(key, { order, value: seed.value });
    }
  }

  /** Read the newest participating submission, refusing ambiguous shared values. */
  read(context: WorkflowSessionContext, inputs: readonly SeedInput[]): number | null {
    const history = this.#histories.get(context.sessionId);
    if (!history || history.generation !== context.generation) return null;
    const values = inputs.flatMap((input) => {
      const value = history.values.get(JSON.stringify([input.path, input.name]));
      return value ? [value] : [];
    });
    const order = values.reduce((latest, value) => Math.max(latest, value.order), -Infinity);
    const latest = new Set(
      values.filter((value) => value.order === order).map((value) => value.value),
    );
    return latest.size === 1 ? ([...latest][0] ?? null) : null;
  }

  /** Release all retained submission values at runtime teardown. */
  dispose(): void {
    this.#histories.clear();
  }
}

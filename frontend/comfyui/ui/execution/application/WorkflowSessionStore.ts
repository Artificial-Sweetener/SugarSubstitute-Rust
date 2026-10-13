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
/** Own workflow bindings, configuration generations, and safe activation commitment. */

import type { WorkflowSessionId } from '../domain/ExecutionIdentity.js';
import type {
  WorkflowLoadToken,
  WorkflowSessionBinding,
  WorkflowSessionContext,
  WorkflowSessionSnapshot,
} from '../domain/WorkflowSession.js';

/** Suspend graph attribution until observed host loads and workflow commitment finish. */
export class WorkflowSessionStore {
  readonly #sessions = new Map<WorkflowSessionId, WorkflowSessionContext>();
  readonly #pending = new Map<number, WorkflowLoadToken>();
  readonly #listeners = new Set<(snapshot: WorkflowSessionSnapshot) => void>();
  #selected: WorkflowSessionId | null = null;
  #ready = true;
  #revision = 0;
  #sequence = 0;
  #loadFailed = false;
  readonly #replaced = new Set<WorkflowSessionId>();

  /** Read a coherent immutable session snapshot without retaining host objects. */
  read(): WorkflowSessionSnapshot {
    return Object.freeze({
      revision: this.#revision,
      open: Object.freeze([...this.#sessions.values()]),
      active: this.#ready && this.#selected ? (this.#sessions.get(this.#selected) ?? null) : null,
      loading: this.#pending.size > 0,
    });
  }

  /** Retain qualified presentation during same-workflow history while execution attribution is paused. */
  presentation(): WorkflowSessionContext | null {
    if (!this.#selected || this.#loadFailed) return null;
    const retainingHistory =
      this.#pending.size > 0 &&
      this.#replaced.size === 0 &&
      [...this.#pending.values()].every(
        (token) => token.kind === 'history' && token.previousSessionId === this.#selected,
      );
    return this.#ready || retainingHistory ? (this.#sessions.get(this.#selected) ?? null) : null;
  }

  /** Synchronize explicit open bindings and return closed identities for adapter cleanup. */
  synchronize(
    bindings: readonly WorkflowSessionBinding[],
    selected: WorkflowSessionId | null,
  ): readonly WorkflowSessionId[] {
    const current = new Set(bindings.map((binding) => binding.sessionId));
    const closed: WorkflowSessionId[] = [];
    let changed = this.#selected !== selected;
    for (const id of this.#sessions.keys()) {
      if (current.has(id)) continue;
      this.#sessions.delete(id);
      closed.push(id);
      changed = true;
    }
    for (const binding of bindings) {
      const previous = this.#sessions.get(binding.sessionId);
      if (previous && previous.title === binding.title && previous.path === binding.path) continue;
      this.#sessions.set(
        binding.sessionId,
        Object.freeze({ ...binding, generation: previous?.generation ?? 0 }),
      );
      changed = true;
    }
    this.#selected = selected && current.has(selected) ? selected : null;
    if (changed) this.#publish();
    return Object.freeze(closed);
  }

  /** Suspend association before the public graph-load operation reaches asynchronous hooks. */
  beginLoad(kind: WorkflowLoadToken['kind'] = 'replace'): WorkflowLoadToken {
    if (this.#pending.size === 0) {
      this.#loadFailed = false;
      this.#replaced.clear();
    }
    const token = Object.freeze({
      sequence: ++this.#sequence,
      previousSessionId: this.#selected,
      kind,
    });
    this.#pending.set(token.sequence, token);
    this.#ready = false;
    this.#publish();
    return token;
  }

  /** Commit only after all overlapping loads settle, using the host's committed active binding. */
  completeLoad(token: WorkflowLoadToken, succeeded: boolean): void {
    if (this.#pending.get(token.sequence) !== token) return;
    this.#pending.delete(token.sequence);
    this.#loadFailed ||= !succeeded;
    if (token.kind === 'replace' && token.previousSessionId)
      this.#replaced.add(token.previousSessionId);
    if (this.#pending.size === 0) {
      this.#ready = !this.#loadFailed;
      if (this.#ready && this.#selected && this.#replaced.has(this.#selected)) {
        const current = this.#sessions.get(this.#selected);
        if (current)
          this.#sessions.set(
            current.sessionId,
            Object.freeze({ ...current, generation: current.generation + 1 }),
          );
      }
    }
    this.#publish();
  }

  /** Fail closed when host observation cannot establish a valid graph binding. */
  suspend(): void {
    if (!this.#ready) return;
    this.#ready = false;
    this.#publish();
  }

  /** Observe session changes through an explicitly disposable application boundary. */
  subscribe(listener: (snapshot: WorkflowSessionSnapshot) => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Cancel obsolete observation lifetimes without allowing delayed completions to revive them. */
  dispose(): void {
    this.#pending.clear();
    this.#replaced.clear();
    this.#sessions.clear();
    this.#selected = null;
    this.#ready = false;
    this.#listeners.clear();
  }

  /** Publish only named binding/load changes rather than presentation mutations. */
  #publish(): void {
    this.#revision++;
    const snapshot = this.read();
    for (const listener of this.#listeners) listener(snapshot);
  }
}

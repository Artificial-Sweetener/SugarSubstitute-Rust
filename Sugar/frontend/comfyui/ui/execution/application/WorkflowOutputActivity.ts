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
/** Track unread accepted image results independently of graph and tab presentation. */
import type { WorkflowSessionId } from '../domain/ExecutionIdentity.js';
import type { WorkflowSessionSnapshot } from '../domain/WorkflowSession.js';

/** Retain session-scoped activity until committed viewing or explicit session retirement. */
export class WorkflowOutputActivity {
  readonly #unread = new Set<WorkflowSessionId>();
  readonly #listeners = new Set<() => void>();
  #active: WorkflowSessionId | null = null;
  #open = new Set<WorkflowSessionId>();

  /** Read activity without manufacturing notifications from restored result snapshots. */
  unread(id: WorkflowSessionId): boolean {
    return this.#unread.has(id);
  }

  /** Accept only inactive, still-open workflow results and publish the first unread transition. */
  record(id: WorkflowSessionId): void {
    if (id === this.#active || !this.#open.has(id) || this.#unread.has(id)) return;
    this.#unread.add(id);
    this.#publish();
  }

  /** Clear viewing activity only after session activation commits and retire closed identities. */
  synchronize(snapshot: WorkflowSessionSnapshot): void {
    this.#open = new Set(snapshot.open.map((session) => session.sessionId));
    this.#active = snapshot.active?.sessionId ?? null;
    let changed = false;
    for (const id of this.#unread)
      if (!this.#open.has(id) || id === this.#active) {
        this.#unread.delete(id);
        changed = true;
      }
    if (changed) this.#publish();
  }

  /** Bind one presentation observer with explicit teardown. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Release activity with the execution runtime rather than persisting it in workflow files. */
  dispose(): void {
    this.#listeners.clear();
    this.#unread.clear();
    this.#open.clear();
  }

  /** Publish one accepted state transition to mounted views. */
  #publish(): void {
    for (const listener of this.#listeners) listener();
  }
}

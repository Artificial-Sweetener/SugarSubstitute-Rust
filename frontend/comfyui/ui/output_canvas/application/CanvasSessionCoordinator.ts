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
/** Restore one independent browser view per open workflow and retire closed bindings. */

import type { WorkflowSessionId } from '../../execution/domain/ExecutionIdentity.js';
import type { WorkflowSessionStore } from '../../execution/application/WorkflowSessionStore.js';
import type { WorkflowResultStore } from '../../execution/application/WorkflowResultStore.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import { canvasSessionId, type CanvasViewId } from '../domain/CanvasIdentity.js';
import type { CanvasViewRegistry } from './CanvasViewRegistry.js';
import type { CanvasSourceInventory } from './CanvasSourceInventory.js';
import type { CanvasSelectionController } from './CanvasSelectionController.js';
import { canvasOutputCatalog } from './CanvasOutputCatalog.js';

/** Coordinate lightweight primary views without owning surface resources or native multi-view accounting. */
export class CanvasSessionCoordinator {
  readonly #primary = new Map<WorkflowSessionId, CanvasViewId>();
  readonly #listeners = new Set<(view: CanvasViewId | null) => void>();
  readonly #retired = new Set<(view: CanvasViewId) => void>();
  readonly #stops: (() => void)[] = [];

  /** Inject authoritative execution and view owners with one metadata change boundary. */
  constructor(
    private readonly sessions: WorkflowSessionStore,
    private readonly results: WorkflowResultStore,
    private readonly sources: CanvasSourceInventory,
    private readonly views: CanvasViewRegistry,
    private readonly selection: CanvasSelectionController,
    private readonly metadata: { subscribe(listener: () => void): () => void },
  ) {}

  /** Observe named result/session changes while source discovery remains with its adapter. */
  install(): void {
    if (this.#stops.length) return;
    this.#stops.push(
      this.sessions.subscribe(() => this.#synchronize()),
      this.results.subscribe((session) => this.#refresh(session)),
      this.metadata.subscribe(() => this.#synchronize()),
    );
    this.#synchronize();
  }

  /** Retain the qualified browser view during history restoration and release it for graph replacement. */
  active(): CanvasViewId | null {
    const active = this.sessions.presentation();
    return active ? (this.#primary.get(active.sessionId) ?? null) : null;
  }

  /** Project an exact open session into its view without consulting the active graph. */
  catalog(view: CanvasViewId): CanvasCatalogSnapshot | null {
    const id = [...this.#primary].find(([, candidate]) => candidate === view)?.[0];
    if (!id) return null;
    const result = this.results.read(id);
    return result ? canvasOutputCatalog(result, this.sources.read(result.context)) : null;
  }

  /** Deliver scoped result changes and explicit active-view transitions. */
  subscribe(listener: (view: CanvasViewId | null) => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Allow inspection state owners to release a retired view through one lifecycle signal. */
  onRetired(listener: (view: CanvasViewId) => void): () => void {
    this.#retired.add(listener);
    return () => this.#retired.delete(listener);
  }

  /** Release only this coordinator's primary views, preserving independently owned native views. */
  dispose(): void {
    for (const stop of this.#stops.splice(0)) stop();
    for (const [session, view] of this.#primary) this.#remove(session, view);
    this.#listeners.clear();
    this.#retired.clear();
  }

  /** Keep view identity stable through rename and activation, while closure releases all light state. */
  #synchronize(): void {
    const snapshot = this.sessions.read();
    const open = new Set(snapshot.open.map((session) => session.sessionId));
    for (const [session, view] of this.#primary)
      if (!open.has(session)) this.#remove(session, view);
    for (const session of snapshot.open) {
      if (!this.#primary.has(session.sessionId))
        this.#primary.set(
          session.sessionId,
          this.views.create(canvasSessionId(session.sessionId)).viewId,
        );
      this.#refresh(session.sessionId);
    }
    this.#publish(this.active());
  }

  /** Reconcile one view using only existing topology metadata and current result descriptors. */
  #refresh(session: WorkflowSessionId): void {
    const view = this.#primary.get(session);
    if (!view) return;
    const catalog = this.catalog(view);
    if (catalog) this.selection.reconcile(view, catalog);
    this.#publish(view);
  }

  /** Retire this browser view and its metadata without manufacturing another binding. */
  #remove(session: WorkflowSessionId, view: CanvasViewId): void {
    this.selection.remove(view);
    this.views.remove(view);
    this.sources.remove(session);
    this.#primary.delete(session);
    for (const listener of this.#retired) listener(view);
  }

  /** Preserve explicit view qualification across presentation delivery. */
  #publish(view: CanvasViewId | null): void {
    for (const listener of this.#listeners) listener(view);
  }
}

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
/** Observe committed Comfy startup and explicit workflow-persistence choice without DOM or timing guesses. */
import type { WorkflowSessionStore } from '../../execution/application/WorkflowSessionStore.js';
import { isRecord, type UnknownRecord } from '../../types/common.js';

export interface DesktopReadiness {
  readonly committed: boolean;
  readonly workflowPersistence: boolean;
}

/** Extension setup registers observers and returns before Comfy's own workflow/tab restoration. */
export class ComfyDesktopReadiness {
  readonly #manager: UnknownRecord;
  readonly #settings: UnknownRecord;
  readonly #listeners = new Set<() => void>();
  readonly #stops: (() => void)[] = [];
  #snapshot: DesktopReadiness = Object.freeze({ committed: false, workflowPersistence: false });

  /** Require the exposed workspace subscription and settings change surface at the dynamic boundary. */
  constructor(
    app: unknown,
    private readonly sessions: WorkflowSessionStore,
  ) {
    const manager = isRecord(app) && isRecord(app.extensionManager) ? app.extensionManager : null;
    const ui = isRecord(app) && isRecord(app.ui) ? app.ui : null;
    const settings = ui && isRecord(ui.settings) ? ui.settings : null;
    if (
      !manager ||
      typeof manager.spinner !== 'boolean' ||
      typeof manager.$subscribe !== 'function' ||
      !settings ||
      typeof settings.getSettingValue !== 'function' ||
      typeof settings.addEventListener !== 'function' ||
      typeof settings.removeEventListener !== 'function'
    )
      throw new TypeError('Committed Comfy desktop startup observation is unavailable.');
    this.#manager = manager;
    this.#settings = settings;
  }

  /** Subscribe before inspecting state so delayed startup completion cannot be lost. */
  install(): void {
    if (this.#stops.length) return;
    const subscribe = this.#manager.$subscribe;
    if (typeof subscribe !== 'function')
      throw new TypeError('Comfy workspace subscription retired.');
    const stop: unknown = subscribe.call(this.#manager, this.#refresh, {
      detached: true,
      flush: 'sync',
    });
    if (typeof stop !== 'function')
      throw new TypeError('Comfy workspace unsubscribe is unavailable.');
    this.#stops.push(() => stop(), this.sessions.subscribe(this.#refresh));
    const add = this.#settings.addEventListener;
    const remove = this.#settings.removeEventListener;
    if (typeof add !== 'function' || typeof remove !== 'function')
      throw new TypeError('Comfy workflow persistence observation retired.');
    add.call(this.#settings, 'Comfy.Workflow.Persist.change', this.#refresh);
    this.#stops.push(() =>
      remove.call(this.#settings, 'Comfy.Workflow.Persist.change', this.#refresh),
    );
    this.#refresh();
  }

  /** Read the latest explicit commitment facts independently of presentation visibility. */
  read(): DesktopReadiness {
    return this.#snapshot;
  }

  /** Notify only semantic readiness/opt-out transitions, not unrelated workspace mutations. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }

  /** Retire every host observation when its owning extension page leaves. */
  dispose(): void {
    for (const stop of this.#stops.splice(0)) stop();
    this.#listeners.clear();
  }

  /** Combine built-in startup completion with the existing asynchronous graph commitment fence. */
  readonly #refresh = (): void => {
    const get = this.#settings.getSettingValue;
    if (typeof get !== 'function')
      throw new TypeError('Comfy workflow persistence setting retired.');
    const value: unknown = get.call(this.#settings, 'Comfy.Workflow.Persist', true);
    if (typeof value !== 'boolean')
      throw new TypeError('Comfy workflow persistence setting is invalid.');
    const snapshot = this.sessions.read();
    const next = {
      committed: this.#manager.spinner === false && !snapshot.loading && snapshot.active !== null,
      workflowPersistence: value,
    };
    if (
      next.committed === this.#snapshot.committed &&
      next.workflowPersistence === this.#snapshot.workflowPersistence
    )
      return;
    this.#snapshot = Object.freeze(next);
    for (const listener of this.#listeners) listener();
  };
}

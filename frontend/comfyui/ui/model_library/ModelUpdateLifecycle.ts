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
/** Refresh prepared notifications on browser activation without requesting provider checks. */
import type { ModelUpdatePort } from './ModelUpdates.js';
import { ModelPageActivity } from './ModelPageActivity.js';

/** Keep browser restoration and focus independent of model-specific provider refreshes. */
export class ModelUpdateLifecycle {
  readonly #window: Window;
  readonly #updates: ModelUpdatePort;
  readonly #report: (error: unknown) => void;
  readonly #page: ModelPageActivity;
  #ready = false;
  #generation = 0;
  #loading: Promise<void> = Promise.resolve();
  /** Bind the browser lifecycle and validated update port. */
  constructor(window: Window, updates: ModelUpdatePort, report: (error: unknown) => void) {
    this.#window = window;
    this.#updates = updates;
    this.#report = report;
    this.#page = new ModelPageActivity(window, this.#activity, () => this.dispose());
  }
  /** Read durable consent on activation and observe focus through reversible event listeners. */
  start(): void {
    this.#page.start();
  }
  /** Remove browser subscriptions without cancelling shared server-owned provider checks. */
  dispose(): void {
    this.#page.dispose();
  }
  /** Reload consent in order after restoration so stale reads cannot overwrite a newer decision. */
  readonly #activity = (active: boolean): void => {
    const generation = ++this.#generation;
    this.#ready = false;
    this.#window.removeEventListener('focus', this.#focus);
    if (!active) return;
    this.#window.addEventListener('focus', this.#focus);
    this.#loading = this.#loading
      .then(async () => {
        if (!this.#page.active || generation !== this.#generation) return;
        await this.#updates.load();
        if (!this.#page.active || generation !== this.#generation) return;
        this.#ready = true;
      })
      .catch(this.#report);
  };
  /** Reload prepared server state on focus without a CivitAI request. */
  readonly #focus = (): void => {
    if (this.#page.active && this.#ready)
      this.#loading = this.#loading
        .then(() => (this.#page.active ? this.#updates.load() : undefined))
        .catch(this.#report);
  };
}

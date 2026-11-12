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
/** Own observed committed saving settings while leaving persistence and policy on the server. */
import type { OutputSavingClient } from './OutputSavingClient.js';
import type { OutputSavingPreferences, OutputSavingSnapshot } from './OutputSavingPreferences.js';

export type OutputSavingCommands = Pick<
  OutputSavingClient,
  'load' | 'save' | 'preview' | 'importSubstitute'
>;

/** Share one presentation snapshot across settings, source controls and mounted native fields. */
export class OutputSavingConfiguration implements OutputSavingCommands {
  readonly #listeners = new Set<() => void>();
  #snapshot: OutputSavingSnapshot | null = null;
  #revision = 0;
  #loading: Promise<OutputSavingSnapshot> | null = null;

  /** Bind validated transport without creating browser-owned saving defaults. */
  constructor(private readonly client: OutputSavingCommands) {}

  /** Expose only the last accepted server observation. */
  read(): OutputSavingSnapshot | null {
    return this.#snapshot;
  }

  /** Observe deliberate settings changes without observing DOM activity or rebuilding graph inventory. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }

  /** Coalesce reads and prevent an older GET from replacing a successfully committed edit. */
  load(): Promise<OutputSavingSnapshot> {
    if (this.#loading) return this.#loading;
    const revision = this.#revision;
    this.#loading = this.client
      .load()
      .then((snapshot) => {
        if (revision !== this.#revision && this.#snapshot) return this.#snapshot;
        this.#accept(snapshot);
        return snapshot;
      })
      .finally(() => {
        this.#loading = null;
      });
    return this.#loading;
  }

  /** Publish settings only after their authoritative write succeeds. */
  async save(preferences: OutputSavingPreferences): Promise<OutputSavingSnapshot> {
    const snapshot = await this.client.save(preferences);
    this.#revision++;
    this.#accept(snapshot);
    return snapshot;
  }

  /** Publish an explicit import without treating it as workflow-manager opt-in. */
  async importSubstitute(value: unknown): Promise<OutputSavingSnapshot> {
    const snapshot = await this.client.importSubstitute(value);
    this.#revision++;
    this.#accept(snapshot);
    return snapshot;
  }

  /** Keep draft path examples out of the committed presentation snapshot. */
  preview(
    preferences: OutputSavingPreferences,
    signal: AbortSignal,
  ): Promise<OutputSavingSnapshot> {
    return this.client.preview(preferences, signal);
  }

  /** Notify consumers of accepted observations without owning their control lifetimes. */
  #accept(snapshot: OutputSavingSnapshot): void {
    this.#snapshot = snapshot;
    for (const listener of this.#listeners) listener();
  }
}

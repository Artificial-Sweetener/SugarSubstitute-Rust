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
/** Coalesce native source replacements into one discovery and presenter refresh. */
export type WorkflowMasonryRebind = (source: HTMLElement | null) => boolean;

/** Keep source discovery scoped to the live view and the current replacement batch. */
export class WorkflowMasonryRemountQueue {
  readonly #pending = new Map<string, WorkflowMasonryRebind>();
  #disposed = false;

  /** Resolve current sources only after their owners finish replacing native controls. */
  constructor(
    readonly getSources: () => ReadonlyMap<string, HTMLElement>,
    readonly onRemounted: () => void,
  ) {}

  /** Deduplicate repeated invalidations of a card within one native replacement batch. */
  request(id: string, rebind: WorkflowMasonryRebind): void {
    if (this.#disposed) return;
    const scheduled = this.#pending.size > 0;
    this.#pending.set(id, rebind);
    if (!scheduled) queueMicrotask(this.#flush);
  }

  /** Discard callbacks when the owning workflow leaves the presentation. */
  dispose(): void {
    this.#disposed = true;
    this.#pending.clear();
  }

  /** Share one ephemeral inventory without retaining replaced native DOM between batches. */
  readonly #flush = (): void => {
    if (this.#disposed || this.#pending.size === 0) return;
    const batch = [...this.#pending];
    this.#pending.clear();
    const sources = this.getSources();
    let changed = false;
    for (const [id, rebind] of batch) {
      if (this.#disposed) return;
      changed = rebind(sources.get(id) ?? null) || changed;
    }
    if (changed && !this.#disposed) this.onRemounted();
  };
}

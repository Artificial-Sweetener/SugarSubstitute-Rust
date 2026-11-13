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
/** Own the temporary cart order without changing workflow instances or catalog artifacts. */

/** Refer to one current workflow instance without copying its embedded definition. */
export interface ExistingCubeStackDraftEntry {
  readonly source: 'existing';
  readonly draftId: string;
  readonly instanceId: string;
}

/** Refer to one exact placement-ready artifact while allowing repeated Cube definitions. */
export interface NewCubeStackDraftEntry {
  readonly source: 'new';
  readonly draftId: string;
  readonly catalogKey: string;
}

export type CubeStackDraftEntry = ExistingCubeStackDraftEntry | NewCubeStackDraftEntry;

/** Keep Reset, Cancel and Apply semantics separate from native graph mutation. */
export class CubeStackDraft {
  readonly #original: readonly ExistingCubeStackDraftEntry[];
  readonly #createId: () => string;
  #entries: readonly CubeStackDraftEntry[];

  /** Capture stable initial instance order and bind a unique draft identity provider. */
  constructor(instanceIds: readonly string[], createId: () => string) {
    if (new Set(instanceIds).size !== instanceIds.length)
      throw new TypeError('Cube cart contains duplicate existing instances.');
    for (const id of instanceIds) requireIdentity(id);
    this.#original = Object.freeze(
      instanceIds.map((instanceId) =>
        Object.freeze({
          source: 'existing' as const,
          draftId: `existing:${instanceId}`,
          instanceId,
        }),
      ),
    );
    this.#entries = this.#original;
    this.#createId = createId;
  }

  /** Expose immutable draft references for presentation and one final application command. */
  get entries(): readonly CubeStackDraftEntry[] {
    return this.#entries;
  }

  /** Enable Apply and Reset only when additions, removals or ordering differ from the opening state. */
  get changed(): boolean {
    return (
      this.#entries.length !== this.#original.length ||
      this.#entries.some((entry, index) => entry !== this.#original[index])
    );
  }

  /** Stage an exact catalog artifact without loading it or creating a live native node. */
  add(catalogKey: string, beforeDraftId: string | null = null): string {
    requireIdentity(catalogKey);
    const index = this.#insertionIndex(beforeDraftId);
    const id = this.#createId();
    requireIdentity(id);
    const draftId = `new:${id}`;
    if (this.#entries.some((entry) => entry.draftId === draftId))
      throw new TypeError('Cube cart identity provider returned a duplicate draft identifier.');
    const entries = [...this.#entries];
    entries.splice(index, 0, Object.freeze({ source: 'new', draftId, catalogKey }));
    this.#entries = Object.freeze(entries);
    return draftId;
  }

  /** Remove one staged reference while retaining its original native instance for Cancel and Reset. */
  remove(draftId: string): void {
    const index = this.#entryIndex(draftId);
    this.#entries = Object.freeze(this.#entries.filter((_entry, candidate) => candidate !== index));
  }

  /** Move one reference before another or to the end without replacing stable draft identities. */
  move(draftId: string, beforeDraftId: string | null): void {
    const index = this.#entryIndex(draftId);
    if (beforeDraftId === draftId) return;
    this.#insertionIndex(beforeDraftId);
    const entries = [...this.#entries];
    const [entry] = entries.splice(index, 1);
    if (!entry) throw new Error('Cube cart entry disappeared during a synchronous move.');
    const destination =
      beforeDraftId === null
        ? entries.length
        : entries.findIndex((candidate) => candidate.draftId === beforeDraftId);
    entries.splice(destination, 0, entry);
    this.#entries = Object.freeze(entries);
  }

  /** Accept a complete keyboard or pointer permutation without allowing omissions or duplicates. */
  reorder(draftIds: readonly string[]): void {
    const byId = new Map(this.#entries.map((entry) => [entry.draftId, entry]));
    if (
      draftIds.length !== byId.size ||
      new Set(draftIds).size !== draftIds.length ||
      draftIds.some((id) => !byId.has(id))
    )
      throw new TypeError('Cube cart reorder must be an exact draft permutation.');
    this.#entries = Object.freeze(
      draftIds.map((id) => {
        const entry = byId.get(id);
        if (!entry) throw new Error('Cube cart entry disappeared during a synchronous reorder.');
        return entry;
      }),
    );
  }

  /** Restore the immutable opening draft without reading a changed live workflow. */
  reset(): void {
    this.#entries = this.#original;
  }

  /** Resolve one stable draft reference before mutating temporary state. */
  #entryIndex(draftId: string): number {
    const index = this.#entries.findIndex((entry) => entry.draftId === draftId);
    if (index < 0) throw new Error('Cube cart entry is unavailable.');
    return index;
  }

  /** Treat the end as an explicit destination and reject stale row targets. */
  #insertionIndex(beforeDraftId: string | null): number {
    return beforeDraftId === null ? this.#entries.length : this.#entryIndex(beforeDraftId);
  }
}

/** Require nonempty stable references while preserving their exact backend identity. */
function requireIdentity(value: string): void {
  if (!value.trim()) throw new TypeError('Cube cart identity must not be empty.');
}

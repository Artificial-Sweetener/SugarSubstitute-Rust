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
/** The person's autocomplete lists: their files, their enabled state and the tags they give. */
import {
  autocompleteListAt,
  autocompleteListId,
  autocompleteListRecord,
  autocompleteListTags,
  disabledAutocompleteLists,
  listNameRefusal,
  type AutocompleteList,
  type AutocompleteListKind,
} from '../domain/AutocompleteList.js';

/** The lists folder, as the library reads and writes it. */
export interface AutocompleteListFiles {
  list(): Promise<readonly { readonly path: string; readonly stamp: string }[]>;
  read(path: string): Promise<string | null>;
  create(path: string, text: string): Promise<'created' | 'exists'>;
  write(path: string, text: string): Promise<void>;
  rename(from: string, to: string): Promise<'renamed' | 'exists' | 'missing'>;
  delete(path: string): Promise<void>;
}

/** The tags enabled lists add to and hide from suggestions, as one revision. */
export interface PromptTagLists {
  readonly revision: string;
  readonly custom: readonly string[];
  readonly censored: readonly string[];
}

/** The record of which lists are disabled, beside the two list folders. */
const RECORD = 'lists.json';

/**
 * Hold the lists folder as last read. Every change made through the library is written first and
 * then read back, so what it publishes is always what the folder holds.
 */
export class AutocompleteListLibrary {
  readonly #listeners = new Set<() => void>();
  readonly #texts = new Map<string, { stamp: string; text: string }>();
  #lists: readonly AutocompleteList[] = [];
  #disabled = new Set<string>();
  #tags: PromptTagLists = Object.freeze({ revision: '0', custom: [], censored: [] });
  #revision = 0;
  #refreshing: Promise<void> | null = null;

  constructor(
    private readonly files: AutocompleteListFiles,
    private readonly report: (error: unknown) => void,
  ) {}

  /** Every list, custom ones first, each kind in path order. */
  get lists(): readonly AutocompleteList[] {
    return this.#lists;
  }

  /** The tags the enabled lists give now. */
  current(): PromptTagLists {
    return this.#tags;
  }

  /** Follow changes to the lists or the tags they give. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** How many tags a list holds, as last read. */
  tagCount(id: string): number {
    return autocompleteListTags(this.#texts.get(id)?.text ?? '').length;
  }

  /** Read the folder again; callers arriving during a refresh share it. */
  refresh(): Promise<void> {
    this.#refreshing ??= this.#refresh()
      .catch((error: unknown) => this.report(error))
      .finally(() => {
        this.#refreshing = null;
      });
    return this.#refreshing;
  }

  /** One list's text; null when it is gone. */
  read(id: string): Promise<string | null> {
    return this.files.read(id);
  }

  /** Save a list's text. */
  async write(id: string, text: string): Promise<void> {
    await this.files.write(id, text);
    await this.refresh();
  }

  /** Create an empty, enabled list; refused when the name is unsafe or taken. */
  async create(kind: AutocompleteListKind, name: string): Promise<string> {
    const refusal = listNameRefusal(name);
    if (refusal) throw new Error(refusal);
    const id = autocompleteListId(kind, name);
    if ((await this.files.create(id, '')) === 'exists')
      throw new Error(`Autocomplete list already exists: ${name}`);
    await this.refresh();
    return id;
  }

  /** Rename a list within its kind, keeping whether it is enabled. */
  async rename(id: string, name: string): Promise<string> {
    const list = this.#lists.find((entry) => entry.id === id);
    if (!list) throw new Error('That autocomplete list is no longer in the folder.');
    const refusal = listNameRefusal(name);
    if (refusal) throw new Error(refusal);
    const target = autocompleteListId(list.kind, name);
    const outcome = await this.files.rename(id, target);
    if (outcome === 'exists') throw new Error(`Autocomplete list already exists: ${name}`);
    if (outcome === 'missing')
      throw new Error('That autocomplete list is no longer in the folder.');
    if (this.#disabled.delete(id)) {
      this.#disabled.add(target);
      await this.#recordDisabled();
    }
    await this.refresh();
    return target;
  }

  /** Delete a list and forget whether it was enabled. */
  async delete(id: string): Promise<void> {
    await this.files.delete(id);
    if (this.#disabled.delete(id)) await this.#recordDisabled();
    await this.refresh();
  }

  /** Turn a list on or off. */
  async setEnabled(id: string, enabled: boolean): Promise<void> {
    if (enabled) this.#disabled.delete(id);
    else this.#disabled.add(id);
    await this.#recordDisabled();
    await this.refresh();
  }

  #recordDisabled(): Promise<void> {
    return this.files.write(RECORD, autocompleteListRecord(this.#disabled));
  }

  async #refresh(): Promise<void> {
    const [entries, record] = await Promise.all([this.files.list(), this.files.read(RECORD)]);
    let parsed: unknown = null;
    try {
      parsed = record === null ? null : JSON.parse(record);
    } catch {
      // A damaged record disables no list, as it did in SugarSubstitute.
    }
    this.#disabled = disabledAutocompleteLists(parsed);
    const lists = entries
      .flatMap((entry) => {
        const list = autocompleteListAt(entry.path, entry.stamp, this.#disabled);
        return list ? [list] : [];
      })
      .sort(
        (a, b) =>
          Number(a.kind === 'censored') - Number(b.kind === 'censored') ||
          (a.id < b.id ? -1 : a.id > b.id ? 1 : 0),
      );
    await Promise.all(
      lists.map(async (list) => {
        if (this.#texts.get(list.id)?.stamp === list.stamp) return;
        const text = await this.files.read(list.id);
        if (text === null) this.#texts.delete(list.id);
        else this.#texts.set(list.id, { stamp: list.stamp, text });
      }),
    );
    for (const id of [...this.#texts.keys()])
      if (!lists.some((list) => list.id === id)) this.#texts.delete(id);
    const tags = (kind: AutocompleteListKind): string[] =>
      lists
        .filter((list) => list.kind === kind && list.enabled)
        .flatMap((list) => autocompleteListTags(this.#texts.get(list.id)?.text ?? ''));
    const custom = tags('custom');
    const censored = tags('censored');
    const changed =
      JSON.stringify(lists) !== JSON.stringify(this.#lists) ||
      JSON.stringify([custom, censored]) !==
        JSON.stringify([this.#tags.custom, this.#tags.censored]);
    this.#lists = Object.freeze(lists);
    if (!changed) return;
    this.#tags = Object.freeze({
      revision: String(++this.#revision),
      custom: Object.freeze(custom),
      censored: Object.freeze(censored),
    });
    for (const listener of [...this.#listeners]) listener();
  }
}

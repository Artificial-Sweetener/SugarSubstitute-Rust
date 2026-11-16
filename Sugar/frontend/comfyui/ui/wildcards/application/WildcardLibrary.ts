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
/** Hold the wildcard folder's files: their catalog for editors and their candidates for a run. */
import {
  prepareWildcardCatalog,
  type WildcardCandidateSource,
  type WildcardCatalog,
} from '../../../../sugarprompt/services/src/index.js';
import {
  csvColumns,
  csvWildcard,
  textCandidates,
  wildcardIdentifier,
  type WildcardFile,
} from '../domain/WildcardSource.js';

/** Where wildcard files are listed and read. */
export interface WildcardFileStore {
  list(): Promise<readonly WildcardFile[]>;
  /** A file's text, or null when it no longer exists. */
  read(path: string): Promise<string | null>;
}

/** What a prompt editor follows to decorate and complete wildcards. */
export interface WildcardCatalogSource {
  /** The current catalog, or null before the folder was first read. */
  readonly catalog: WildcardCatalog | null;
  subscribe(listener: () => void): () => void;
  /** Read the folder again; the catalog changes only when a file did. */
  refresh(): Promise<void>;
}

/** How many CSV files are read at once while their headers are collected. */
const HEADER_READS = 8;

/**
 * Follow the wildcard folder without holding its contents.
 *
 * A refresh lists the folder and reads only CSV headers, and only of files that changed since the
 * last one. Candidates are read when a run names their wildcard and kept until their file changes
 * or leaves the folder, so memory follows the wildcards in use rather than the collection's size.
 */
export class WildcardLibrary implements WildcardCatalogSource {
  readonly #listeners = new Set<() => void>();
  readonly #columns = new Map<string, { stamp: string; columns: readonly string[] }>();
  readonly #candidates = new Map<string, { stamp: string; source: WildcardCandidateSource }>();
  #files = new Map<string, WildcardFile>();
  #signature: string | null = null;
  #revision = 0;
  #catalog: WildcardCatalog | null = null;
  #refreshing: Promise<void> | null = null;

  /** Borrow the folder and the product's error reporting. */
  constructor(
    private readonly store: WildcardFileStore,
    private readonly report: (error: unknown) => void,
  ) {}

  get catalog(): WildcardCatalog | null {
    return this.#catalog;
  }

  /** Identify the folder state candidates and the catalog were read from. */
  get revision(): string {
    return String(this.#revision);
  }

  /** The folder's files as last listed, in path order. */
  get files(): readonly WildcardFile[] {
    return [...this.#files.values()];
  }

  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Read the folder again; callers arriving during a refresh share it. */
  refresh(): Promise<void> {
    this.#refreshing ??= this.#refresh().finally(() => {
      this.#refreshing = null;
    });
    return this.#refreshing;
  }

  /**
   * Read the candidates of the wildcard a prompt names, or null when the folder has no such file.
   *
   * Tagged picks are derived from `sourceId`. It is the wildcard's own name, so the same prompt
   * and seed pick the same candidate wherever the folder lives.
   */
  async candidates(form: 'simple' | 'csv', name: string): Promise<WildcardCandidateSource | null> {
    const identifier = wildcardIdentifier(name);
    const file = this.#files.get(key(form === 'simple' ? 'text' : 'csv', identifier));
    if (!file) return null;
    const held = this.#candidates.get(file.path);
    if (held?.stamp === file.stamp) return held.source;
    const text = await this.store.read(file.path);
    const source: WildcardCandidateSource =
      text === null
        ? { status: 'unavailable', form, identifier }
        : form === 'simple'
          ? { status: 'ready', form, identifier, sourceId: identifier, lines: textCandidates(text) }
          : {
              status: 'ready',
              form,
              identifier,
              sourceId: identifier,
              rows: csvWildcard(text).rows,
            };
    if (text !== null) this.#candidates.set(file.path, { stamp: file.stamp, source });
    return source;
  }

  /** List the folder and republish the catalog when any file was added, changed or removed. */
  async #refresh(): Promise<void> {
    let listed: readonly WildcardFile[];
    try {
      listed = await this.store.list();
    } catch (error: unknown) {
      this.report(error);
      if (this.#signature !== null || !this.#catalog) {
        this.#signature = null;
        this.#files = new Map();
        this.#publish(
          prepareWildcardCatalog({ status: 'unavailable', revision: String(++this.#revision) }),
        );
      }
      return;
    }
    const files = [...listed].sort((left, right) =>
      left.path < right.path ? -1 : left.path > right.path ? 1 : 0,
    );
    const signature = JSON.stringify(files.map((file) => [file.path, file.stamp]));
    if (signature === this.#signature) return;
    const index = new Map<string, WildcardFile>();
    // Two files that differ only by letter case name one wildcard: the first in path order.
    for (const file of files) {
      const name = key(file.kind, file.identifier);
      if (!index.has(name)) index.set(name, file);
    }
    const current = new Map([...index.values()].map((file) => [file.path, file.stamp]));
    for (const held of [this.#columns, this.#candidates])
      for (const [path, entry] of held) if (current.get(path) !== entry.stamp) held.delete(path);
    const csv = [...index.values()].filter((file) => file.kind === 'csv');
    for (let start = 0; start < csv.length; start += HEADER_READS)
      await Promise.all(csv.slice(start, start + HEADER_READS).map((file) => this.#header(file)));
    let catalog: WildcardCatalog;
    try {
      catalog = prepareWildcardCatalog({
        status: 'ready',
        revision: String(this.#revision + 1),
        textIdentifiers: [...index.values()]
          .filter((file) => file.kind === 'text')
          .map((file) => file.identifier),
        csvSources: csv.flatMap((file) => {
          const columns = this.#columns.get(file.path)?.columns;
          return columns?.length ? [{ identifier: file.identifier, columns }] : [];
        }),
      });
    } catch (error: unknown) {
      this.report(error);
      return;
    }
    this.#revision++;
    this.#signature = signature;
    this.#files = index;
    this.#publish(catalog);
  }

  /** Read one changed CSV file's header; an unreadable file offers no columns. */
  async #header(file: WildcardFile): Promise<void> {
    if (this.#columns.get(file.path)?.stamp === file.stamp) return;
    let columns: readonly string[] = [];
    try {
      const text = await this.store.read(file.path);
      if (text !== null) columns = csvColumns(text);
    } catch (error: unknown) {
      this.report(error);
    }
    this.#columns.set(file.path, { stamp: file.stamp, columns });
  }

  /** Install a catalog and tell every editor following it. */
  #publish(catalog: WildcardCatalog): void {
    this.#catalog = catalog;
    for (const listener of this.#listeners) listener();
  }
}

/** Key a file by kind and identifier: a TXT and a CSV file may share a name. */
function key(kind: WildcardFile['kind'], identifier: string): string {
  return kind + ':' + identifier;
}

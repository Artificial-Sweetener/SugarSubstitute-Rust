import {
  prepareWildcardCatalog
} from "../chunk-AO5UJLCO.js";
import {
  csvColumns,
  csvWildcard,
  textCandidates,
  wildcardIdentifier
} from "../chunk-66NUEKNZ.js";
import "../chunk-BJZB62WU.js";

// frontend/comfyui/ui/wildcards/application/WildcardLibrary.ts
var HEADER_READS = 8;
var WildcardLibrary = class {
  /** Borrow the folder and the product's error reporting. */
  constructor(store, report) {
    this.store = store;
    this.report = report;
  }
  #listeners = /* @__PURE__ */ new Set();
  #columns = /* @__PURE__ */ new Map();
  #candidates = /* @__PURE__ */ new Map();
  #files = /* @__PURE__ */ new Map();
  #signature = null;
  #revision = 0;
  #catalog = null;
  #refreshing = null;
  get catalog() {
    return this.#catalog;
  }
  /** Identify the folder state candidates and the catalog were read from. */
  get revision() {
    return String(this.#revision);
  }
  /** The folder's files as last listed, in path order. */
  get files() {
    return [...this.#files.values()];
  }
  subscribe(listener) {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }
  /** Read the folder again; callers arriving during a refresh share it. */
  refresh() {
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
  async candidates(form, name) {
    const identifier = wildcardIdentifier(name);
    const file = this.#files.get(key(form === "simple" ? "text" : "csv", identifier));
    if (!file) return null;
    const held = this.#candidates.get(file.path);
    if (held?.stamp === file.stamp) return held.source;
    const text = await this.store.read(file.path);
    const source = text === null ? { status: "unavailable", form, identifier } : form === "simple" ? { status: "ready", form, identifier, sourceId: identifier, lines: textCandidates(text) } : {
      status: "ready",
      form,
      identifier,
      sourceId: identifier,
      rows: csvWildcard(text).rows
    };
    if (text !== null) this.#candidates.set(file.path, { stamp: file.stamp, source });
    return source;
  }
  /** List the folder and republish the catalog when any file was added, changed or removed. */
  async #refresh() {
    let listed;
    try {
      listed = await this.store.list();
    } catch (error) {
      this.report(error);
      if (this.#signature !== null || !this.#catalog) {
        this.#signature = null;
        this.#files = /* @__PURE__ */ new Map();
        this.#publish(
          prepareWildcardCatalog({ status: "unavailable", revision: String(++this.#revision) })
        );
      }
      return;
    }
    const files = [...listed].sort(
      (left, right) => left.path < right.path ? -1 : left.path > right.path ? 1 : 0
    );
    const signature = JSON.stringify(files.map((file) => [file.path, file.stamp]));
    if (signature === this.#signature) return;
    const index = /* @__PURE__ */ new Map();
    for (const file of files) {
      const name = key(file.kind, file.identifier);
      if (!index.has(name)) index.set(name, file);
    }
    const current = new Map([...index.values()].map((file) => [file.path, file.stamp]));
    for (const held of [this.#columns, this.#candidates])
      for (const [path, entry] of held) if (current.get(path) !== entry.stamp) held.delete(path);
    const csv = [...index.values()].filter((file) => file.kind === "csv");
    for (let start = 0; start < csv.length; start += HEADER_READS)
      await Promise.all(csv.slice(start, start + HEADER_READS).map((file) => this.#header(file)));
    let catalog;
    try {
      catalog = prepareWildcardCatalog({
        status: "ready",
        revision: String(this.#revision + 1),
        textIdentifiers: [...index.values()].filter((file) => file.kind === "text").map((file) => file.identifier),
        csvSources: csv.flatMap((file) => {
          const columns = this.#columns.get(file.path)?.columns;
          return columns?.length ? [{ identifier: file.identifier, columns }] : [];
        })
      });
    } catch (error) {
      this.report(error);
      return;
    }
    this.#revision++;
    this.#signature = signature;
    this.#files = index;
    this.#publish(catalog);
  }
  /** Read one changed CSV file's header; an unreadable file offers no columns. */
  async #header(file) {
    if (this.#columns.get(file.path)?.stamp === file.stamp) return;
    let columns = [];
    try {
      const text = await this.store.read(file.path);
      if (text !== null) columns = csvColumns(text);
    } catch (error) {
      this.report(error);
    }
    this.#columns.set(file.path, { stamp: file.stamp, columns });
  }
  /** Install a catalog and tell every editor following it. */
  #publish(catalog) {
    this.#catalog = catalog;
    for (const listener of this.#listeners) listener();
  }
};
function key(kind, identifier) {
  return kind + ":" + identifier;
}
export {
  WildcardLibrary
};

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
/** Prepared lookup indexes resolve complete names and suffixes without traversing the catalog during queries. */
import { requireValue } from '../../../core/src/required.js';
import type { LoraCatalogItem } from './contracts.js';
import { loraNameKey, withoutLoraExtension } from './names.js';
/** A collision stays explicit instead of selecting an unrelated ranked model. */
export type CatalogMatch =
  | {
      readonly status: 'found';
      readonly item: LoraCatalogItem;
    }
  | {
      readonly status: 'ambiguous';
      readonly candidateCount: number;
    }
  | {
      readonly status: 'missing';
    };
/** Reverse component nodes retain suffix candidates in linear path storage, rather than copying every long suffix string. */
interface SuffixNode {
  readonly children: Map<string, SuffixNode>;
  readonly items: Set<LoraCatalogItem>;
}
/** Python's lexical POSIX basename ignores empty and dot components without resolving parents. */
function basename(value: string): string {
  return (
    value
      .replaceAll('\\', '/')
      .split('/')
      .filter((part) => part && part !== '.')
      .at(-1) ?? ''
  );
}
/** The donor appends its default extension only when neither lexical path interpretation has a suffix. */
function backendQuery(value: string): string {
  const name = basename(value),
    dot = name.lastIndexOf('.');
  return loraNameKey(dot > 0 && dot < name.length - 1 ? value : value + '.safetensors');
}
/** Add one model once even when multiple fields produce the same normalized alias. */
function add(index: Map<string, Set<LoraCatalogItem>>, value: string, item: LoraCatalogItem): void {
  let bucket = index.get(value);
  if (!bucket) {
    bucket = new Set();
    index.set(value, bucket);
  }
  bucket.add(item);
}
/** Preserve candidate identity/count instead of certifying a first-ranked fallback as installed. */
function match(items: ReadonlySet<LoraCatalogItem> | undefined): CatalogMatch | null {
  if (!items?.size) return null;
  return items.size === 1
    ? { status: 'found', item: requireValue(items.values().next().value, 'catalog-index') }
    : { status: 'ambiguous', candidateCount: items.size };
}
/** One immutable catalog owns all derived indexes; the existing resource session remains the sole result cache/request owner. */
export class LoraCatalogIndex {
  private readonly prompt = new Map<string, Set<LoraCatalogItem>>();
  private readonly backend = new Map<string, Set<LoraCatalogItem>>();
  private readonly backendPrompt = new Map<string, Set<LoraCatalogItem>>();
  private readonly aliases = new Map<string, Set<LoraCatalogItem>>();
  private readonly collisions = new Map<string, Set<LoraCatalogItem>>();
  private readonly suffixes: SuffixNode = {
    children: new Map(),
    items: new Set(),
  };
  /** Build only from validated immutable metadata; no filesystem, network, media decoding or source document is retained. */
  constructor(items: readonly LoraCatalogItem[]) {
    for (const item of items) {
      const model = item.model;
      add(this.prompt, loraNameKey(model.promptName, true), item);
      add(this.backend, loraNameKey(model.backendValue), item);
      add(this.backendPrompt, loraNameKey(model.backendValue, true), item);
      add(this.collisions, loraNameKey(model.basename), item);
      for (const alias of [
        model.promptName,
        withoutLoraExtension(model.backendValue),
        item.title,
        model.basename,
      ])
        if (alias) add(this.aliases, loraNameKey(alias), item);
      for (const path of [model.promptName, model.backendValue]) this.indexSuffixes(path, item);
    }
  }
  /** Keep donor lookup priority for definite matches; an explicit installed backend filename disambiguates a colliding canonical stem. */
  find(name: string): CatalogMatch {
    const normalized = loraNameKey(name, true);
    const canonical = match(this.prompt.get(normalized));
    if (canonical?.status === 'ambiguous' && withoutLoraExtension(name) !== name) {
      const filename = match(this.backend.get(loraNameKey(name)));
      if (filename?.status === 'found') return filename;
    }
    const exact =
      canonical ??
      match(this.backend.get(backendQuery(name))) ??
      match(this.backendPrompt.get(normalized));
    if (exact) return exact;
    const alias = match(this.aliases.get(normalized));
    if (alias) return alias;
    const parts = normalized.split('/');
    let suffix: SuffixNode | undefined = this.suffixes;
    if (parts.some((part) => !part)) suffix = undefined;
    else for (const part of parts.reverse()) suffix = suffix?.children.get(part);
    const path = match(suffix?.items);
    if (path) return path;
    const bare = loraNameKey(basename(name), true);
    return (
      match(this.aliases.get(bare)) ?? match(this.collisions.get(bare)) ?? { status: 'missing' }
    );
  }
  /** Store each component once along its reverse path; all proper suffixes receive the same immutable model identity. */
  private indexSuffixes(path: string, item: LoraCatalogItem): void {
    const parts = loraNameKey(path, true).split('/').filter(Boolean).reverse();
    let node = this.suffixes;
    for (const [depth, part] of parts.entries()) {
      let child = node.children.get(part);
      if (!child) {
        child = { children: new Map(), items: new Set() };
        node.children.set(part, child);
      }
      node = child;
      if (depth < parts.length - 1) node.items.add(item);
    }
  }
}

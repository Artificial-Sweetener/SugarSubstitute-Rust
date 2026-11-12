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
/** Validate decoded list boundaries and delegate all tag semantics to the existing prepared catalog. */
import { physicalLines } from '../../../core/src/prompt/physical-lines.js';
import { strip } from '../../../core/src/prompt/text.js';
import { normalizeTagIdentity } from '../../../core/src/index.js';
import { prepareTagCompletionCatalog } from '../completion/catalog.js';
import type { BundledTag } from '../completion/contracts.js';
import type {
  AutocompleteList,
  AutocompleteListCommand,
  AutocompleteListHostResult,
  PreparedAutocompleteLists,
  AutocompleteListSnapshot,
} from './contracts.js';

/** Require only declared enumerable decoded keys; unknown fields never acquire runtime meaning. */
function record(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  return (
    value !== null &&
    typeof value === 'object' &&
    !Array.isArray(value) &&
    Object.keys(value).sort().join() === [...keys].sort().join()
  );
}

/** Bound opaque single-line host identities without reproducing host path or naming policy. */
function identity(value: unknown): value is string {
  return (
    typeof value === 'string' &&
    value.length > 0 &&
    value.length <= 256 &&
    // eslint-disable-next-line no-control-regex -- Preserve exact Python-compatible control-code and whitespace boundaries.
    !/[\r\n\u0000]/u.test(value)
  );
}

/** Preserve exact authored source under a combined one-million UTF-16-unit snapshot limit. */
function text(value: unknown): value is string {
  if (typeof value !== 'string' || value.length > 1_000_000 || value.includes('\u0000'))
    return false;
  for (const line of physicalLines(value))
    if (strip(line.text.slice(0, line.contentTo - line.from)).length > 1024) return false;
  return true;
}

/** Capture each external command field once; validation and immutable adaptation share those exact values. */
export function readListCommand(value: unknown): AutocompleteListCommand | null {
  if (record(value, ['kind', 'name', 'listKind', 'text'])) {
    const { kind, name, listKind, text: source } = value;
    if (
      kind === 'create' &&
      identity(name) &&
      (listKind === 'custom' || listKind === 'censored') &&
      text(source)
    )
      return Object.freeze({ kind, name, listKind, text: source });
    return null;
  }
  if (record(value, ['kind', 'id', 'text'])) {
    const { kind, id, text: source } = value;
    return kind === 'edit' && identity(id) && text(source)
      ? Object.freeze({ kind, id, text: source })
      : null;
  }
  if (record(value, ['kind', 'id', 'name'])) {
    const { kind, id, name } = value;
    return kind === 'rename' && identity(id) && identity(name)
      ? Object.freeze({ kind, id, name })
      : null;
  }
  if (record(value, ['kind', 'id'])) {
    const { kind, id } = value;
    return kind === 'delete' && identity(id) ? Object.freeze({ kind, id }) : null;
  }
  if (record(value, ['kind', 'id', 'enabled'])) {
    const { kind, id, enabled } = value;
    return kind === 'enable' && identity(id) && typeof enabled === 'boolean'
      ? Object.freeze({ kind, id, enabled })
      : null;
  }
  return null;
}

/** Capture full decoded host results once before validating immutable identities, capacity and publication. */
export function readListResult(value: unknown): AutocompleteListHostResult {
  if (record(value, ['status'])) {
    const { status } = value;
    if (
      status === 'unavailable' ||
      status === 'invalid' ||
      status === 'conflict' ||
      status === 'not-found' ||
      status === 'name-conflict'
    )
      return Object.freeze({ status });
    throw new TypeError('Invalid autocomplete list result');
  }
  if (!record(value, ['status', 'snapshot', 'affectedId']))
    throw new TypeError('Invalid autocomplete list result');
  const { status, snapshot, affectedId } = value;
  if (
    status !== 'ready' ||
    !(affectedId === null || identity(affectedId)) ||
    !record(snapshot, ['revision', 'lists'])
  )
    throw new TypeError('Invalid autocomplete list result');
  const { revision, lists: externalLists } = snapshot;
  if (!identity(revision) || !Array.isArray(externalLists))
    throw new TypeError('Invalid autocomplete list result');
  const count: unknown = externalLists.length;
  if (
    typeof count !== 'number' ||
    !Number.isSafeInteger(count) ||
    count < 0 ||
    listCapacity(count, 0) !== null
  )
    throw new TypeError('Invalid autocomplete list result');
  const lists: AutocompleteList[] = [],
    ids = new Set<string>();
  let units = 0;
  for (let index = 0; index < count; ++index) {
    const row: unknown = externalLists[index];
    if (!record(row, ['id', 'name', 'kind', 'enabled', 'text']))
      throw new TypeError('Invalid autocomplete list record');
    const { id, name, kind, enabled, text: source } = row;
    if (
      !identity(id) ||
      ids.has(id) ||
      !identity(name) ||
      !(kind === 'custom' || kind === 'censored') ||
      typeof enabled !== 'boolean' ||
      !text(source)
    )
      throw new TypeError('Invalid autocomplete list record');
    units += source.length;
    if (listCapacity(lists.length + 1, units) === 'source')
      throw new RangeError('Autocomplete list source limit');
    ids.add(id);
    lists.push(Object.freeze({ id, name, kind, enabled, text: source }));
  }
  return Object.freeze({
    status,
    affectedId,
    snapshot: Object.freeze({ revision, lists: Object.freeze(lists) }),
  });
}

/** Capacity depends only on exact source/count, independently of host identities or display names. */
function listCapacity(count: number, units: number): 'count' | 'source' | null {
  if (count > 1024) return 'count';
  return units > 1_000_000 ? 'source' : null;
}

/** Canonical physical-line normalization and combined row admission are shared by intended commands and publications. */
function prepareListRows(
  lists: readonly Pick<AutocompleteList, 'kind' | 'enabled' | 'text'>[],
  bundled: readonly BundledTag[],
): { readonly customTags: string[]; readonly censoredTags: string[] } | null {
  const customTags: string[] = [],
    censoredTags: string[] = [];
  for (const list of lists) {
    if (!list.enabled) continue;
    for (const line of physicalLines(list.text)) {
      const tag = strip(line.text.slice(0, line.contentTo - line.from));
      if (normalizeTagIdentity(tag)) {
        if (bundled.length + customTags.length + censoredTags.length >= 250000) return null;
        (list.kind === 'custom' ? customTags : censoredTags).push(tag);
      }
    }
  }
  return { customTags, censoredTags };
}

/** Project only command-owned semantic fields for admission; a create has no invented host identifier or persisted name. */
export function admitListMutation(
  before: AutocompleteListSnapshot,
  command: AutocompleteListCommand,
  bundled: readonly BundledTag[],
): boolean {
  const lists =
    command.kind === 'create'
      ? [...before.lists, { kind: command.listKind, enabled: true, text: command.text }]
      : command.kind === 'delete'
        ? before.lists.filter((row) => row.id !== command.id)
        : before.lists.map((row) =>
            row.id !== command.id
              ? row
              : command.kind === 'edit'
                ? { ...row, text: command.text }
                : command.kind === 'enable'
                  ? { ...row, enabled: command.enabled }
                  : row,
          );
  return (
    listCapacity(
      lists.length,
      lists.reduce((units, row) => units + row.text.length, 0),
    ) === null && prepareListRows(lists, bundled) !== null
  );
}

/** Physical line semantics already belong to core; catalog owns normalized deduplication, censorship and lexicon. */
export function prepareLists(
  snapshot: Extract<AutocompleteListHostResult, { status: 'ready' }>['snapshot'],
  bundled: readonly BundledTag[],
): PreparedAutocompleteLists {
  const rows = prepareListRows(snapshot.lists, bundled);
  if (!rows) throw new RangeError('Autocomplete list catalog limit');
  return Object.freeze({
    ...snapshot,
    catalog: prepareTagCompletionCatalog({
      revision: snapshot.revision,
      bundled,
      ...rows,
    }),
  });
}

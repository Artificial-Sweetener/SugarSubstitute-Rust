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
/** Verify host atomic mutation acknowledgements without assuming a persisted name or identifier format. */
import { requireValue } from '../../../core/src/required.js';
import type {
  AutocompleteList,
  AutocompleteListCommand,
  AutocompleteListSnapshot,
} from './contracts.js';
/** Equality covers every authoritative field, independently of host snapshot ordering. */
function same(a: AutocompleteList, b: AutocompleteList): boolean {
  return (
    a.id === b.id &&
    a.name === b.name &&
    a.kind === b.kind &&
    a.enabled === b.enabled &&
    a.text === b.text
  );
}
/** Host revision equality requires exact ordered content equality, including disabled lists and display metadata. */
export function sameListSnapshot(
  a: AutocompleteListSnapshot,
  b: AutocompleteListSnapshot,
): boolean {
  return (
    a.revision === b.revision &&
    a.lists.length === b.lists.length &&
    a.lists.every((row, index) => same(row, requireValue(b.lists[index], 'admission')))
  );
}
/** A ready acknowledgement advances revision and changes exactly its command-owned record. */
export function assertListMutation(
  before: AutocompleteListSnapshot,
  after: AutocompleteListSnapshot,
  command: AutocompleteListCommand,
  affectedId: string | null,
): void {
  if (after.revision === before.revision || affectedId === null)
    throw new TypeError('Invalid autocomplete mutation revision');
  const old =
    command.kind === 'create' ? undefined : before.lists.find((row) => row.id === command.id);
  const changed = after.lists.find((row) => row.id === affectedId);
  const retained = before.lists.filter((row) => row !== old);
  if (
    retained.some((row) => !after.lists.some((next) => same(row, next))) ||
    after.lists.length !==
      before.lists.length + (command.kind === 'create' ? 1 : command.kind === 'delete' ? -1 : 0)
  )
    throw new TypeError('Invalid autocomplete mutation scope');
  if (command.kind === 'create') {
    if (
      !changed ||
      before.lists.some((row) => row.id === affectedId) ||
      changed.kind !== command.listKind ||
      !changed.enabled ||
      changed.text !== command.text
    )
      throw new TypeError('Invalid autocomplete creation');
    return;
  }
  if (!old) throw new TypeError('Invalid autocomplete mutation target');
  if (command.kind === 'delete') {
    if (affectedId !== old.id || changed) throw new TypeError('Invalid autocomplete deletion');
    return;
  }
  if (
    !changed ||
    retained.some((row) => row.id === affectedId) ||
    changed.kind !== old.kind ||
    (command.kind !== 'rename' && (changed.id !== old.id || changed.name !== old.name)) ||
    changed.text !== (command.kind === 'edit' ? command.text : old.text) ||
    changed.enabled !== (command.kind === 'enable' ? command.enabled : old.enabled)
  )
    throw new TypeError('Invalid autocomplete update');
}

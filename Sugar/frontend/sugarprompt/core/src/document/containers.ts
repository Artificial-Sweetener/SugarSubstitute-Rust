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
/** Physical destination contracts complement semantic candidates without changing their enumeration. */
import { requireValue } from '../required.js';
import type {
  PromptDocumentKind,
  PreparedDocumentValues,
  PreparedDocumentContainer,
  DocumentContainerOutcome,
} from './contracts.js';
/** Resolve current source/kind IDs in the canonical snapshot without rebuilding maps or indexes. */
export function documentContainerById(
  prepared: PreparedDocumentValues,
  source: string,
  kind: PromptDocumentKind,
  id: string,
): PreparedDocumentContainer | null {
  if (prepared.source !== source || prepared.kind !== kind) return null;
  return prepared.containersById[id] ?? null;
}
/** Admit an actual current snapshot member; equal bytes and IDs from a separately prepared snapshot remain foreign. */
export function admitDocumentContainer(
  prepared: PreparedDocumentValues,
  source: string,
  kind: PromptDocumentKind,
  container: PreparedDocumentContainer,
): DocumentContainerOutcome {
  if (prepared.source !== source || prepared.kind !== kind)
    return Object.freeze({ status: 'rejected', reason: 'stale' });
  if (prepared.containersById[container.id] !== container)
    return Object.freeze({ status: 'rejected', reason: 'foreign' });
  return requireValue(prepared.containerOutcomes[container.id], 'containers');
}
/** Binary search exact storage bounds; separator interiors and scalar/escape gaps never authorize decoded edits. */
export function documentContainerAtSource(
  prepared: PreparedDocumentValues,
  source: string,
  kind: PromptDocumentKind,
  position: number,
): DocumentContainerOutcome {
  if (prepared.source !== source || prepared.kind !== kind)
    return Object.freeze({ status: 'rejected', reason: 'stale' });
  if (!Number.isSafeInteger(position) || position < 0 || position > source.length)
    return Object.freeze({ status: 'rejected', reason: 'missing' });
  let low = 0,
    high = prepared.containers.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (requireValue(prepared.containers[middle], 'containers').storageRange.to < position)
      low = middle + 1;
    else high = middle;
  }
  const container = prepared.containers[low];
  if (!container || position < container.storageRange.from)
    return Object.freeze({ status: 'rejected', reason: 'missing' });
  const outcome = requireValue(prepared.containerOutcomes[container.id], 'containers');
  if (outcome.status === 'protected') return outcome;
  const mapping = requireValue(container.mapping, 'containers');
  if (mapping.sourceToDecoded[position - mapping.sourceRange.from] == null)
    return Object.freeze({ status: 'rejected', reason: 'unmapped' });
  return outcome;
}

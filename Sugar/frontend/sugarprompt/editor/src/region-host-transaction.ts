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
/** Canonical host names adapt pure minimal source replacements to one native history-mapped synchronization. */
import { requireValue } from '../../core/src/required.js';
import { Annotation, Transaction, type EditorState, type TransactionSpec } from '@codemirror/state';
import { isolateHistory } from '@codemirror/commands';
import { applyRegionNames, type RegionName } from '../../core/src/index.js';
import { sourcePositionsToDocument } from '../../core/src/line-endings.js';
import { lineEndings } from './exact-source.js';
import type { PreparedStructure } from './structure.js';
import { editorFreshness } from './freshness.js';
/** Captured complete names seed peer observation without creating another source buffer or echoed rename. */
export const synchronizedRegionNames = Annotation.define<readonly RegionName[]>();
/** Prepare prompt peer marker-name changes; candidate documents have separate per-value name ownership. */
export function sharedRegionNameEdit(
  state: EditorState,
  semantic: PreparedStructure,
  names: readonly RegionName[],
): TransactionSpec {
  if (semantic.kind !== 'prompt') throw new Error('shared-region-names-require-prompt');
  if (semantic.sourceIdentity !== state.field(editorFreshness).source)
    throw new Error('Stale shared region source');
  const application = applyRegionNames(semantic.structure.document, names);
  const positions = sourcePositionsToDocument(
    state.doc.toString(),
    state.field(lineEndings),
    application.replacements.flatMap((edit) => [edit.from, edit.to]),
  );
  const identities = new Map(names.map((value) => [value.ordinal, value.name]));
  const current = Object.freeze(
    semantic.regionNames.map((value) =>
      identities.has(value.ordinal)
        ? Object.freeze({
            ordinal: value.ordinal,
            name: requireValue(identities.get(value.ordinal), 'region-host-transaction'),
          })
        : value,
    ),
  );
  return {
    changes: application.replacements.map((edit, index) => ({
      from: requireValue(positions[index * 2], 'region-host-transaction'),
      to: requireValue(positions[index * 2 + 1], 'region-host-transaction'),
      insert: edit.text,
    })),
    annotations: [
      Transaction.addToHistory.of(false),
      isolateHistory.of('full'),
      synchronizedRegionNames.of(current),
    ],
    userEvent: 'input.region-sync',
  };
}

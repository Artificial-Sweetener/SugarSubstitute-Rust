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
/** One decoded command delegates storage semantics to core and publishes through one actual CM transaction. */
import { requireValue } from '../../core/src/required.js';
import {
  EditorSelection,
  type Transaction,
  type ChangeSet,
  type EditorState,
} from '@codemirror/state';
import {
  decodedRangeToSource,
  prepareDocumentReplacement,
  prepareDocumentPromptScopes,
  type DocumentReplacementRejection,
  type DocumentReplacementResult,
  type PreparedDocumentSemantics,
  type PreparedDocumentPromptScopes,
  type PreparedDocumentValues,
  type SourceRange,
} from '../../core/src/index.js';
import {
  encodeSource,
  type EncodedSource,
  documentToSource,
  sourcePositionsToDocument,
} from '../../core/src/line-endings.js';
import { exactReplacement, type ExactSourceReplacement } from './exact-source.js';
import {
  currentNativeDocumentValue,
  type NativeDocumentValueContext,
  type NativeDocumentValueRejection,
  type NativeDocumentValueTarget,
  type PreparedNativeDocumentValues,
} from './document-values.js';
import type { DocumentValueEditPublication } from './document-value-edit-publication.js';
/** Decoded endpoints are post-edit UTF-16 relative to the old semantic padding offset, so full-container padding stays selectable. */
export type DocumentValueSelectionRange =
  | {
      readonly kind: 'decoded';
      readonly anchor: number;
      readonly head: number;
    }
  | {
      readonly kind: 'outside';
      readonly index: number;
    };
/** One actual admitted target and decoded edit; the caller supplies current prepared context and directional selection intent. */
export interface DecodedDocumentValueEdit {
  readonly context: NativeDocumentValueContext;
  readonly target: NativeDocumentValueTarget;
  readonly range: SourceRange;
  readonly text: string;
  readonly publication?: DocumentValueEditPublication;
  readonly selection: {
    readonly ranges: readonly DocumentValueSelectionRange[];
    readonly mainIndex: number;
  };
}
/** Expected refusal unites canonical admission/storage codes with native authority and post-output safety. */
export type DocumentValueEditRejection =
  | NativeDocumentValueRejection
  | DocumentReplacementRejection
  | 'unsafe-selection'
  | 'unsafe-origin'
  | 'cardinality';
/** The next structure family adopts these exact post-edit seeds; transactions have one captured actual startState. */
export interface PreparedDocumentValueEdit {
  readonly startState: EditorState;
  readonly target: NativeDocumentValueTarget;
  readonly replacement: Extract<
    DocumentReplacementResult,
    {
      status: 'ready';
    }
  >;
  readonly currentValues: PreparedNativeDocumentValues;
  readonly preparedValues: PreparedDocumentValues;
  readonly postValues: PreparedNativeDocumentValues;
  readonly semantics: PreparedDocumentSemantics;
  readonly transactions: readonly Transaction[];
}
/** No-op commands retain populated derivations and produce at most a selection-only transaction, with no history event. */
export type DocumentValueEditOutcome =
  | {
      readonly status: 'ready';
      readonly edit: PreparedDocumentValueEdit;
    }
  | {
      readonly status: 'unchanged';
      readonly edit: PreparedDocumentValueEdit;
    }
  | {
      readonly status: 'rejected';
      readonly reason: DocumentValueEditRejection;
    };
/** Validate physical destinations/untouched decoded values against actual core output, without another parser or encoder. */
function preservesContainers(
  before: PreparedDocumentValues,
  after: PreparedDocumentValues,
  id: string,
  expected: string,
): boolean {
  if (before.kind === 'prompt') return true;
  if (!after.valid || before.containers.length !== after.containers.length) return false;
  return before.containers.every((container, index) => {
    const next = requireValue(after.containers[index], 'document-value-edits');
    if (
      next.id !== container.id ||
      next.editability !== container.editability ||
      next.mapping?.text !== (container.id === id ? expected : container.mapping?.text)
    )
      return false;
    if (
      container.location.kind === 'txt' &&
      next.location.kind === 'txt' &&
      container.location.record.ending !== next.location.record.ending
    )
      return false;
    if (
      container.location.kind === 'csv' &&
      next.location.kind === 'csv' &&
      (container.location.cell.postQuoteText !== next.location.cell.postQuoteText ||
        (container.location.cell.quoted && !next.location.cell.quoted))
    )
      return false;
    return true;
  });
}
/** Prepare quote-safe edits and full-container directional selection; no transaction is published on a refusal. */
export function prepareDocumentValueEditInput(
  state: EditorState,
  intent: DecodedDocumentValueEdit,
):
  | PreparedDocumentValueEditInput
  | Extract<
      DocumentValueEditOutcome,
      {
        status: 'rejected';
      }
    > {
  const admitted = currentNativeDocumentValue(state, intent.context, intent.target);
  if (admitted.status === 'rejected') return admitted;
  const { target } = admitted;
  const before = target.values.scopes.values;
  const exactRange = decodedRangeToSource(target.scope.mapping, intent.range);
  if (!exactRange) return { status: 'rejected', reason: 'invalid-boundary' };
  const replacement = prepareDocumentReplacement(
    {
      source: before.source,
      kind: before.kind,
      range: exactRange,
      text: intent.text,
    },
    before,
  );
  if (replacement.status === 'rejected') return replacement;
  const preparedValues = replacement.preparedValues;
  const container = target.scope.container;
  const padding = container.valueRange.from;
  const full = container.mapping;
  if (!full) throw new Error('decoded-edit-full-map-missing');
  const expected =
    full.text.slice(0, padding + intent.range.from) +
    intent.text +
    full.text.slice(padding + intent.range.to);
  if (!preservesContainers(before, preparedValues, container.id, expected))
    return { status: 'rejected', reason: 'cardinality' };
  const nextContainer = preparedValues.containersById[container.id];
  if (!nextContainer?.mapping) throw new Error('decoded-edit-post-container-missing');
  const changed = preparedValues.source !== before.source;
  const exact = changed
    ? exactReplacement(state, replacement.range.from, replacement.range.to, replacement.text)
    : null;
  const changes = state.changes(exact?.changes ?? []);
  const ranges = intent.selection.ranges;
  if (
    !ranges.length ||
    !Number.isSafeInteger(intent.selection.mainIndex) ||
    intent.selection.mainIndex < 0 ||
    intent.selection.mainIndex >= ranges.length
  )
    return { status: 'rejected', reason: 'unsafe-selection' };
  const endpoints: number[] = [];
  for (const range of ranges) {
    if (range.kind !== 'decoded') continue;
    for (const endpoint of [range.anchor, range.head]) {
      const decoded = padding + endpoint;
      const mapped = decodedRangeToSource(nextContainer.mapping, {
        from: decoded,
        to: decoded,
      });
      if (!mapped) return { status: 'rejected', reason: 'unsafe-selection' };
      endpoints.push(mapped.from);
    }
  }
  const encoded = encodeSource(preparedValues.source);
  const logical = sourcePositionsToDocument(encoded.logical, encoded.endings, endpoints);
  let cursor = 0;
  const selected = [];
  for (const range of ranges) {
    if (range.kind === 'decoded')
      selected.push(
        EditorSelection.range(
          requireValue(logical[cursor++], 'document-value-edits'),
          requireValue(logical[cursor++], 'document-value-edits'),
        ),
      );
    else {
      if (!Number.isSafeInteger(range.index) || range.index < 0)
        return { status: 'rejected', reason: 'unsafe-selection' };
      const previous = state.selection.ranges[range.index];
      if (
        !previous ||
        !(
          previous.to < target.entry.logicalStorage.from ||
          previous.from > target.entry.logicalStorage.to
        )
      )
        return { status: 'rejected', reason: 'unsafe-selection' };
      selected.push(previous.map(changes));
      const mapped = requireValue(selected.at(-1), 'document-value-edits');
      for (const [document, endings, values, endpoints] of [
        [state.doc.toString(), target.values.endings, before, [previous.anchor, previous.head]],
        [encoded.logical, encoded.endings, preparedValues, [mapped.anchor, mapped.head]],
      ] as const) {
        for (const position of endpoints) {
          const exactPosition = documentToSource(document, endings, position);
          if (values.sourceOffsets.utf16ToPython[exactPosition] == null)
            return { status: 'rejected', reason: 'unsafe-selection' };
        }
      }
    }
  }
  const selection = EditorSelection.create(selected, intent.selection.mainIndex);
  const retainedSelection = !changed && selection.eq(state.selection);
  const scopes = prepareDocumentPromptScopes(
    preparedValues,
    target.values.syntax,
    target.values.scopes,
  );
  return {
    status: 'prepared',
    state,
    intent,
    target,
    replacement,
    preparedValues,
    exact,
    encoded,
    changes,
    selection,
    retainedSelection,
    scopes,
  };
}
/** Pure actual-target preparation carries no native issuance; construction always calls this owner directly. */
export interface PreparedDocumentValueEditInput {
  readonly status: 'prepared';
  readonly state: EditorState;
  readonly intent: DecodedDocumentValueEdit;
  readonly target: NativeDocumentValueTarget;
  readonly replacement: Extract<
    DocumentReplacementResult,
    {
      status: 'ready';
    }
  >;
  readonly preparedValues: PreparedDocumentValues;
  readonly exact: ExactSourceReplacement | null;
  readonly encoded: EncodedSource;
  readonly changes: ChangeSet;
  readonly selection: EditorSelection;
  readonly retainedSelection: boolean;
  readonly scopes: PreparedDocumentPromptScopes;
}

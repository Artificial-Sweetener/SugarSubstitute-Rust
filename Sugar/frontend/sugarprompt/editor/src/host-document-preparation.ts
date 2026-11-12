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
/** Prepare exact-source bundles against actual native state without mutating revision, focus or configuration. */
import { requireValue } from '../../core/src/required.js';
import {
  EditorSelection,
  type EditorState,
  type Transaction,
  type TransactionSpec,
} from '@codemirror/state';
import { encodeSource, sourceToDocument } from '../../core/src/line-endings.js';
import { readExactSource, exactReplacement } from './exact-source.js';
import { prepareLoraMetadataActions, installLoraMetadataActions } from './lora-metadata-actions.js';
import type { HostUpdate } from './host-update.js';
import type { NativeConfiguration, PreparedNativeConfiguration } from './native-configuration.js';
import { configurationHistoryIntent } from './prompt-history.js';
import { documentSeedSource, type NativeDocumentSeed } from './document-state.js';
/** All output is derived from one final validated native bundle; preparation owns no mutable source state. */
export interface PreparedHostDocument {
  readonly configuration: PreparedNativeConfiguration;
  readonly transaction: Transaction;
  /** Complete prepared input retains exact edits and public history annotations through permission adoption. */
  readonly specification: TransactionSpec;
  readonly baseline: EditorState | undefined;
  readonly readOnly: boolean;
  readonly sourceChanged: boolean;
}
/** Existing native construction owns history initialization and semantic preparation for baselines. */
export type BaselinePreparation = (
  source: string,
  configuration: PreparedNativeConfiguration['values'],
  revision: number,
  readOnly: boolean,
  document?: NativeDocumentSeed,
) => EditorState;
/** Validate source, final native configuration and directional exact-source boundaries before adoption. */
export function prepareHostDocument(
  state: EditorState,
  update: HostUpdate,
  configuration: NativeConfiguration,
  revision: number,
  createBaseline: BaselinePreparation,
  document?: NativeDocumentSeed,
): PreparedHostDocument {
  const source =
    update.mode === 'preserve'
      ? readExactSource(state)
      : update.mode === 'restore-document'
        ? documentSeedSource(requireValue(document, 'host-document-preparation'))
        : update.source;
  if (typeof source !== 'string') throw new TypeError('Invalid source');
  if (
    'configuration' in update &&
    (!update.configuration ||
      typeof update.configuration !== 'object' ||
      Array.isArray(update.configuration))
  )
    throw new TypeError('Invalid configuration');
  if (
    'selection' in update &&
    (!update.selection || typeof update.selection !== 'object' || Array.isArray(update.selection))
  )
    throw new TypeError('Invalid selection');
  const readOnly = update.configuration?.readOnly ?? state.readOnly;
  const prepared = configuration.prepare(
    update.configuration ?? {},
    revision + (update.mode === 'baseline' || update.mode === 'restore-document' ? 1 : 0),
  );
  const encoded = encodeSource(source);
  const requested =
    update.selection ??
    (update.mode === 'preserve' || update.mode === 'restore-document'
      ? undefined
      : { anchor: source.length, head: source.length });
  if (
    requested &&
    (typeof requested !== 'object' ||
      Object.keys(requested).sort().join() !== 'anchor,head' ||
      !Number.isSafeInteger(requested.anchor) ||
      !Number.isSafeInteger(requested.head))
  )
    throw new TypeError('Invalid selection');
  const selection = requested
    ? EditorSelection.single(
        sourceToDocument(encoded.logical, encoded.endings, requested.anchor),
        sourceToDocument(encoded.logical, encoded.endings, requested.head),
      )
    : undefined;
  const sourceChanged = source !== readExactSource(state);
  const replacement =
    update.mode === 'exact-edit' && sourceChanged
      ? exactReplacement(state, 0, readExactSource(state).length, source)
      : undefined;
  const selected =
    update.mode !== 'baseline' &&
    update.mode !== 'restore-document' &&
    selection &&
    !(update.mode === 'exact-edit' && !sourceChanged && !update.selection);
  const effects = [...(replacement?.effects ?? []), ...prepared.effects];
  let specification: TransactionSpec = {
    ...(replacement ?? {}),
    ...(selected ? { selection } : {}),
    effects,
  };
  let transaction = state.update(specification);
  if (prepared.values.loraMetadataHost !== configuration.values.loraMetadataHost) {
    specification = {
      ...(replacement ?? {}),
      ...(selected ? { selection } : {}),
      effects: [
        ...effects,
        installLoraMetadataActions.of(prepareLoraMetadataActions(transaction.state)),
      ],
    };
    transaction = state.update(specification);
  }
  const finalEffects = Array.isArray(specification.effects)
    ? specification.effects
    : specification.effects
      ? [specification.effects]
      : [];
  const historyIntent = configurationHistoryIntent(prepared.changedKeys, finalEffects);
  if (historyIntent) {
    specification = {
      ...specification,
      effects: [...finalEffects, historyIntent],
    };
    transaction = state.update(specification);
  }
  const baseline =
    update.mode === 'baseline' || update.mode === 'restore-document'
      ? createBaseline(source, prepared.values, revision + 1, readOnly, document).update({
          ...(selection ? { selection } : {}),
        }).state
      : undefined;
  return {
    configuration: prepared,
    transaction,
    specification,
    baseline,
    readOnly,
    sourceChanged,
  };
}

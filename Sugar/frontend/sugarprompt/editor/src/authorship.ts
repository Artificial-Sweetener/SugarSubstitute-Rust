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
/** Native logical UTF-16 authoring metadata retains generated depth and explicit escape intent through history. */
import { requireValue } from '../../core/src/required.js';
import {
  StateEffect,
  StateField,
  type Extension,
  type EditorState,
  type ChangeSet,
} from '@codemirror/state';
import { invertedEffects } from '@codemirror/commands';
import type { PreparedDocumentPromptScopes } from '../../core/src/index.js';
import { sourcePositionsToDocument, type EncodedSource } from '../../core/src/line-endings.js';
import type { NativeDocumentValueTarget } from './document-values.js';
import {
  consumeDocumentValueNormalization,
  type DocumentValueNormalizationProvenance,
} from './document-value-ingestion.js';
import {
  generatedShells,
  literalIntents,
  mappedAuthorship,
  preparedAuthorship,
  type Authorship,
  type GeneratedShell,
} from './authorship-ranges.js';
const empty = preparedAuthorship([], []);
/** Install prepared authoring metadata with native history inversion. */
export const installAuthorship = StateEffect.define<Authorship>();
/** The finite EditorState owns all authoring provenance and manual overrides. */
export const authorship = StateField.define<Authorship>({
  /** Exact baselines introduce no generated or manual authoring history. */
  create() {
    return empty;
  },
  /** Native edits map metadata; prepared effects and undo restore the authoritative outcome. */
  update(value, transaction) {
    let next = mappedAuthorship(value, transaction);
    for (const effect of transaction.effects) if (effect.is(installAuthorship)) next = effect.value;
    return next;
  },
});
/** Install native inversion rather than implementing another history owner. */
export const authorshipExtension: Extension = [
  authorship,
  invertedEffects.of((transaction) => {
    const before = transaction.startState.field(authorship),
      after = transaction.state.field(authorship);
    return before === after ? [] : [installAuthorship.of(before)];
  }),
];
/** Prepare native authoring metadata from the sole canonical post-scope parse and genuine finite normalization evidence. */
export function normalizedAuthorship(
  state: EditorState,
  target: NativeDocumentValueTarget,
  provenance: DocumentValueNormalizationProvenance,
  postScopes: PreparedDocumentPromptScopes,
  changes: ChangeSet,
  encodedPost: EncodedSource,
): Authorship {
  consumeDocumentValueNormalization(target, provenance, postScopes);
  const value = state.field(authorship),
    normal = provenance.normalization,
    previousGenerated = generatedShells(value),
    previousLiterals = literalIntents(value),
    native = requireValue(target.entry.semanticMapping, 'authorship'),
    oldContainer = target.scope.container,
    nextContainer = requireValue(postScopes.values.containersById[oldContainer.id], 'authorship'),
    postScope = requireValue(postScopes.byId[target.scope.id], 'authorship'),
    positions: number[] = [];
  /** Queue canonical exact endpoints; negative slots retain already-native untouched ranges. */
  function postBoundary(decoded: number): number {
    const exact = requireValue(nextContainer.mapping, 'authorship').decodedToSource[
      oldContainer.valueRange.from + decoded
    ];
    if (exact == null) throw new Error('decoded-authorship-unmapped');
    positions.push(exact);
    return positions.length - 1;
  }
  /** Observe only an actual semantic inverse slot, never infer coordinates from string bounds. */
  function decoded(position: number): number | null {
    return native.logicalToDecoded[position - native.logicalRange.from] ?? null;
  }
  /** Map old endpoints into the proposed normalization input with native boundary association. */
  function proposed(position: number, assoc: -1 | 1): number {
    const insertion = provenance.insertion;
    if (!insertion) return position;
    const { from, to } = insertion.range;
    if (position < from || (position === from && assoc < 0)) return position;
    if (position > to || (position === to && assoc > 0))
      return position + insertion.text.length - (to - from);
    return from + (assoc < 0 ? 0 : insertion.text.length);
  }
  /** Untouched metadata uses the actual final ChangeSet; admitted metadata uses Core's dense boundary output. */
  function mapped(position: number, assoc: -1 | 1): number {
    const local = decoded(position);
    return local === null
      ? -changes.mapPos(position, assoc) - 1
      : postBoundary(requireValue(normal.boundaryPositions[proposed(local, assoc)], 'authorship'));
  }
  const generated: GeneratedShell[] = [];
  for (const shell of previousGenerated) {
    const from = decoded(shell.from),
      to = decoded(shell.to),
      contentFrom = decoded(shell.contentFrom),
      contentTo = decoded(shell.contentTo);
    if (from !== null && to !== null) {
      const insertion = provenance.insertion;
      if (
        insertion &&
        insertion.range.to > from &&
        insertion.range.from < to &&
        !(
          insertion.range.from === insertion.range.to &&
          (insertion.range.from === from || insertion.range.from === to)
        ) &&
        (contentFrom === null ||
          contentTo === null ||
          insertion.range.from < contentFrom ||
          insertion.range.to > contentTo)
      )
        continue;
      if (
        normal.transitions.some(
          (transition) => transition.from <= proposed(from, 1) && proposed(to, -1) <= transition.to,
        )
      )
        continue;
    }
    generated.push({
      ...shell,
      from: mapped(shell.from, 1),
      to: mapped(shell.to, -1),
      contentFrom: mapped(shell.contentFrom, -1),
      contentTo: mapped(shell.contentTo, 1),
    });
  }
  for (const shell of normal.generatedEmphases) {
    const offset = oldContainer.valueRange.from - nextContainer.valueRange.from;
    const span = postScope.document.emphasisSpans.find(
      (span) =>
        span.outerRange.from === shell.from + offset && span.outerRange.to === shell.to + offset,
    );
    if (!span) throw new Error('decoded-authorship-shell-missing');
    generated.push({
      from: postBoundary(shell.from),
      to: postBoundary(shell.to),
      contentFrom: postBoundary(span.contentRange.from - offset),
      contentTo: postBoundary(span.contentRange.to - offset),
      nestingDepth: shell.nestingDepth,
    });
  }
  const literals = previousLiterals
    .filter((intent) => {
      const from = decoded(intent.from),
        to = decoded(intent.to),
        insertion = provenance.insertion;
      return (
        !insertion ||
        from === null ||
        to === null ||
        (!(insertion.range.from <= from && insertion.range.to >= to) &&
          !(insertion.range.from < to && insertion.range.to > from && insertion.text.includes(',')))
      );
    })
    .map((intent) => ({
      from: mapped(intent.from, -1),
      to: mapped(intent.to, 1),
    }));
  if (provenance.manualPosition !== null) {
    const position = requireValue(
        normal.boundaryPositions[provenance.manualPosition],
        'authorship',
      ),
      from = normal.text.lastIndexOf(',', position - 1) + 1,
      comma = normal.text.indexOf(',', position),
      to = comma < 0 ? normal.text.length : comma;
    literals.push({ from: postBoundary(from), to: postBoundary(to) });
  }
  const logical = sourcePositionsToDocument(encodedPost.logical, encodedPost.endings, positions);
  /** Resolve queued canonical endpoints after one exact-source/native conversion batch. */
  const resolve = (slot: number): number =>
    slot < 0 ? -slot - 1 : requireValue(logical[slot], 'authorship');
  const outputGenerated = generated.map((shell) =>
    Object.freeze({
      ...shell,
      from: resolve(shell.from),
      to: resolve(shell.to),
      contentFrom: resolve(shell.contentFrom),
      contentTo: resolve(shell.contentTo),
    }),
  );
  const outputLiterals = literals.map((intent) =>
    Object.freeze({ from: resolve(intent.from), to: resolve(intent.to) }),
  );
  outputGenerated.sort((a, b) => a.from - b.from);
  outputLiterals.sort((a, b) => a.from - b.from);
  const uniqueGenerated = [
    ...new Map(outputGenerated.map((shell) => [`${shell.from}:${shell.to}`, shell])).values(),
  ];
  const uniqueLiterals = [
    ...new Map(outputLiterals.map((intent) => [`${intent.from}:${intent.to}`, intent])).values(),
  ];
  if (
    JSON.stringify(uniqueGenerated) === JSON.stringify(previousGenerated) &&
    JSON.stringify(uniqueLiterals) === JSON.stringify(previousLiterals)
  )
    return value;
  return preparedAuthorship(uniqueGenerated, uniqueLiterals);
}

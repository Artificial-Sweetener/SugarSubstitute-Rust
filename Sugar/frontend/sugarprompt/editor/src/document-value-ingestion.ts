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
/** Core alone normalizes admitted decoded values; weak preparation evidence grants no native edit authority. */
import { requireValue } from '../../core/src/required.js';
import {
  normalizePasteRange,
  normalizeTypedEdit,
  normalizeTypedDeletion,
  type SourceNormalization,
  type SourceRange,
  type TagLexicon,
  type PreparedDocumentPromptScopes,
} from '../../core/src/index.js';
import { identityNormalization } from '../../core/src/normalization/replacements.js';
import { generatedShells, type Authorship } from './authorship-ranges.js';
import type { NativeDocumentValueTarget, NativeDocumentValueContext } from './document-values.js';
import { currentNativeDocumentValue } from './document-values.js';
import type { EditorState } from '@codemirror/state';
import { prepareDuplicateEdit } from '../../core/src/diagnostics/duplicate-edits.js';
import {
  currentDuplicateDiagnostic,
  type DuplicateDiagnosticValue,
} from './duplicate-diagnostics.js';
/** Coordinates use decoded scope-local UTF-16; insertion describes old value to proposed normalization input. */
export interface DocumentValueNormalizationProvenance {
  readonly normalization: SourceNormalization;
  readonly insertion: {
    readonly range: SourceRange;
    readonly text: string;
  } | null;
  readonly manualPosition: number | null;
}
/** Preparation captures only finite evidence for one actual target and one consumption. */
interface NormalizationPreparation {
  readonly target: NativeDocumentValueTarget;
  readonly contents: string;
  readonly proposedLength: number;
}
const prepared = new WeakMap<DocumentValueNormalizationProvenance, NormalizationPreparation>();
/** A source range must have ordered integral boundaries in the current decoded value. */
function validRange(range: SourceRange, length: number): boolean {
  return (
    Number.isSafeInteger(range.from) &&
    Number.isSafeInteger(range.to) &&
    range.from >= 0 &&
    range.from <= range.to &&
    range.to <= length
  );
}
/** Capture the owner's complete finite public output before it enters native adaptation. */
function retain(
  target: NativeDocumentValueTarget,
  normalization: SourceNormalization,
  insertion: DocumentValueNormalizationProvenance['insertion'],
  manualPosition: number | null,
  proposedLength: number,
): DocumentValueNormalizationProvenance {
  const provenance = Object.freeze({
    normalization,
    insertion,
    manualPosition,
  });
  prepared.set(provenance, {
    target,
    contents: JSON.stringify(provenance),
    proposedLength,
  });
  return provenance;
}
/** Prepare Core's exact duplicate recipe only for an actual current indexed finding and its genuinely issued native value target. */
export function prepareDocumentValueDuplicate(
  state: EditorState,
  context: NativeDocumentValueContext,
  target: NativeDocumentValueTarget,
  value: DuplicateDiagnosticValue,
  action: 'remove' | 'emphasize-first',
): DocumentValueNormalizationProvenance | null {
  if (!currentDuplicateDiagnostic(state, value) || target.scope !== value.diagnostic.group.scope)
    return null;
  const admitted = currentNativeDocumentValue(state, context, target);
  if (admitted.status !== 'ready') return null;
  const proposed = prepareDuplicateEdit(
    value.diagnostic.group.prepared,
    target.scope.mapping.text,
    value.diagnostic.finding,
    action,
  );
  return proposed.status === 'ready'
    ? retain(target, proposed.output, null, null, target.scope.mapping.text.length)
    : null;
}
/** Normalize a genuine insertion within one actual decoded scope, preserving neighboring storage as storage. */
export function prepareDocumentValuePaste(
  target: NativeDocumentValueTarget,
  range: SourceRange,
  text: string,
  lexicon?: TagLexicon,
): DocumentValueNormalizationProvenance {
  const before = target.scope.mapping.text;
  if (
    !validRange(range, before.length) ||
    target.scope.mapping.decodedToSource[range.from] == null ||
    target.scope.mapping.decodedToSource[range.to] == null
  )
    throw new RangeError('decoded-ingestion-range');
  const insertion = Object.freeze({ range: Object.freeze({ ...range }), text });
  const proposed = before.slice(0, range.from) + text + before.slice(range.to);
  const end = range.from + text.length;
  const normalized = normalizePasteRange(
    proposed,
    {
      from:
        proposed[range.from - 1] === '\r' && proposed[range.from] === '\n'
          ? range.from - 1
          : range.from,
      to: proposed[end - 1] === '\r' && proposed[end] === '\n' ? end + 1 : end,
    },
    lexicon ? { lexicon } : {},
  );
  return retain(target, normalized, insertion, null, proposed.length);
}
/** Committed native input uses current canonical decoded coordinates and the existing generated-depth owner. */
export function prepareDocumentValueTypedNormalization(
  target: NativeDocumentValueTarget,
  range: SourceRange,
  value: Authorship,
  options: {
    readonly deletion: boolean;
    readonly manual: boolean;
    readonly protected: boolean;
    readonly lexicon?: TagLexicon;
  },
): DocumentValueNormalizationProvenance {
  const source = target.scope.mapping.text;
  if (
    !validRange(range, source.length) ||
    target.scope.mapping.decodedToSource[range.from] == null ||
    target.scope.mapping.decodedToSource[range.to] == null
  )
    throw new RangeError('decoded-ingestion-range');
  const mapping = requireValue(target.entry.semanticMapping, 'document-value-ingestion');
  const generatedEmphases = generatedShells(value).flatMap((shell) => {
    const from = mapping.logicalToDecoded[shell.from - mapping.logicalRange.from];
    const to = mapping.logicalToDecoded[shell.to - mapping.logicalRange.from];
    return from == null || to == null ? [] : [{ from, to, nestingDepth: shell.nestingDepth }];
  });
  const normalization =
    options.manual || options.protected
      ? identityNormalization(source)
      : options.deletion
        ? normalizeTypedDeletion(source, range.to)
        : normalizeTypedEdit(
            source,
            { ...range, replacementText: source.slice(range.from, range.to) },
            {
              ...(options.lexicon ? { lexicon: options.lexicon } : {}),
              generatedEmphases,
            },
          );
  return retain(target, normalization, null, options.manual ? range.from : null, source.length);
}
/** Consume exact preparation once and validate dense boundary slots and actual canonical post-value correspondence. */
export function consumeDocumentValueNormalization(
  target: NativeDocumentValueTarget,
  provenance: DocumentValueNormalizationProvenance,
  postScopes: PreparedDocumentPromptScopes,
): void {
  const evidence = prepared.get(provenance);
  prepared.delete(provenance);
  if (!evidence || evidence.target !== target || evidence.contents !== JSON.stringify(provenance))
    throw new Error('decoded-normalization-foreign');
  const normal = provenance.normalization;
  if (
    normal.boundaryPositions.length !== evidence.proposedLength + 1 ||
    Array.from({ length: normal.boundaryPositions.length }, (_, index) => index).some(
      (index) =>
        !Object.hasOwn(normal.boundaryPositions, index) ||
        !Number.isSafeInteger(normal.boundaryPositions[index]) ||
        requireValue(normal.boundaryPositions[index], 'document-value-ingestion') < 0 ||
        requireValue(normal.boundaryPositions[index], 'document-value-ingestion') >
          normal.text.length,
    ) ||
    normal.transitions.some((range) => !validRange(range, evidence.proposedLength)) ||
    normal.generatedEmphases.some((range) => !validRange(range, normal.text.length)) ||
    (provenance.manualPosition !== null &&
      (!Number.isSafeInteger(provenance.manualPosition) ||
        provenance.manualPosition < 0 ||
        provenance.manualPosition > evidence.proposedLength))
  )
    throw new Error('decoded-normalization-boundaries');
  const old = target.scope.container;
  const next = postScopes.values.containersById[old.id];
  const scope = postScopes.byId[target.scope.id];
  const full = old.mapping;
  if (
    !full ||
    !next?.mapping ||
    !scope ||
    scope.container !== next ||
    next.mapping.text !==
      full.text.slice(0, old.valueRange.from) + normal.text + full.text.slice(old.valueRange.to)
  )
    throw new Error('decoded-normalization-post-value');
}

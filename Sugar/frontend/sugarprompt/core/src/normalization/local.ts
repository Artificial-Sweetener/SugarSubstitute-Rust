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
/** Local ingestion composes authoritative semantics while preserving surrounding source and original transition ranges. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
import type { SourceNormalization, TypedSourceEdit } from './contracts.js';
import { canonicalizeParentheses, type ParenthesisOptions } from './parentheses.js';
import { normalizeForStorage } from './storage.js';
import {
  prepareTypedRegions,
  prepareEmptyRegionInsertion,
  preparePastedRegions,
} from './regions.js';
import { normalizeRange, validateSourceRange } from './ranges.js';
import { reclassifyTypedEmphasis, typedParenthesisScope } from './typed-parentheses.js';
/** Prepare a post-paste exact-source UTF-16 range with minimum separator placement and canonical stored syntax. */
export function normalizePasteRange(
  text: string,
  range: SourceRange,
  options: Pick<ParenthesisOptions, 'lexicon'> = {},
): SourceNormalization {
  validateSourceRange(text, range);
  const empty = prepareEmptyRegionInsertion(text, range),
    first = empty.normalization;
  const separators = preparePastedRegions(first.text, {
      from: requireValue(first.boundaryPositions[range.from], 'local'),
      to: requireValue(first.boundaryPositions[range.to], 'local'),
    }),
    second = separators.normalization;
  const canonical = normalizeRange(
    second.text,
    {
      from: requireValue(
        second.boundaryPositions[requireValue(first.boundaryPositions[range.from], 'local')],
        'local',
      ),
      to: requireValue(
        second.boundaryPositions[requireValue(first.boundaryPositions[range.to], 'local')],
        'local',
      ),
    },
    (source) => normalizeForStorage(source, options),
  );
  return Object.freeze({
    text: canonical.text,
    boundaryPositions: Object.freeze(
      first.boundaryPositions.map((position) =>
        requireValue(
          canonical.boundaryPositions[requireValue(second.boundaryPositions[position], 'local')],
          'local',
        ),
      ),
    ),
    transitions: Object.freeze(
      canonical.transitions.map((transition) =>
        Object.freeze({
          ...transition,
          from: requireValue(
            empty.originalPositions[
              requireValue(separators.originalPositions[transition.from], 'local')
            ],
            'local',
          ),
          to: requireValue(
            empty.originalPositions[
              requireValue(separators.originalPositions[transition.to], 'local')
            ],
            'local',
          ),
        }),
      ),
    ),
    generatedEmphases: canonical.generatedEmphases,
  });
}
/** Prepare one completed typed edit without rewriting unrelated weights or unfinished authoring/composition. */
export function normalizeTypedEdit(
  text: string,
  edit: TypedSourceEdit,
  options: Pick<ParenthesisOptions, 'lexicon' | 'generatedEmphases'> = {},
): SourceNormalization {
  validateSourceRange(text, edit);
  const empty = prepareEmptyRegionInsertion(text, edit),
    preparation =
      empty.normalization.text !== text
        ? empty
        : prepareTypedRegions(text, edit, edit.replacementText),
    region = preparation.normalization;
  if (region.text !== text) {
    if (edit.replacementText.length === 1) return region;
    const from = requireValue(region.boundaryPositions[edit.from], 'local'),
      to = requireValue(region.boundaryPositions[edit.to], 'local');
    const authored = normalizeTypedEdit(
      region.text,
      { from, to, replacementText: region.text.slice(from, to) },
      {
        ...options,
        ...(options.generatedEmphases
          ? {
              generatedEmphases: options.generatedEmphases.map((shell) => ({
                ...shell,
                from: requireValue(region.boundaryPositions[shell.from], 'local'),
                to: requireValue(region.boundaryPositions[shell.to], 'local'),
              })),
            }
          : {}),
      },
    );
    return Object.freeze({
      text: authored.text,
      boundaryPositions: Object.freeze(
        region.boundaryPositions.map((position) =>
          requireValue(authored.boundaryPositions[position], 'local'),
        ),
      ),
      transitions: Object.freeze(
        authored.transitions.map((transition) =>
          Object.freeze({
            ...transition,
            from: requireValue(preparation.originalPositions[transition.from], 'local'),
            to: requireValue(preparation.originalPositions[transition.to], 'local'),
          }),
        ),
      ),
      generatedEmphases: authored.generatedEmphases,
    });
  }
  const explicit = reclassifyTypedEmphasis(text, edit);
  if (explicit) return explicit;
  const scope = typedParenthesisScope(text, edit);
  if (!scope) return region;
  const generated = (options.generatedEmphases ?? [])
    .filter((span) => span.from >= scope.from && span.to <= scope.to)
    .map((span) => ({
      ...span,
      from: span.from - scope.from,
      to: span.to - scope.from,
    }));
  return normalizeRange(text, scope, (source) =>
    canonicalizeParentheses(source, {
      ...options,
      generatedEmphases: generated,
    }),
  );
}

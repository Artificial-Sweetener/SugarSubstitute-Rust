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
/** Document-specific scene capability and candidate-leading marker ownership are pure source semantics. */
import type { SourceRange } from '../offsets.js';
import { freezeStructure } from '../prompt/text.js';
import { prepareDocumentValues } from './values.js';
import type {
  PromptDocumentKind,
  PreparedDocumentSemantics,
  PreparedDocumentValues,
} from './contracts.js';
import { decodedRangeToSource } from './value-mapping.js';

/** Reject malformed host intent before it reaches editor reconfiguration or parsing. */
export function validateDocumentKind(kind: unknown): asserts kind is PromptDocumentKind {
  if (kind !== 'prompt' && kind !== 'wildcard-txt' && kind !== 'wildcard-csv')
    throw new TypeError('Invalid prompt document kind');
}

/** Inspect candidate starts once; an actual matching seed retains post-edit parser/map identity. Mismatched seeds are invariant errors. */
export function prepareDocumentSemantics(
  source: string,
  kind: PromptDocumentKind,
  previous?: PreparedDocumentSemantics,
  seed?: PreparedDocumentValues,
): PreparedDocumentSemantics {
  validateDocumentKind(kind);
  if (seed && (seed.source !== source || seed.kind !== kind))
    throw new Error('document-semantics-seed-mismatch');
  if (
    previous?.preparedValues.source === source &&
    previous.kind === kind &&
    (!seed || previous.preparedValues === seed)
  )
    return previous;
  const ranges: SourceRange[] = [];
  const preparedValues = seed ?? prepareDocumentValues(source, kind);
  for (const value of preparedValues.values)
    if (value.mapping.text.startsWith('**')) {
      const range = decodedRangeToSource(value.mapping, { from: 0, to: 2 });
      if (!range) throw new Error('invalid-scene-marker-mapping');
      ranges.push(range);
    }
  return freezeStructure({
    kind,
    scenesEnabled: preparedValues.scenesEnabled,
    valid: preparedValues.valid,
    unsupportedSceneMarkerRanges: ranges,
    preparedValues,
  });
}

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
/** Exact duplicate cleanup proposals consume prepared findings; native source, selection and history remain editor-owned. */
import { requireValue } from '../required.js';
import type { SourceReplacement, SourceNormalization } from '../normalization/contracts.js';
import { applyReplacements } from '../normalization/replacements.js';
import { addWeights, formatWeight } from '../normalization/decimals.js';
import { freezeStructure } from '../prompt/text.js';
import type { DuplicateFinding, PreparedDuplicateSegments } from './duplicate-segments.js';
/** Removal and emphasis share one source-bound proposal; all replacements use original prompt UTF-16. */
export type DuplicateEditOutcome =
  | {
      readonly status: 'ready';
      readonly edits: readonly SourceReplacement[];
      readonly output: SourceNormalization;
    }
  | {
      readonly status: 'rejected';
      readonly reason: 'stale' | 'foreign' | 'overlap';
    };
/** Remove exactly the later occurrence and its adjacent horizontal comma carrier; hard endings remain authored. */
function removal(source: string, finding: DuplicateFinding): SourceReplacement {
  const range = finding.duplicate.range;
  let after = range.to;
  while (source[after] === ' ' || source[after] === '\t') after++;
  if (source[after] === ',') {
    after++;
    while (source[after] === ' ' || source[after] === '\t') after++;
    return { from: range.from, to: after, text: '' };
  }
  let before = range.from;
  while (source[before - 1] === ' ' || source[before - 1] === '\t') before--;
  return {
    from: source[before - 1] === ',' ? before - 1 : range.from,
    to: range.to,
    text: '',
  };
}
/** Prepare the actual current finding only, preserving positive duplicate excess and final half-even decimal formatting. */
export function prepareDuplicateEdit(
  prepared: PreparedDuplicateSegments,
  source: string,
  finding: DuplicateFinding,
  action: 'remove' | 'emphasize-first',
): DuplicateEditOutcome {
  if (source !== prepared.document.sourceText) return { status: 'rejected', reason: 'stale' };
  if (!prepared.findings.includes(finding)) return { status: 'rejected', reason: 'foreign' };
  const edits: SourceReplacement[] = [removal(source, finding)];
  if (action === 'emphasize-first') {
    const excess = addWeights(finding.duplicate.weight, '-1.00');
    const weight = formatWeight(
      addWeights(addWeights(finding.first.weight, '0.10'), excess.startsWith('-') ? '0' : excess),
    );
    const content = finding.first.contentRange;
    edits.push({
      ...finding.first.range,
      text: `(${source.slice(content.from, content.to)}:${weight})`,
      boundaryPositions: Array.from(
        { length: finding.first.range.to - finding.first.range.from + 1 },
        (_, offset) =>
          1 +
          Math.max(
            0,
            Math.min(content.to - content.from, finding.first.range.from + offset - content.from),
          ),
      ),
    });
  }
  edits.sort((a, b) => a.from - b.from);
  if (
    edits.some(
      (edit, index) =>
        index > 0 && edit.from < requireValue(edits[index - 1], 'duplicate-edits').to,
    )
  )
    return { status: 'rejected', reason: 'overlap' };
  return freezeStructure({
    status: 'ready' as const,
    edits,
    output: applyReplacements(source, edits),
  });
}

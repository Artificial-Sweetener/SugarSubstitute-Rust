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
/** Source-safe wildcard tag transformations consume parsed source; native commands own freshness, caret and history. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
import { utf16ToCodePoint } from '../offsets.js';
import { validateSourceRange } from '../normalization/ranges.js';
import { parsePrompt } from '../prompt/parser.js';
import type { PromptDocument, WildcardSpan, WildcardSyntax } from '../prompt/contracts.js';
import { strip } from '../prompt/text.js';
import { prepareWildcardGroups, type WildcardGroup } from './groups.js';
/** Explicit tags or one-unit numeric steps; text tags never receive numeric controls. */
export type WildcardTagChange =
  | {
      readonly kind: 'set';
      readonly tag: string;
    }
  | {
      readonly kind: 'step';
      readonly direction: 'increase' | 'decrease';
    };
/** Proposed exact source and collapsed content-end caret use exact-source UTF-16, including CRLF units. */
export interface WildcardTagMutation {
  readonly text: string;
  readonly selectionRange: SourceRange;
}
/** Reuse the actual parser on one local placeholder, rejecting edits that close their delimiter or alter the intended tag. */
function tagReplacement(source: string, span: WildcardSpan, tag: string): string | null {
  if (!tag || strip(tag) !== tag) return null;
  const prefix = source.slice(span.outerRange.from, span.contentRange.from),
    suffix = source.slice(span.contentRange.to, span.outerRange.to),
    base = requireValue(
      source.slice(span.contentRange.from, span.contentRange.to).split('|', 1)[0],
      'tag-mutations',
    );
  const replacement = `${base}|${tag}`,
    candidate = `${prefix}${replacement}${suffix}`;
  const profile: WildcardSyntax =
    prefix === '{' && suffix === '}'
      ? { style: 'curly' }
      : { style: 'custom', prefix, suffix, alsoRecognizeCurly: false };
  const parsed = parsePrompt(candidate, profile).wildcardSpans;
  return parsed.length === 1 &&
    requireValue(parsed[0], 'tag-mutations').outerRange.from === 0 &&
    requireValue(parsed[0], 'tag-mutations').outerRange.to === candidate.length &&
    requireValue(parsed[0], 'tag-mutations').tag === tag
    ? replacement
    : null;
}
/** Set or step one parsed outer range; supplied canonical groups must retain every actual document span in order. */
export function mutateWildcardTag(
  document: PromptDocument,
  outer: SourceRange,
  change: WildcardTagChange,
  preparedGroups?: readonly WildcardGroup[],
): WildcardTagMutation | null {
  const source = document.sourceText;
  validateSourceRange(source, outer);
  utf16ToCodePoint(source, outer.from);
  utf16ToCodePoint(source, outer.to);
  if (
    preparedGroups &&
    (preparedGroups.length !== document.wildcardSpans.length ||
      preparedGroups.some((group, index) => group.span !== document.wildcardSpans[index]))
  )
    throw new TypeError('foreign-wildcard-groups');
  const group = (preparedGroups ?? prepareWildcardGroups(document.wildcardSpans)).find(
    ({ span }) => span.outerRange.from === outer.from && span.outerRange.to === outer.to,
  );
  if (!group) return null;
  let tag: string;
  if (change.kind === 'set') tag = change.tag;
  else {
    if (!group.numeric || group.displayTag === null) return null;
    const stepped = BigInt(group.displayTag) + (change.direction === 'increase' ? 1n : -1n);
    tag = String(stepped < 1n ? 1n : stepped);
  }
  const replacement = tagReplacement(source, group.span, tag);
  if (replacement === null) return null;
  const range = group.span.contentRange,
    caret = range.from + replacement.length;
  return Object.freeze({
    text: source.slice(0, range.from) + replacement + source.slice(range.to),
    selectionRange: Object.freeze({ from: caret, to: caret }),
  });
}

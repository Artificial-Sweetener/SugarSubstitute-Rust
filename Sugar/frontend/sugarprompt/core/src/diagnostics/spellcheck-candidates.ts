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
/** Pure spelling candidates consume canonical decoded scopes and prepared exact-tag knowledge; no backend or request lifetime. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
import type { PreparedDocumentPromptScope } from '../document/prompt-scopes.js';
import type { TagLexicon } from '../normalization/contracts.js';
import { normalizeTagIdentity } from '../normalization/lexicon.js';
import { freezeStructure, strip, whitespace } from '../prompt/text.js';
import { decimalZeros } from '../prompt/unicode-decimal-data.js';
/** A prose-like ASCII word/apostrophe range uses decoded scope-local UTF-16; storage conversion remains the value-map owner. */
export interface SpellcheckCandidate {
  readonly range: SourceRange;
  readonly text: string;
}
/** Fully populated output is reusable only with the same actual canonical scope and exact-tag knowledge. */
export interface PreparedSpellcheckCandidates {
  readonly scope: PreparedDocumentPromptScope;
  readonly lexicon: TagLexicon | null;
  readonly candidates: readonly SpellcheckCandidate[];
}
/** Python3.12 Unicode15 isdigit minus isdecimal; decimal ranges remain owned by the existing pinned grammar data. */
const nondecimalDigits: readonly (readonly [number, number])[] = [
  [178, 179],
  [185, 185],
  [4969, 4977],
  [6618, 6618],
  [8304, 8304],
  [8308, 8313],
  [8320, 8329],
  [9312, 9320],
  [9332, 9340],
  [9352, 9360],
  [9450, 9450],
  [9461, 9469],
  [9471, 9471],
  [10102, 10110],
  [10112, 10120],
  [10122, 10130],
  [68160, 68163],
  [69216, 69224],
  [69714, 69722],
  [127232, 127242],
];
/** Classify only the pinned digit repertoire, including superscripts; fractions/Roman numerals do not acquire digit status. */
function containsDigit(text: string): boolean {
  for (const character of text) {
    const point = requireValue(character.codePointAt(0), 'spellcheck-candidates');
    if (
      decimalZeros.some((zero) => point >= zero && point < zero + 10) ||
      nondecimalDigits.some(([from, to]) => point >= from && point <= to)
    )
      return true;
  }
  return false;
}
/** Exclude prompt syntax/paths/hash/model metadata without altering the native or decoded source. */
function skippedToken(token: string): boolean {
  const text = strip(token);
  return (
    !text ||
    /[_/\\<>:]/u.test(text) ||
    text.startsWith('#') ||
    text.startsWith('**') ||
    /^(?:#|0x)?[0-9a-fA-F]{8,}$/u.test(text) ||
    /^[A-Za-z0-9_-]+\.[A-Za-z0-9]{1,8}$/u.test(text) ||
    /[a-z][A-Z]|[A-Z]{2,}[a-z]/u.test(text) ||
    containsDigit(text)
  );
}
/** Locate the surrounding whitespace/comma token; this lexical boundary never interprets bracket or document grammar. */
function tokenAt(source: string, from: number, to: number): SourceRange {
  while (
    from > 0 &&
    source[from - 1] !== ',' &&
    !whitespace.test(requireValue(source[from - 1], 'spellcheck-candidates'))
  )
    from--;
  while (
    to < source.length &&
    source[to] !== ',' &&
    !whitespace.test(requireValue(source[to], 'spellcheck-candidates'))
  )
    to++;
  return { from, to };
}
/** Prepare outside typing/measurement; syntax ranges and complete known-tag segments never reach a spelling backend. */
export function prepareSpellcheckCandidates(
  scope: PreparedDocumentPromptScope,
  lexicon?: TagLexicon,
  previous?: PreparedSpellcheckCandidates,
): PreparedSpellcheckCandidates {
  if (previous?.scope === scope && previous.lexicon === (lexicon ?? null)) return previous;
  const document = scope.document,
    source = document.sourceText;
  const decisions = new Map<string, boolean>();
  /** Memoize bounded per-preparation membership by the existing authoritative tag identity. */
  const known = (text: string): boolean => {
    if (!lexicon) return false;
    const key = normalizeTagIdentity(text);
    if (!key) return false;
    const cached = decisions.get(key);
    if (cached !== undefined) return cached;
    const value = lexicon.contains(text);
    decisions.set(key, value);
    return value;
  };
  const segmentSkipped = new Map<number, boolean>();
  const candidates: SpellcheckCandidate[] = [];
  let syntaxIndex = 0,
    segmentIndex = 0;
  let tokenRange: SourceRange = { from: 0, to: 0 },
    tokenSkipped = false;
  for (const match of source.matchAll(/[A-Za-z][A-Za-z']*/gu)) {
    const from = match.index,
      to = from + match[0].length;
    while (
      document.syntaxSpans[syntaxIndex] &&
      requireValue(document.syntaxSpans[syntaxIndex], 'spellcheck-candidates').sourceRange.to <=
        from
    )
      syntaxIndex++;
    const syntax = document.syntaxSpans[syntaxIndex];
    if (syntax && syntax.sourceRange.from <= from && to <= syntax.sourceRange.to) continue;
    if (from >= tokenRange.to) {
      tokenRange = tokenAt(source, from, to);
      tokenSkipped = skippedToken(source.slice(tokenRange.from, tokenRange.to));
    }
    if (tokenSkipped || known(match[0])) continue;
    while (
      document.segments[segmentIndex] &&
      requireValue(document.segments[segmentIndex], 'spellcheck-candidates').contentRange.to < from
    )
      segmentIndex++;
    const segment = document.segments[segmentIndex];
    if (segment && segment.contentRange.from <= from) {
      let skipped = segmentSkipped.get(segmentIndex);
      if (skipped === undefined) {
        const text = source.slice(segment.contentRange.from, segment.contentRange.to);
        skipped = skippedToken(text) || known(text);
        segmentSkipped.set(segmentIndex, skipped);
      }
      if (skipped) continue;
    }
    candidates.push({ range: { from, to }, text: match[0] });
  }
  return freezeStructure({ scope, lexicon: lexicon ?? null, candidates });
}

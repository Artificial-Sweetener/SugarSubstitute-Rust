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
/** Tolerant source scanner; exact ranges and separators feed native editor extensions. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
import type {
  EmphasisSpan,
  LoraSpan,
  PromptDocument,
  PromptSegment,
  RegionSeparator,
  SyntaxSpan,
  WildcardSyntax,
} from './contracts.js';
import { loraSpan } from './lora.js';
import { regionSeparatorAt, regionStructure } from './regions.js';
import { freezeStructure, isStructuralQuote, strip } from './text.js';
import { alternateWildcards, wildcardDelimiters, wildcardSpan } from './wildcards.js';
import { weightField } from './weights.js';
/** One active bracket with its own final colon; literal parentheses shield their entire contents. */
interface BracketFrame {
  readonly opener: string;
  readonly closer: string;
  readonly from: number;
  readonly literal: boolean;
  colon: number | null;
  readonly commas: number[];
  readonly hardBreaks: SourceRange[];
}
/** Close direct segment ranges from commas already observed on one valid emphasis frame. */
function emphasisSegments(
  source: string,
  from: number,
  to: number,
  commas: readonly number[],
  hardBreaks: readonly SourceRange[],
): PromptSegment[] {
  const segments: PromptSegment[] = [];
  let start = from;
  let breakIndex = 0;
  /** Consume direct ending records once, assigning each to its exact content rather than comma padding. */
  const endings = (end: number): SourceRange[] => {
    const result: SourceRange[] = [];
    while (
      breakIndex < hardBreaks.length &&
      requireValue(hardBreaks[breakIndex], 'parser').to <= end
    ) {
      const range = requireValue(hardBreaks[breakIndex++], 'parser');
      if (range.from >= start) result.push(range);
    }
    return result;
  };
  for (const comma of commas) {
    if (comma >= to) break;
    let end = comma + 1;
    while (end < to && ' \t\r\n'.includes(requireValue(source[end], 'parser'))) end++;
    segments.push({
      index: segments.length,
      text: source.slice(start, comma),
      contentRange: { from: start, to: comma },
      separatorRange: { from: comma, to: end },
      hardBreaks: endings(comma),
    });
    start = end;
  }
  if (start < to)
    segments.push({
      index: segments.length,
      text: source.slice(start, to),
      contentRange: { from: start, to },
      separatorRange: null,
      hardBreaks: endings(to),
    });
  return segments;
}
/** Grammar bracket pairs; closers recover only their current matching opener. */
const pairs: Readonly<Record<string, string>> = {
  '(': ')',
  '[': ']',
  '{': '}',
  '<': '>',
};
/** Sort and assign same-kind depth with one nesting stack over well-nested scanner output. */
function withDepth<
  T extends {
    readonly outerRange: SourceRange;
    readonly depth: number;
  },
>(spans: T[]): T[] {
  spans.sort((a, b) => a.outerRange.from - b.outerRange.from || a.outerRange.to - b.outerRange.to);
  const ends: number[] = [];
  return spans.map((span) => {
    while (ends.length && requireValue(ends[ends.length - 1], 'parser') <= span.outerRange.from)
      ends.pop();
    const next = { ...span, depth: ends.length };
    ends.push(span.outerRange.to);
    return next;
  });
}
/** Parse source without normalization. Returned trees are frozen and ranges use exact-source UTF-16. */
export function parsePrompt(
  source: string,
  profile: WildcardSyntax = { style: 'curly' },
): PromptDocument {
  const delimiters = wildcardDelimiters(profile);
  const wildcards = alternateWildcards(source, delimiters);
  const alternateAt = new Map(wildcards.map((span) => [span.outerRange.from, span]));
  const curly = delimiters.some(
    (delimiter) => delimiter.prefix === '{' && delimiter.suffix === '}',
  );
  const segments: PromptSegment[] = [],
    emphasis: EmphasisSpan[] = [],
    loras: LoraSpan[] = [],
    regions: RegionSeparator[] = [];
  const stack: BracketFrame[] = [];
  let segmentBreaks: SourceRange[] = [];
  let quote: string | null = null,
    escape = false,
    segmentFrom = 0,
    index = 0,
    angleDepth = 0;
  while (index < source.length) {
    const alternate = alternateAt.get(index);
    if (alternate && !stack.length && quote === null) {
      index = alternate.outerRange.to;
      continue;
    }
    const character = requireValue(source[index], 'parser');
    if (quote !== null) {
      if (escape) escape = false;
      else if (character === '\\') escape = true;
      else if (character === quote && isStructuralQuote(source, index)) quote = null;
      index++;
      continue;
    }
    const top = stack[stack.length - 1];
    if (top?.literal) {
      if (character === '\\' && index + 1 < source.length) {
        const next = source[index + 1];
        if (next === '(') {
          stack.push({
            opener: '\\(',
            closer: ')',
            from: index,
            literal: true,
            colon: null,
            commas: [],
            hardBreaks: [],
          });
          index += 2;
          continue;
        }
        if (next === ')') {
          stack.pop();
          index += 2;
          continue;
        }
      }
      index++;
      continue;
    }
    if (
      (character === "'" || character === '"') &&
      angleDepth === 0 &&
      isStructuralQuote(source, index)
    ) {
      quote = character;
      index++;
      continue;
    }
    if (character === '[') {
      const separator = regionSeparatorAt(source, index);
      if (separator) {
        regions.push(separator);
        stack.length = 0;
        angleDepth = 0;
        index = separator.tokenRange.to;
        continue;
      }
    }
    if (character === '\\') {
      if (source[index + 1] === '(')
        stack.push({
          opener: '\\(',
          closer: ')',
          from: index,
          literal: true,
          colon: null,
          commas: [],
          hardBreaks: [],
        });
      index += index + 1 < source.length ? 2 : 1;
      continue;
    }
    const closer = pairs[character];
    if (closer) {
      stack.push({
        opener: character,
        closer,
        from: index,
        literal: false,
        colon: null,
        commas: [],
        hardBreaks: [],
      });
      if (character === '<') angleDepth++;
      index++;
      continue;
    }
    if (')]}>'.includes(character) && top) {
      if (top.closer === character) {
        stack.pop();
        if (top.opener === '<') {
          angleDepth--;
          const span = loraSpan(source, top.from, index);
          if (span) loras.push(span);
        } else if (top.opener === '(' && top.colon !== null) {
          const weight = weightField(source, top.colon + 1, index, false);
          if (weight)
            emphasis.push({
              outerRange: { from: top.from, to: index + 1 },
              contentRange: { from: top.from + 1, to: top.colon },
              weightRange: weight.range,
              weight: weight.value,
              depth: 0,
              segments: emphasisSegments(
                source,
                top.from + 1,
                top.colon,
                top.commas,
                top.hardBreaks,
              ),
            });
        } else if (top.opener === '{' && curly && !stack.some((frame) => frame.opener === '{')) {
          const span = wildcardSpan(source, top.from, index + 1, top.from + 1, index, true);
          if (span) wildcards.push(span);
        }
      }
      index++;
      continue;
    }
    if (character === ':' && top) top.colon = index;
    if (character === ',' && top?.opener === '(') top.commas.push(index);
    if (
      (character === '\r' || character === '\n') &&
      !(character === '\n' && source[index - 1] === '\r') &&
      (!top || top.opener === '(')
    ) {
      const to = index + (character === '\r' && source[index + 1] === '\n' ? 2 : 1);
      (top?.hardBreaks ?? segmentBreaks).push({ from: index, to });
      index = to;
      continue;
    }
    if (character === ',' && !stack.length) {
      let to = index + 1;
      while (to < source.length && ' \t\r\n'.includes(requireValue(source[to], 'parser'))) to++;
      segments.push({
        index: segments.length,
        text: source.slice(segmentFrom, index),
        contentRange: { from: segmentFrom, to: index },
        separatorRange: { from: index, to },
        hardBreaks: segmentBreaks,
      });
      segmentFrom = to;
      segmentBreaks = [];
      index = to;
      continue;
    }
    index++;
  }
  if (segmentFrom < source.length)
    segments.push({
      index: segments.length,
      text: source.slice(segmentFrom),
      contentRange: { from: segmentFrom, to: source.length },
      separatorRange: null,
      hardBreaks: segmentBreaks,
    });
  const emphasisSpans = withDepth(emphasis),
    loraSpans = withDepth(loras);
  const wildcardSpans = wildcards.sort(
    (a, b) => a.outerRange.from - b.outerRange.from || a.outerRange.to - b.outerRange.to,
  );
  const syntaxSpans: SyntaxSpan[] = [
    ...emphasisSpans.map(
      (span): SyntaxSpan => ({
        kind: 'emphasis',
        sourceRange: span.outerRange,
        depth: span.depth,
      }),
    ),
    ...wildcardSpans.map(
      (span): SyntaxSpan => ({
        kind: 'wildcard',
        sourceRange: span.outerRange,
        depth: span.depth,
      }),
    ),
    ...loraSpans.map(
      (span): SyntaxSpan => ({
        kind: 'lora',
        sourceRange: span.outerRange,
        depth: span.depth,
      }),
    ),
  ].sort((a, b) => a.sourceRange.from - b.sourceRange.from || a.sourceRange.to - b.sourceRange.to);
  return freezeStructure({
    sourceText: source,
    segments,
    emphasisSpans,
    wildcardSpans,
    loraSpans,
    syntaxSpans,
    regions: regionStructure(source.length, regions),
    hasTrailingComma: strip(source, 'right').endsWith(','),
  });
}
/** Serialize a parsed document exactly; normalization is a separate source transformation. */
export function serializePrompt(document: PromptDocument): string {
  return document.sourceText;
}

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
/** Canonical parenthesis classification consumes prepared membership and the authoritative syntax parser. */
import { requireValue } from '../required.js';
import { parsePrompt } from '../prompt/parser.js';
import type { EmphasisSpan } from '../prompt/contracts.js';
import type {
  GeneratedEmphasis,
  ParenthesisTransition,
  SourceNormalization,
  SourceReplacement,
  TagLexicon,
} from './contracts.js';
import { implicitWeight } from './decimals.js';
import { parenthesisPairs, type ParenthesisPair } from './parenthesis-pairs.js';
import { applyReplacements } from './replacements.js';
import { sequenceBoundaryMap } from './sequence-map.js';
/** Prepared semantic inputs; membership is exact, synchronous and free of catalog traversal or I/O. */
export interface ParenthesisOptions {
  readonly lexicon?: TagLexicon;
  readonly forceLiteral?: boolean;
  readonly generatedEmphases?: readonly GeneratedEmphasis[];
}
/** One pending rendering instruction; the explicit stack supports deeply nested authored input. */
type RenderTask =
  | {
      readonly kind: 'close';
      readonly weight: string;
      readonly from: number;
      readonly depth: number | null;
    }
  | {
      readonly kind: 'range';
      readonly from: number;
      readonly to: number;
    };
/** Render implicit groups while retaining manually authored explicit weights and their exact spelling. */
function renderUnknown(
  text: string,
  pairs: readonly ParenthesisPair[],
  explicit: ReadonlyMap<number, EmphasisSpan>,
  generated: ReadonlyMap<number, GeneratedEmphasis>,
): {
  readonly text: string;
  readonly depth: number;
  readonly generated: readonly GeneratedEmphasis[];
} {
  const byOpen = new Map(pairs.map((pair) => [pair.from, pair]));
  const tasks: RenderTask[] = [{ kind: 'range', from: 0, to: text.length }],
    parts: string[] = [],
    generatedShells: GeneratedEmphasis[] = [];
  let maximum = 1,
    length = 0;
  /** Append one rendered fragment and maintain output coordinates without joining partial text. */
  function append(value: string): void {
    parts.push(value);
    length += value.length;
  }
  while (tasks.length) {
    const task = requireValue(tasks.pop(), 'parentheses');
    if (task.kind === 'close') {
      append(':' + task.weight + ')');
      if (task.depth !== null)
        generatedShells.push({
          from: task.from,
          to: length,
          nestingDepth: task.depth,
        });
      continue;
    }
    let cursor = task.from;
    while (cursor < task.to) {
      const pair = byOpen.get(cursor);
      if (!pair || pair.close >= task.to) {
        append(requireValue(text[cursor], 'parentheses'));
        cursor++;
        continue;
      }
      const weighted = explicit.get(pair.from);
      let from = pair.from + 1,
        to = pair.close,
        weight: string,
        generatedDepth: number | null = null;
      if (weighted?.outerRange.to === pair.close + 1) {
        from = weighted.contentRange.from;
        to = weighted.contentRange.to;
        weight = text.slice(weighted.weightRange.from, weighted.weightRange.to);
        const authored = generated.get(pair.from);
        if (authored?.to === pair.close + 1) generatedDepth = authored.nestingDepth;
      } else {
        let depth = 1;
        while (from < to) {
          const nested = byOpen.get(from);
          if (!nested || nested.close !== to - 1) break;
          const inner = explicit.get(nested.from);
          if (inner?.outerRange.to === nested.close + 1) {
            const authored = generated.get(nested.from);
            if (!authored || authored.to !== nested.close + 1) break;
            depth += authored.nestingDepth;
            from = inner.contentRange.from;
            to = inner.contentRange.to;
          } else {
            depth++;
            from = nested.from + 1;
            to = nested.close;
          }
        }
        maximum = Math.max(maximum, depth);
        weight = implicitWeight(depth);
        generatedDepth = depth;
      }
      const shellFrom = length;
      append('(');
      tasks.push(
        { kind: 'range', from: pair.close + 1, to: task.to },
        { kind: 'close', weight, from: shellFrom, depth: generatedDepth },
        { kind: 'range', from, to },
      );
      break;
    }
  }
  return {
    text: parts.join(''),
    depth: maximum,
    generated: generatedShells.sort((a, b) => a.from - b.from),
  };
}
/** Escape balanced literal delimiters only; unmatched or already escaped characters remain exact. */
function escapeLiteral(text: string, pairs: readonly ParenthesisPair[]): string {
  const positions = new Set(pairs.flatMap((pair) => [pair.from, pair.close]));
  const result: string[] = [];
  for (let index = 0; index < text.length; index++)
    result.push((positions.has(index) ? '\\' : '') + requireValue(text[index], 'parentheses'));
  return result.join('');
}
/** Canonicalize complete segment parentheses and return original-to-result exact-source UTF-16 mappings. */
export function canonicalizeParentheses(
  text: string,
  options: ParenthesisOptions = {},
): SourceNormalization {
  const document = parsePrompt(text),
    edits: SourceReplacement[] = [],
    transitions: ParenthesisTransition[] = [],
    generatedShells: GeneratedEmphasis[] = [];
  let delta = 0;
  for (const generated of options.generatedEmphases ?? []) {
    if (
      !Number.isSafeInteger(generated.nestingDepth) ||
      generated.nestingDepth < 1 ||
      !Number.isInteger(generated.from) ||
      !Number.isInteger(generated.to) ||
      generated.from < 0 ||
      generated.to <= generated.from ||
      generated.to > text.length
    )
      throw new RangeError('Invalid generated emphasis');
  }
  for (const segment of document.segments) {
    const leading = requireValue(/^[ \t]*/u.exec(segment.text), 'parentheses')[0].length,
      trailing = requireValue(/[ \t]*$/u.exec(segment.text), 'parentheses')[0].length;
    const from = segment.contentRange.from + leading,
      to = segment.contentRange.to - trailing;
    if (from >= to) continue;
    const before = text.slice(from, to),
      pairs = parenthesisPairs(before);
    if (!pairs.length) continue;
    const literal = options.forceLiteral === true || options.lexicon?.contains(before) === true;
    const explicit = new Map(
      document.emphasisSpans
        .filter((span) => span.outerRange.from >= from && span.outerRange.to <= to)
        .map((span) => [
          span.outerRange.from - from,
          {
            ...span,
            outerRange: {
              from: span.outerRange.from - from,
              to: span.outerRange.to - from,
            },
            contentRange: {
              from: span.contentRange.from - from,
              to: span.contentRange.to - from,
            },
            weightRange: {
              from: span.weightRange.from - from,
              to: span.weightRange.to - from,
            },
          },
        ]),
    );
    const generated = new Map(
      (options.generatedEmphases ?? [])
        .filter((span) => span.from >= from && span.to <= to)
        .map((span) => [span.from - from, { ...span, from: span.from - from, to: span.to - from }]),
    );
    const rendered = literal
      ? { text: escapeLiteral(before, pairs), depth: 1, generated: [] }
      : renderUnknown(before, pairs, explicit, generated);
    generatedShells.push(
      ...rendered.generated.map((shell) => ({
        ...shell,
        from: from + delta + shell.from,
        to: from + delta + shell.to,
      })),
    );
    if (rendered.text === before) continue;
    delta += rendered.text.length - before.length;
    edits.push({
      from,
      to,
      text: rendered.text,
      boundaryPositions: sequenceBoundaryMap(before, rendered.text),
    });
    transitions.push({
      from,
      to,
      kind: literal ? 'known_literal_tag' : 'implicit_emphasis',
      nestingDepth: rendered.depth,
    });
  }
  return Object.freeze({
    ...applyReplacements(text, edits, transitions),
    generatedEmphases: Object.freeze(generatedShells.map((shell) => Object.freeze(shell))),
  });
}
/** Identify one complete explicit shell without treating a comma-delimited document as a single token. */
export function isExplicitEmphasis(text: string): boolean {
  const document = parsePrompt(text);
  return (
    document.segments.length === 1 &&
    !document.hasTrailingComma &&
    document.emphasisSpans.some(
      (span) => span.outerRange.from === 0 && span.outerRange.to === text.length,
    )
  );
}

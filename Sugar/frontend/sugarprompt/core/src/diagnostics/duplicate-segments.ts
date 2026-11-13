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
/** Prepared duplicate findings consume the sole grammar and scope owners; ranges are decoded/exact prompt UTF-16. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
import type { EmphasisSpan, PromptDocument, SyntaxSpan } from '../prompt/contracts.js';
import type { ConditioningTopology } from '../prompt/conditioning.js';
import type { PreparedSceneContexts } from '../scenes/context.js';
import { freezeStructure, whitespace } from '../prompt/text.js';
import { normalizeTagIdentity } from '../normalization/lexicon.js';
/** Actual prepared ownership decides inheritance; this owner never interprets scene or region syntax. */
export type DuplicateOwnership =
  | {
      readonly kind: 'value';
    }
  | {
      readonly kind: 'conditioning';
      readonly topology: ConditioningTopology;
    }
  | {
      readonly kind: 'scenes';
      readonly contexts: PreparedSceneContexts;
    };
/** One eligible segment retains original authored bytes and the parser's recognized inner content/weight. */
export interface DuplicateOccurrence {
  readonly range: SourceRange;
  readonly contentRange: SourceRange;
  readonly key: string;
  readonly weight: string;
}
/** A later occurrence refers to the actual first occurrence in its effective scope. */
export interface DuplicateFinding {
  readonly scopeId: string;
  readonly first: DuplicateOccurrence;
  readonly duplicate: DuplicateOccurrence;
}
/** Immutable source-bound proof operands retain actual grammar and ownership identities. */
export interface PreparedDuplicateSegments {
  readonly document: PromptDocument;
  readonly ownership: DuplicateOwnership;
  readonly findings: readonly DuplicateFinding[];
}
/** Resolve a whole recognized shell through actual spans; literal parentheses remain authored text. */
function occurrence(
  document: PromptDocument,
  range: SourceRange,
  emphasis: ReadonlyMap<number, EmphasisSpan>,
): DuplicateOccurrence | null {
  const source = document.sourceText;
  let content = range;
  let weight = '1.00';
  for (;;) {
    const span = emphasis.get(content.from);
    if (!span || span.outerRange.to !== content.to) break;
    if (span.weightRange.from !== span.weightRange.to) weight = span.weight;
    content = span.contentRange;
  }
  const text = source.slice(content.from, content.to);
  if (/[<>{}[\]|\\\r\n]/u.test(text)) return null;
  const key = normalizeTagIdentity(text);
  if (
    !key ||
    key.includes('://') ||
    key.includes('/') ||
    /(?:\.safetensors|\.ckpt|\.pt|\.pth|\.onnx)$/u.test(key) ||
    /^[a-f0-9]{8,}$/u.test(key)
  )
    return null;
  return freezeStructure({ range, contentRange: content, key, weight });
}
/** Admit only complete parser-owned segment/hard-line extents contained by actual semantic scope bounds. */
function occurrences(
  document: PromptDocument,
  emphasis: ReadonlyMap<number, EmphasisSpan>,
  blocked: readonly SyntaxSpan[],
): readonly DuplicateOccurrence[] {
  const result: DuplicateOccurrence[] = [];
  let blockedIndex = 0;
  for (const segment of document.segments) {
    let start = segment.contentRange.from;
    for (const end of [
      ...segment.hardBreaks,
      { from: segment.contentRange.to, to: segment.contentRange.to },
    ]) {
      let from = start,
        to = end.from;
      start = end.to;
      while (
        from < to &&
        whitespace.test(requireValue(document.sourceText[from], 'duplicate-segments'))
      )
        from++;
      while (
        to > from &&
        whitespace.test(requireValue(document.sourceText[to - 1], 'duplicate-segments'))
      )
        to--;
      if (from >= to) continue;
      while (
        blocked[blockedIndex] &&
        requireValue(blocked[blockedIndex], 'duplicate-segments').sourceRange.to <= from
      )
        blockedIndex++;
      if (
        blocked[blockedIndex] &&
        requireValue(blocked[blockedIndex], 'duplicate-segments').sourceRange.from < to
      )
        continue;
      const value = occurrence(document, { from, to }, emphasis);
      if (value) result.push(value);
    }
  }
  return result;
}
/** Temporary scope-local overlay retains inherited first identities without copying the universal map per child. */
interface FirstOccurrences {
  readonly own: Map<string, DuplicateOccurrence>;
  readonly parent: FirstOccurrences | null;
}
/** Follow only the actual bounded topology/scene parent chain for one key. */
function firstOccurrence(first: FirstOccurrences, key: string): DuplicateOccurrence | undefined {
  for (let current: FirstOccurrences | null = first; current; current = current.parent) {
    const found = current.own.get(key);
    if (found) return found;
  }
  return undefined;
}
/** Binary lookup admits only complete occurrence intervals within the actual scope, never clipped syntax. */
function occurrenceStart(values: readonly DuplicateOccurrence[], from: number): number {
  let low = 0,
    high = values.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (requireValue(values[middle], 'duplicate-segments').range.from < from) low = middle + 1;
    else high = middle;
  }
  return low;
}
/** Prepare one parser-owned occurrence stream; scope owners decide inheritance and siblings never share local firsts. */
export function prepareDuplicateSegments(
  document: PromptDocument,
  ownership: DuplicateOwnership,
  previous?: PreparedDuplicateSegments,
): PreparedDuplicateSegments {
  if (previous?.document === document && previous.ownership === ownership) return previous;
  const emphasis = new Map(document.emphasisSpans.map((span) => [span.outerRange.from, span]));
  const blocked = document.syntaxSpans.filter((span) => span.kind !== 'emphasis');
  const values = occurrences(document, emphasis, blocked);
  const findings: DuplicateFinding[] = [];
  /** Visit the prepared interval once and store only this scope's new firsts in its finite overlay. */
  const collect = (
    range: SourceRange,
    id: string,
    parent: FirstOccurrences | null,
  ): FirstOccurrences => {
    const first: FirstOccurrences = { own: new Map(), parent };
    for (let index = occurrenceStart(values, range.from); index < values.length; index++) {
      const value = requireValue(values[index], 'duplicate-segments');
      if (value.range.to > range.to) break;
      const previous = firstOccurrence(first, value.key);
      if (previous) findings.push({ scopeId: id, first: previous, duplicate: value });
      else first.own.set(value.key, value);
    }
    return first;
  };
  if (ownership.kind === 'value') {
    collect({ from: 0, to: document.sourceText.length }, 'value', null);
  } else if (ownership.kind === 'scenes') {
    const contexts = ownership.contexts;
    if (contexts.document.sourceText !== document.sourceText)
      throw new Error('foreign-duplicate-scene-source');
    const universal = collect(contexts.document.universalRange, 'universal', null);
    for (const context of contexts.scenes) {
      const scene = context.scene;
      if (!scene) throw new Error('missing-duplicate-scene');
      collect(scene.contentRange, `scene:${scene.marker.markerRange.from}`, universal);
    }
  } else {
    const topology = ownership.topology;
    if (
      topology.scopes.length !== document.regions.partitions.length ||
      topology.scopes.some(
        (scope, index) =>
          scope.range.from !==
            requireValue(document.regions.partitions[index], 'duplicate-segments').sourceRange
              .from ||
          scope.range.to !==
            requireValue(document.regions.partitions[index], 'duplicate-segments').sourceRange.to,
      )
    )
      throw new Error('foreign-duplicate-conditioning-source');
    /** Traverse actual scope relationships with overlays rather than cloning inherited maps. */
    const visit = (id: string, parent: FirstOccurrences | null): void => {
      const scope = topology.scope(id),
        first = collect(scope.range, scope.id, parent);
      for (const child of topology.children(id)) visit(child.id, first);
    };
    for (const root of topology.roots) visit(root.id, null);
  }
  return freezeStructure({ document, ownership, findings });
}

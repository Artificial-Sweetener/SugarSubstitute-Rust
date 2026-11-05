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
/** Derive line-placement units from exact native text and already prepared decoration boundaries. */
import type { ChangeSet } from '@codemirror/state';
import type { DecorationSet } from '@codemirror/view';
import type { SourceRange } from '../../core/src/index.js';
import { strip, whitespace } from '../../core/src/prompt/text.js';
import { unicodeAlphanumeric } from '../../core/src/prompt/alphanumeric.js';
import { EmphasisWidget, type SourceToken } from './projection-widgets.js';
import { WildcardWidget } from './wildcard-widget.js';

export interface PromptWrapGroup extends SourceRange {
  readonly kind: 'tag' | 'leading' | 'trailing' | 'word';
}

/** Keep short comma tags whole and retain edge-word attachment when a whole tag cannot fit. */
export function preparePromptWrapGroups(
  text: string,
  decorations: DecorationSet,
): readonly PromptWrapGroup[] {
  const tags: PromptWrapGroup[] = [],
    edges: PromptWrapGroup[] = [];
  let lineStart = 0;
  for (const line of text.split('\n')) {
    if (line.includes(',')) {
      let start = 0;
      for (const segment of line.split(',')) {
        const label = segment.replace(/^[ \t]+|[ \t]+$/gu, '');
        if (label && label.split(whitespace).filter(Boolean).length <= 3) {
          const from = lineStart + start + (segment.match(/^[ \t]*/u)?.[0].length ?? 0);
          const comma = start + segment.length < line.length;
          tags.push({
            from,
            to:
              lineStart +
              start +
              (comma ? segment.length + 1 : segment.replace(/[ \t]+$/u, '').length),
            kind: 'tag',
          });
        }
        start += segment.length + 1;
      }
    }
    lineStart += line.length + 1;
  }
  const tokens = new Map<SourceToken, { prefix?: SourceRange; suffix?: SourceRange }>();
  const concealed: SourceRange[] = [];
  decorations.between(0, text.length, (from, to, decoration) => {
    const widget: unknown = decoration.spec.widget;
    if (widget && to > from) concealed.push({ from, to });
    if (!(widget instanceof EmphasisWidget) && !(widget instanceof WildcardWidget)) return;
    const token = tokens.get(widget.token) ?? {};
    if (widget.part === 'prefix') token.prefix = { from, to };
    else token.suffix = { from, to };
    tokens.set(widget.token, token);
  });
  for (const { prefix, suffix } of tokens.values()) {
    if (!prefix || !suffix) continue;
    const content = text.slice(prefix.to, suffix.from);
    const firstStart = content.length - strip(content, 'left').length;
    let firstEnd = firstStart;
    while (firstEnd < content.length && !whitespace.test(content.charAt(firstEnd))) firstEnd++;
    let lastStart = strip(content, 'right').length;
    const lastEnd = lastStart;
    while (lastStart > 0 && !whitespace.test(content.charAt(lastStart - 1))) lastStart--;
    if (firstEnd > firstStart)
      edges.push({ from: prefix.from, to: prefix.to + firstEnd, kind: 'leading' });
    const separator = /^[ \t]*,/u.exec(text.slice(suffix.to));
    if (lastEnd > lastStart)
      edges.push({
        from: prefix.to + lastStart,
        to: suffix.to + (separator?.[0].length ?? 0),
        kind: 'trailing',
      });
  }
  const words: PromptWrapGroup[] = [];
  concealed.sort((left, right) => left.from - right.from);
  let wordStart = -1,
    offset = 0,
    concealedIndex = 0,
    concealedEnd = -1;
  /** Match the reference's alphanumeric/underscore/hyphen/apostrophe word integrity. */
  const finishWord = (): void => {
    while (concealedIndex < concealed.length) {
      const range = concealed[concealedIndex];
      if (!range || range.from >= offset) break;
      concealedEnd = Math.max(concealedEnd, range.to);
      concealedIndex++;
    }
    if (wordStart >= 0 && offset - wordStart > 1 && concealedEnd <= wordStart)
      words.push({ from: wordStart, to: offset, kind: 'word' });
    wordStart = -1;
  };
  for (const character of text) {
    if (unicodeAlphanumeric(character) || "_-'".includes(character)) {
      if (wordStart < 0) wordStart = offset;
    } else finishWord();
    offset += character.length;
  }
  finishWord();
  return [...tags, ...edges, ...words].filter(
    (group) => !text.slice(group.from, group.to).includes('\n'),
  );
}

/**
 * Carry an admitted group through an edit confined to it, until layout re-measures the group.
 * Returns null when an edit crosses the group's edges, adds a line break or empties the group.
 * Insertions exactly at an edge stay outside; a replacement that starts or ends at an edge is
 * part of the group, as when a weight step rewrites a whole emphasis.
 */
export function mapPromptWrapRange(range: SourceRange, changes: ChangeSet): SourceRange | null {
  let startSide: -1 | 1 = 1,
    endSide: -1 | 1 = -1,
    confined = true;
  changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
    const touches =
      fromA === toA ? fromA > range.from && fromA < range.to : fromA < range.to && toA > range.from;
    if (!touches) return;
    if (fromA < range.from || toA > range.to || inserted.lines > 1) confined = false;
    if (fromA === range.from) startSide = -1;
    if (toA === range.to) endSide = 1;
  });
  if (!confined) return null;
  const from = changes.mapPos(range.from, startSide),
    to = changes.mapPos(range.to, endSide);
  return to > from ? { from, to } : null;
}

/** Prefer a fitting whole tag; otherwise independently admit non-overlapping decoration attachments. */
export function fittingPromptWrapGroups(
  groups: readonly PromptWrapGroup[],
  widths: ReadonlyMap<PromptWrapGroup, number>,
  contentWidth: number,
): readonly PromptWrapGroup[] {
  const selected: PromptWrapGroup[] = [];
  for (const group of groups) {
    const width = widths.get(group);
    if (width === undefined || width > contentWidth + 0.01) continue;
    let from = 0,
      to = selected.length;
    while (from < to) {
      const middle = (from + to) >>> 1;
      if ((selected[middle]?.from ?? Infinity) < group.from) from = middle + 1;
      else to = middle;
    }
    if (
      (selected[from - 1]?.to ?? -1) > group.from ||
      (selected[from]?.from ?? Infinity) < group.to
    )
      continue;
    selected.splice(from, 0, group);
  }
  return selected;
}

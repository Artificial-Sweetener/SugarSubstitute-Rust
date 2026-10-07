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
/** Match source values and semantic labels in authoritative masonry reading order. */
import type { SearchQuery } from './SearchQuery.js';

export interface SearchField {
  readonly id: string;
  readonly key: string;
  readonly label: string;
  readonly text: string | null;
}

export interface SearchNode {
  readonly id: string;
  readonly name: string;
  readonly type: string;
  readonly title: string;
  readonly fields: readonly SearchField[];
}

export interface SearchMatch {
  readonly node: string;
  readonly field: string;
  readonly start: number;
  readonly length: number;
}

export interface SearchResult {
  readonly query: SearchQuery;
  readonly nodes: ReadonlySet<string>;
  readonly fields: ReadonlySet<string>;
  readonly matches: readonly SearchMatch[];
}

/** Find non-overlapping literal matches while retaining UTF-16 source offsets for native selection. */
export function findSearchRanges(
  source: string,
  needle: string,
): Array<{ start: number; length: number }> {
  if (!needle) return [];
  const folded = source.toLowerCase();
  const positions: Array<{ start: number; end: number }> = [];
  if (folded.length !== source.length) {
    let offset = 0;
    for (const character of source) {
      positions.push(
        ...Array.from({ length: character.toLowerCase().length }, () => ({
          start: offset,
          end: offset + character.length,
        })),
      );
      offset += character.length;
    }
  }
  const ranges: Array<{ start: number; length: number }> = [];
  for (
    let index = folded.indexOf(needle);
    index >= 0;
    index = folded.indexOf(needle, index + needle.length)
  ) {
    const start = positions[index]?.start ?? index;
    const end = positions[index + needle.length - 1]?.end ?? index + needle.length;
    if (ranges.at(-1)?.start !== start) ranges.push({ start, length: end - start });
  }
  return ranges;
}

/** Apply Field's all-token matching, Node's substring matching, and scoped quoted value matching. */
export function matchSearch(nodes: readonly SearchNode[], query: SearchQuery): SearchResult {
  const matchingNodes = new Set<string>();
  const matchingFields = new Set<string>();
  const matches: SearchMatch[] = [];
  for (const node of nodes) {
    if (query.mode === 'field') {
      for (const field of node.fields)
        if (
          query.tokens.every((token) => `${field.key} ${field.label}`.toLowerCase().includes(token))
        ) {
          matchingFields.add(field.id);
          matchingNodes.add(node.id);
        }
    } else if (query.mode === 'node') {
      const labels = [
        node.name,
        node.type,
        node.title,
        ...node.fields.flatMap((field) => [field.key, field.label]),
      ];
      if (!query.node || labels.some((label) => label.toLowerCase().includes(query.node)))
        matchingNodes.add(node.id);
    }
    if (!query.text || (query.mode === 'node' && query.node && !matchingNodes.has(node.id)))
      continue;
    for (const field of node.fields)
      if (field.text !== null)
        for (const range of findSearchRanges(field.text, query.text))
          matches.push({ node: node.id, field: field.id, ...range });
  }
  return { query, nodes: matchingNodes, fields: matchingFields, matches };
}

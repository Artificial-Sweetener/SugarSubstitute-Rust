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
/** Ordinal regional identities consume canonical parsed documents independently of host topology. */
import { requireValue } from '../required.js';
import type { PromptDocument } from './contracts.js';
import type { SourceReplacement } from '../normalization/contracts.js';
import { regionNameReplacement } from './region-naming.js';
import { strip } from './text.js';
/** A zero-based separator ordinal shares one literal authored name across related values. */
export interface RegionName {
  readonly ordinal: number;
  readonly name: string;
}
/** All replacements use original exact-source UTF-16 bounds; resulting source preserves every unedited byte. */
export interface RegionNameApplication {
  readonly source: string;
  readonly replacements: readonly SourceReplacement[];
}
/** Compare trimmed authored identities; deleted separators do not manufacture a name change. */
export function changedRegionNames(
  previous: PromptDocument,
  current: PromptDocument,
): readonly RegionName[] {
  return changedRegionIdentities(regionNameIdentities(previous), regionNameIdentities(current));
}
/** Prepared ordinal names include unnamed separators, allowing source-free host transition observation. */
export function regionNameIdentities(document: PromptDocument): readonly RegionName[] {
  return Object.freeze(
    document.regions.separators.map((separator, ordinal) =>
      Object.freeze({ ordinal, name: strip(separator.name ?? '') }),
    ),
  );
}
/** Compare canonical prepared names; deleted separators and new unnamed separators do not manufacture edits. */
export function changedRegionIdentities(
  previous: readonly RegionName[],
  current: readonly RegionName[],
): readonly RegionName[] {
  const before = new Map(previous.map((value) => [value.ordinal, value.name]));
  return Object.freeze(
    current.filter((value) =>
      before.has(value.ordinal) ? before.get(value.ordinal) !== value.name : value.name !== '',
    ),
  );
}
/** Source order chooses the first non-empty authored identity for each available ordinal. */
export function firstRegionNames(documents: readonly PromptDocument[]): readonly RegionName[] {
  const names = new Map<number, string>();
  for (const document of documents)
    for (const [ordinal, separator] of document.regions.separators.entries()) {
      const name = strip(separator.name ?? '');
      if (name && !names.has(ordinal)) names.set(ordinal, name);
    }
  return Object.freeze(Array.from(names, ([ordinal, name]) => Object.freeze({ ordinal, name })));
}
/** Apply only available ordinal identities in reverse source order; invalid or duplicate identities reject explicitly. */
export function applyRegionNames(
  document: PromptDocument,
  names: readonly RegionName[],
): RegionNameApplication {
  const replacements: SourceReplacement[] = [],
    seen = new Set<number>();
  for (const { ordinal, name } of names) {
    if (!Number.isSafeInteger(ordinal) || ordinal < 0 || seen.has(ordinal))
      throw new RangeError('Invalid region identity');
    seen.add(ordinal);
    const separator = document.regions.separators[ordinal];
    if (separator && strip(separator.name ?? '') !== name)
      replacements.push(regionNameReplacement(document, ordinal, name));
  }
  replacements.sort((left, right) => left.from - right.from);
  let source = document.sourceText;
  for (let index = replacements.length - 1; index >= 0; index--) {
    const edit = requireValue(replacements[index], 'region-name-identity');
    source = source.slice(0, edit.from) + edit.text + source.slice(edit.to);
  }
  return Object.freeze({ source, replacements: Object.freeze(replacements) });
}

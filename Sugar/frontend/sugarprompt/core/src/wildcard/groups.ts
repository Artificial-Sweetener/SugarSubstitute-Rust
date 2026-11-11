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
/** Wildcard group display is derived from source identity, independently of catalog availability or generated choices. */
import { requireValue } from '../required.js';
import type { WildcardSpan } from '../prompt/contracts.js';
import type { SourceRange } from '../offsets.js';
/** Editable body excludes a CSV activator and authored group suffix; bounds use exact-source UTF-16. */
export function wildcardBodyRange(span: WildcardSpan): SourceRange {
  return Object.freeze({
    from: span.contentRange.from + (span.form === 'csv' ? 4 : 0),
    to: span.contentRange.to - (span.tag === null ? 0 : span.tag.length + 1),
  });
}
/** A prepared group retains its exact-source UTF-16 span; implicit display never changes authored source. */
export interface WildcardGroup {
  readonly span: WildcardSpan;
  readonly sourceKey: string;
  readonly occurrenceCount: number;
  readonly displayTag: string | null;
  readonly explicit: boolean;
  readonly numeric: boolean;
}
/** Match the reference renderer's strict ASCII positive-integer control policy, without numeric precision loss. */
export function isNumericWildcardTag(tag: string | null): boolean {
  return tag !== null && /^[1-9][0-9]*$/u.test(tag);
}
/** Count references by source form and identifier; CSV columns and explicit tags share their source's display-only default. */
export function prepareWildcardGroups(spans: readonly WildcardSpan[]): readonly WildcardGroup[] {
  const counts = new Map<string, number>();
  const keys = spans.map((span) => `${span.form}:${span.identifier}`);
  for (const key of keys) counts.set(key, (counts.get(key) ?? 0) + 1);
  return Object.freeze(
    spans.map((span, index) => {
      const sourceKey = requireValue(keys[index], 'groups'),
        occurrenceCount = requireValue(counts.get(sourceKey), 'groups'),
        displayTag = span.tag ?? (occurrenceCount > 1 ? '1' : null);
      return Object.freeze({
        span,
        sourceKey,
        occurrenceCount,
        displayTag,
        explicit: span.tag !== null,
        numeric: isNumericWildcardTag(displayTag),
      });
    }),
  );
}

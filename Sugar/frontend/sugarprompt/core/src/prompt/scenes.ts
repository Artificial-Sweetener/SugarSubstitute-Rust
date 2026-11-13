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
/** Scene heading recognition and universal/local source ownership. */
import { requireValue } from '../required.js';
import type { SceneBlock, SceneDocument, SceneMarker } from './contracts.js';
import { caseFold, freezeStructure, strip } from './text.js';
import { physicalLines } from './physical-lines.js';
/** Normalize scene identity by reference whitespace collapsing and full Unicode case folding. */
export function normalizeSceneTitle(title: string): string {
  return caseFold(
    strip(title).replace(
      // eslint-disable-next-line no-control-regex -- Preserve exact Python-compatible control-code and whitespace boundaries.
      /[\t-\r\x1c-\x20\x85\xa0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]+/gu,
      ' ',
    ),
  );
}
/** Parse headings independently of prompt brackets/quotes, using exact-source UTF-16 ranges; disabled documents retain ordinary source ownership. */
export function parseScenes(source: string, enabled = true): SceneDocument {
  const candidates: {
    marker: SceneMarker;
    contentFrom: number;
  }[] = [];
  const seen = new Set<string>();
  for (const record of enabled ? physicalLines(source) : []) {
    const line = record.text,
      from = record.from,
      lineEnd = record.to;
    const contentEnd = lineEnd - (line.endsWith('\r\n') ? 2 : /[\r\n]$/u.test(line) ? 1 : 0);
    const leading = requireValue(/^[ \t]*/u.exec(line), 'scenes')[0].length;
    const markerFrom = from + leading,
      markerTo = markerFrom + 2;
    if (markerTo > contentEnd || source.slice(markerFrom, markerTo) !== '**') continue;
    const raw = source.slice(markerTo, contentEnd),
      title = strip(raw);
    if (!title) continue;
    const titleFrom = markerTo + raw.length - strip(raw, 'left').length;
    const normalizedKey = normalizeSceneTitle(title);
    const marker: SceneMarker = {
      title,
      normalizedKey,
      markerRange: { from: markerFrom, to: markerTo },
      titleRange: { from: titleFrom, to: titleFrom + title.length },
      lineRange: { from, to: contentEnd },
      duplicate: seen.has(normalizedKey),
    };
    candidates.push({ marker, contentFrom: lineEnd });
    seen.add(normalizedKey);
  }
  const universalRange = {
    from: 0,
    to: candidates[0]?.marker.lineRange.from ?? source.length,
  };
  const scenes: SceneBlock[] = candidates.map((candidate, index) => {
    const contentRange = {
      from: candidate.contentFrom,
      to: candidates[index + 1]?.marker.lineRange.from ?? source.length,
    };
    return {
      marker: candidate.marker,
      contentRange,
      text: source.slice(contentRange.from, contentRange.to),
    };
  });
  return freezeStructure({
    sourceText: source,
    universalRange,
    universalText: source.slice(0, universalRange.to),
    scenes,
  });
}

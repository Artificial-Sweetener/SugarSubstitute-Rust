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
/** Wildcard grammar and validated wrapper profiles; resolution belongs to catalog services. */
import { requireValue } from '../required.js';
import type { WildcardSpan, WildcardSyntax } from './contracts.js';
import { strip, whitespace } from './text.js';
import { unicodeAlphanumeric } from './alphanumeric.js';
/** One active wrapper pair, in matching priority order. */
export interface WildcardDelimiter {
  readonly prefix: string;
  readonly suffix: string;
}
/** Validate custom delimiters and return the reference's active wrapper ordering. */
export function wildcardDelimiters(profile: WildcardSyntax): readonly WildcardDelimiter[] {
  if (profile.style === 'curly') return [{ prefix: '{', suffix: '}' }];
  if (profile.style === 'double-underscore')
    return [
      { prefix: '__', suffix: '__' },
      { prefix: '{', suffix: '}' },
    ];
  if (profile.style !== 'custom' || typeof profile.alsoRecognizeCurly !== 'boolean')
    throw new TypeError('Invalid wildcard profile');
  const { prefix, suffix } = profile;
  for (const part of [prefix, suffix]) {
    if (
      typeof part !== 'string' ||
      !part ||
      [...Array.from(part)].length > 8 ||
      whitespace.test(part) ||
      // eslint-disable-next-line no-control-regex -- Preserve exact Python-compatible control-code and whitespace boundaries.
      /[\x00-\x1f'"/\\{}]/u.test(part) ||
      unicodeAlphanumeric(part)
    )
      throw new RangeError('Invalid wildcard delimiter');
  }
  if ((prefix === '<' && suffix === '>') || (prefix === '(' && suffix === ')'))
    throw new RangeError('Conflicting wildcard delimiters');
  return [
    { prefix, suffix },
    ...(profile.alsoRecognizeCurly ? [{ prefix: '{', suffix: '}' }] : []),
  ];
}
/** Normalize a nonempty slash path; malformed paths remain literal source. */
function identifier(text: string): string | null {
  if (!text || strip(text) !== text || /[\\:]/u.test(text)) return null;
  return text
    .split('/')
    .some((part) => !part || strip(part) !== part || part === '.' || part === '..')
    ? null
    : text;
}
/** Parse one wrapper's content, retaining the donor's permissive optional tag semantics. */
export function wildcardSpan(
  source: string,
  from: number,
  to: number,
  contentFrom: number,
  contentTo: number,
  curly: boolean,
): WildcardSpan | null {
  const content = source.slice(contentFrom, contentTo);
  if (curly && (!content || strip(content) !== content || /[{}]/u.test(content))) return null;
  const pipe = content.indexOf('|');
  let base = content,
    tag: string | null = null;
  if (pipe >= 0) {
    const candidate = content.slice(pipe + 1);
    if (candidate && strip(candidate) === candidate) {
      base = content.slice(0, pipe);
      tag = candidate;
    }
  }
  let form: 'simple' | 'csv' = 'simple',
    csvColumn: string | null = null;
  if (base.startsWith('csv:')) {
    const fields = base.slice(4).split(':');
    if (fields.length !== 2 || !fields[1] || strip(fields[1]) !== fields[1]) return null;
    base = requireValue(fields[0], 'wildcards');
    csvColumn = fields[1];
    form = 'csv';
  }
  const name = identifier(base);
  return name === null
    ? null
    : {
        outerRange: { from, to },
        contentRange: { from: contentFrom, to: contentTo },
        form,
        identifier: name,
        csvColumn,
        tag,
        depth: 0,
      };
}
/** Find non-curly wrappers independently, including those inside quoted or bracketed text. */
export function alternateWildcards(
  source: string,
  delimiters: readonly WildcardDelimiter[],
): WildcardSpan[] {
  const spans: WildcardSpan[] = [];
  for (const { prefix, suffix } of delimiters) {
    if (prefix === '{' && suffix === '}') continue;
    let index = 0;
    while (index < source.length) {
      const from = source.indexOf(prefix, index);
      if (from < 0) break;
      const contentFrom = from + prefix.length;
      const contentTo = source.indexOf(suffix, contentFrom);
      if (contentTo < 0) break;
      index = contentTo + suffix.length;
      const span = wildcardSpan(source, from, index, contentFrom, contentTo, false);
      if (span) spans.push(span);
    }
  }
  return spans.sort(
    (a, b) => a.outerRange.from - b.outerRange.from || a.outerRange.to - b.outerRange.to,
  );
}

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
/** Keep exact tuple components distinct, including provider and local backend/path identity. */
import { requireValue } from '../../../core/src/required.js';
import { caseFold } from '../../../core/src/prompt/text.js';
import { compareScalarText } from '../text/scalar.js';
import { captureArray, captureRecord, captureText } from './capture.js';
import type {
  SavedSegment,
  SavedSegmentAdapter,
  SavedSegmentCatalog,
  SavedSegmentContext,
  SavedSegmentScope,
} from './contracts.js';
function sameTarget(left: SavedSegmentScope, right: SavedSegmentScope): boolean {
  return left.kind === right.kind && left.provider === right.provider && left.key === right.key;
}
/** Global precedes families and the exact version/local target in save choices. */
function rank(scope: SavedSegmentScope): number {
  return scope.kind === 'global' ? 0 : scope.kind === 'family' ? 1 : 2;
}
/** Capture persisted associations; provider-model is retained but is never an offered save choice. */
export function captureSavedSegmentScope(value: unknown): SavedSegmentScope {
  const item = captureRecord(value, ['kind', 'provider', 'key', 'label']);
  const kind = item.kind;
  if (
    kind !== 'global' &&
    kind !== 'family' &&
    kind !== 'provider-model' &&
    kind !== 'model-version' &&
    kind !== 'local-model'
  )
    throw new TypeError('invalid-saved-segment-scope');
  const key = captureText(item.key, 4096),
    label = captureText(item.label, 2048);
  if (kind === 'global') {
    if (item.provider !== null || key !== 'global')
      throw new TypeError('invalid-saved-segment-scope');
    return Object.freeze({ kind, provider: null, key, label });
  }
  const provider = captureText(item.provider, 4096);
  if (kind === 'local-model' && provider !== 'local')
    throw new TypeError('invalid-saved-segment-scope');
  return Object.freeze({ kind, provider, key, label });
}
/** Own bounded unique host choices; library admits Global only and workflow has at most one exact target. */
export function captureSavedSegmentContext(value: unknown): SavedSegmentContext {
  const item = captureRecord(value, ['mode', 'scopes']),
    mode = item.mode;
  if (mode !== 'library' && mode !== 'workflow')
    throw new TypeError('invalid-saved-segment-context');
  const scopes = captureArray(item.scopes, 32).map(captureSavedSegmentScope);
  if (
    !scopes.length ||
    scopes.filter((scope) => scope.kind === 'global').length !== 1 ||
    scopes.filter((scope) => rank(scope) === 2).length > 1
  )
    throw new TypeError('invalid-saved-segment-context');
  for (let index = 0; index < scopes.length; index++) {
    const scope = requireValue(scopes[index], 'preparation');
    if (
      scope.kind === 'provider-model' ||
      (mode === 'library' && scope.kind !== 'global') ||
      scopes.slice(0, index).some((prior) => sameTarget(prior, scope))
    )
      throw new TypeError('invalid-saved-segment-context');
  }
  return Object.freeze({
    mode,
    scopes: Object.freeze(scopes.sort((a, b) => rank(a) - rank(b))),
  });
}
/** Capture methods once while preserving their original host receiver; no later host property reads occur. */
export function captureSavedSegmentAdapter(value: unknown): SavedSegmentAdapter {
  const item = captureRecord(value, ['read', 'save']);
  if (typeof item.read !== 'function' || typeof item.save !== 'function')
    throw new TypeError('invalid-saved-segment-adapter');
  const read = item.read as SavedSegmentAdapter['read'],
    save = item.save as SavedSegmentAdapter['save'];
  return Object.freeze({
    read(
      context: SavedSegmentContext,
      signal: AbortSignal,
    ): ReturnType<SavedSegmentAdapter['read']> {
      return Reflect.apply(read, value, [context, signal]) as ReturnType<
        SavedSegmentAdapter['read']
      >;
    },
    save(
      request: Parameters<SavedSegmentAdapter['save']>[0],
      signal: AbortSignal,
    ): ReturnType<SavedSegmentAdapter['save']> {
      return Reflect.apply(save, value, [request, signal]) as ReturnType<
        SavedSegmentAdapter['save']
      >;
    },
  });
}
/** Validate injected methods without invoking persistence. */
export function validateSavedSegmentAdapter(value: unknown): asserts value is SavedSegmentAdapter {
  captureSavedSegmentAdapter(value);
}
/** Own and sort finite storage data; each ID enters its first matching group, and empty sections are omitted. */
export function prepareSavedSegmentCatalog(
  context: SavedSegmentContext,
  value: unknown,
): SavedSegmentCatalog {
  const segments: SavedSegment[] = [];
  // A catalog response has an eight-million UTF-16-unit aggregate payload budget, including repeated rows.
  let remaining = 8000000;
  for (const raw of captureArray(value, 4096)) {
    const item = captureRecord(raw, ['id', 'label', 'text', 'associations']);
    const id = captureText(item.id, 4096),
      label = captureText(item.label, 2048),
      text = captureText(item.text, remaining);
    remaining -= text.length;
    const associations = captureArray(item.associations, 32).map(captureSavedSegmentScope);
    if (!associations.length) throw new TypeError('invalid-saved-segment-catalog');
    segments.push(
      Object.freeze({
        id,
        label,
        text,
        associations: Object.freeze(associations),
      }),
    );
  }
  const sorted = segments.map((segment) => ({
    segment,
    label: caseFold(segment.label),
    text: caseFold(segment.text),
  }));
  sorted.sort(
    (a, b) =>
      compareScalarText(a.label, b.label) ||
      compareScalarText(a.text, b.text) ||
      compareScalarText(a.segment.id, b.segment.id),
  );
  const shown = new Set<string>(),
    groups: SavedSegmentCatalog['groups'][number][] = [];
  const scopes = [...context.scopes].sort((a, b) => rank(b) - rank(a));
  for (const scope of scopes) {
    const matching: SavedSegment[] = [];
    for (const { segment } of sorted) {
      if (
        !shown.has(segment.id) &&
        segment.associations.some((association) => sameTarget(association, scope))
      ) {
        matching.push(segment);
        shown.add(segment.id);
      }
    }
    if (matching.length) groups.push(Object.freeze({ scope, segments: Object.freeze(matching) }));
  }
  return Object.freeze({ context, groups: Object.freeze(groups) });
}

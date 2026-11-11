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
/** Capture and validate generation candidates once; hosts own file acquisition and decoded value preparation. */
import { unicodeLowerCase, type WildcardSyntax } from '../../../core/src/index.js';
import { wildcardDelimiters } from '../../../core/src/prompt/wildcards.js';
import { strip } from '../../../core/src/prompt/text.js';
import { wildcardCatalogKey } from './metadata.js';
import { isWildcardUnicode } from './generation-digest.js';
import type { WildcardCandidateSource } from './generation-contracts.js';

/** One prepared source owns immutable candidate values, separate from authoring/catalog display metadata. */
export type CapturedGenerationSource =
  | { readonly status: 'unavailable' }
  | {
      readonly status: 'ready';
      readonly sourceId: string;
      readonly values: readonly string[] | readonly ReadonlyMap<string, string>[];
    };
/** Captured generation state is private to one pass and never shared with an editor document. */
export interface CapturedGenerationInput {
  readonly revision: string;
  readonly seed: string | null;
  readonly entropySeed: string | null;
  readonly enabled: boolean;
  readonly syntax: WildcardSyntax;
  readonly sources: Map<string, CapturedGenerationSource>;
}
/** Closed input records reject unknown fields and prototype-bearing instances at the external boundary. */
function record(value: unknown): value is Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return false;
  const prototype: unknown = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}
/** Canonical signed decimal seeds retain exact integer spelling through JSON host boundaries. */
export function validGenerationSeed(value: unknown): value is string {
  return typeof value === 'string' && value.length <= 80 && /^(?:0|-?[1-9][0-9]*)$/u.test(value);
}
/** Capture a bounded closed host snapshot; expected malformed input yields null without hiding invariants. */
export function captureGenerationInput(value: unknown): CapturedGenerationInput | null {
  if (
    !record(value) ||
    Object.keys(value).some(
      (key) => !['revision', 'sources', 'seed', 'entropySeed', 'syntax', 'enabled'].includes(key),
    )
  )
    return null;
  const revision = value.revision,
    externalSources = value.sources,
    seed = value.seed,
    entropySeed = value.entropySeed,
    enabled = value.enabled,
    externalSyntax = value.syntax;
  if (
    typeof revision !== 'string' ||
    !revision ||
    revision.length > 256 ||
    !Array.isArray(externalSources) ||
    (seed !== null && !validGenerationSeed(seed)) ||
    (entropySeed !== undefined && !validGenerationSeed(entropySeed)) ||
    (seed === null && entropySeed === undefined) ||
    (enabled !== undefined && typeof enabled !== 'boolean')
  )
    return null;
  const sourceCount = externalSources.length;
  if (!Number.isSafeInteger(sourceCount) || sourceCount < 0 || sourceCount > 10000) return null;
  let syntax: WildcardSyntax = { style: 'curly' };
  if (externalSyntax !== undefined) {
    if (!record(externalSyntax)) return null;
    const style = externalSyntax.style,
      keys = Object.keys(externalSyntax);
    if ((style === 'curly' || style === 'double-underscore') && keys.length === 1)
      syntax = { style };
    else if (
      style === 'custom' &&
      keys.sort().join() === 'alsoRecognizeCurly,prefix,style,suffix'
    ) {
      const prefix = externalSyntax.prefix,
        suffix = externalSyntax.suffix,
        alsoRecognizeCurly = externalSyntax.alsoRecognizeCurly;
      if (
        typeof prefix !== 'string' ||
        typeof suffix !== 'string' ||
        typeof alsoRecognizeCurly !== 'boolean'
      )
        return null;
      syntax = { style, prefix, suffix, alsoRecognizeCurly };
    } else return null;
    try {
      wildcardDelimiters(syntax);
    } catch (error) {
      if (error instanceof TypeError || error instanceof RangeError) return null;
      throw error;
    }
  }
  const sources = new Map<string, CapturedGenerationSource>();
  let cells = 0,
    units = 0,
    items = 0;
  /** Capture exact decoded values without silently filtering, trimming or truncating candidates. */
  function text(value: unknown): value is string {
    return (
      typeof value === 'string' &&
      value.length <= 1000000 &&
      ++cells <= 250000 &&
      (units += value.length) <= 8000000
    );
  }
  for (let sourceIndex = 0; sourceIndex < sourceCount; sourceIndex++) {
    const source: unknown = externalSources[sourceIndex];
    if (!record(source)) return null;
    const form = source.form,
      identifier = source.identifier,
      status = source.status;
    if (
      (form !== 'simple' && form !== 'csv') ||
      typeof identifier !== 'string' ||
      identifier.length > 1024 ||
      (status !== 'ready' && status !== 'unavailable')
    )
      return null;
    const expected =
      status === 'unavailable'
        ? ['status', 'form', 'identifier']
        : ['status', 'form', 'identifier', 'sourceId', form === 'simple' ? 'lines' : 'rows'];
    if (Object.keys(source).sort().join() !== expected.sort().join()) return null;
    let key: string;
    try {
      key = JSON.stringify([form, wildcardCatalogKey(identifier)]);
    } catch (error) {
      if (error instanceof TypeError) return null;
      throw error;
    }
    if (sources.has(key)) return null;
    if (status === 'unavailable') {
      sources.set(key, Object.freeze({ status: 'unavailable' }));
      continue;
    }
    const sourceId = source.sourceId,
      values = form === 'simple' ? source.lines : source.rows;
    if (
      typeof sourceId !== 'string' ||
      !sourceId ||
      sourceId.length > 1024 ||
      !isWildcardUnicode(sourceId) ||
      !Array.isArray(values)
    )
      return null;
    const valueCount = values.length;
    if (!Number.isSafeInteger(valueCount) || valueCount < 0 || (items += valueCount) > 250000)
      return null;
    if (form === 'simple') {
      const lines: string[] = [];
      for (let index = 0; index < valueCount; index++) {
        const line: unknown = values[index];
        if (!text(line)) return null;
        lines.push(line);
      }
      sources.set(
        key,
        Object.freeze({
          status: 'ready',
          sourceId,
          values: Object.freeze(lines),
        }),
      );
    } else {
      const rows: ReadonlyMap<string, string>[] = [];
      for (let index = 0; index < valueCount; index++) {
        const row: unknown = values[index];
        if (!record(row)) return null;
        const captured = new Map<string, string>();
        for (const column of Object.keys(row)) {
          const cell = row[column];
          if (column.length > 1024 || !text(column) || !text(cell)) return null;
          captured.set(unicodeLowerCase(strip(column)), cell);
        }
        rows.push(captured);
      }
      sources.set(
        key,
        Object.freeze({
          status: 'ready',
          sourceId,
          values: Object.freeze(rows),
        }),
      );
    }
  }
  return {
    revision,
    seed,
    entropySeed: entropySeed ?? null,
    enabled: enabled !== false,
    syntax: Object.freeze(syntax),
    sources,
  };
}
/** Look up actual candidates with the existing canonical catalog path/lowercase authority. */
export function generationSourceKey(
  form: WildcardCandidateSource['form'],
  identifier: string,
): string {
  return JSON.stringify([form, wildcardCatalogKey(identifier)]);
}

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
/** Coordinate adaptation only; structure owns preparation/publication and callers supply current input permission. */
import { requireValue } from '../../core/src/required.js';
import type { EditorState, EditorSelection, Text } from '@codemirror/state';
import {
  admitDocumentPromptScope,
  type DocumentPromptScopeOutcome,
  type PreparedDocumentContainer,
  type PreparedDocumentPromptScope,
  type PreparedDocumentPromptScopes,
  type PreparedValueMapping,
  type PreparedDocumentSemantics,
  type SourceRange,
  type WildcardSyntax,
} from '../../core/src/index.js';
import { sourcePositionsToDocument, type LineEnding } from '../../core/src/line-endings.js';
import { wildcardDelimiters } from '../../core/src/prompt/wildcards.js';
import { documentKind } from './document-kind.js';
import { lineEndings, readExactSource } from './exact-source.js';
import { editorFreshness, freshnessRejection, type EditorFreshness } from './freshness.js';
/** Safe decoded UTF-16 endpoints map to logical CM positions; inverse indices are relative to logicalRange.from. */
export interface NativeValueMapping {
  readonly mapping: PreparedValueMapping;
  readonly logicalRange: SourceRange;
  readonly decodedToLogical: readonly (number | null)[];
  readonly logicalToDecoded: readonly (number | null)[];
}
/** Full physical storage hit entry; protected containers retain no grammar scope. */
export interface NativeDocumentValue {
  readonly container: PreparedDocumentContainer;
  readonly logicalStorage: SourceRange;
  readonly fullMapping: NativeValueMapping | null;
  readonly scope: PreparedDocumentPromptScope | null;
  readonly semanticMapping: NativeValueMapping | null;
}
/** Immutable derived coordinates, never an editable buffer or a memoization/lifetime owner. */
export interface PreparedNativeDocumentValues {
  readonly scopes: PreparedDocumentPromptScopes;
  readonly syntax: WildcardSyntax;
  readonly sourceIdentity: symbol;
  readonly document: Text;
  readonly endings: readonly LineEnding[];
  readonly entries: readonly NativeDocumentValue[];
  readonly byId: Readonly<Record<string, NativeDocumentValue>>;
}
/** The next structure family supplies its actual current slot; the existing caller owns feature permission and composition. */
export interface NativeDocumentValueContext {
  readonly values: PreparedNativeDocumentValues;
  readonly semantics: PreparedDocumentSemantics;
  readonly enabled: boolean;
  readonly composing: boolean;
}
/** Issued command authority includes actual member, decoded range, epochs and directional native selection identity. */
export interface NativeDocumentValueTarget {
  readonly values: PreparedNativeDocumentValues;
  readonly entry: NativeDocumentValue;
  readonly scope: PreparedDocumentPromptScope;
  readonly decodedRange: SourceRange;
  readonly freshness: EditorFreshness;
  readonly selection: EditorSelection;
}
/** Actual frozen targets carry finite captured authority; reconstructed decoded ranges are never issued commands. */
const issuedTargets = new WeakSet<NativeDocumentValueTarget>();
/** Stable refusals expose neither authored text nor invariant causes. */
export type NativeDocumentValueRejection =
  | Extract<
      DocumentPromptScopeOutcome,
      {
        status: 'rejected';
      }
    >['reason']
  | 'stale-source'
  | 'stale-context'
  | 'read-only'
  | 'composing'
  | 'disabled';
/** Admission returns the actual target; no fallback grammar is synthesized. */
export type NativeDocumentValueOutcome =
  | {
      readonly status: 'ready';
      readonly target: NativeDocumentValueTarget;
    }
  | {
      readonly status: 'rejected';
      readonly reason: NativeDocumentValueRejection;
    };
/** Reuse single-width coordinates or adapt every CRLF endpoint in one batch, outside hot lookup and publication. */
export function prepareNativeDocumentValues(
  state: EditorState,
  scopes: PreparedDocumentPromptScopes,
  syntax: WildcardSyntax,
  previous?: PreparedNativeDocumentValues,
): PreparedNativeDocumentValues {
  const identity = state.field(editorFreshness).source;
  const endings = state.field(lineEndings);
  if (
    scopes.values.kind !== state.facet(documentKind) ||
    scopes.values.source !== readExactSource(state)
  )
    throw new Error('native-document-values-source-mismatch');
  if (scopes.profileKey !== JSON.stringify(wildcardDelimiters(syntax)))
    throw new Error('native-document-values-profile-mismatch');
  if (
    previous?.scopes === scopes &&
    previous.sourceIdentity === identity &&
    previous.document === state.doc &&
    previous.endings === endings
  )
    return previous;
  const positions: number[] = [];
  const identityCoordinates = !endings.includes('\r\n');
  const slots = new Map<PreparedValueMapping, readonly (number | null)[]>();
  /** Queue only canonical safe endpoints; null scalar/CRLF/escape gaps remain null. */
  const queue = (mapping: PreparedValueMapping): void => {
    if (slots.has(mapping)) return;
    slots.set(
      mapping,
      identityCoordinates
        ? mapping.decodedToSource
        : mapping.decodedToSource.map((position) => {
            if (position === null) return null;
            const slot = positions.length;
            positions.push(position);
            return slot;
          }),
    );
  };
  const storageSlots = scopes.values.containers.map((container) => {
    const from = positions.length;
    positions.push(container.storageRange.from, container.storageRange.to);
    if (container.mapping) queue(container.mapping);
    const scope = scopes.byId[container.id];
    if (scope) queue(scope.mapping);
    return from;
  });
  const logicalPositions = sourcePositionsToDocument(state.doc.toString(), endings, positions);
  const mappings = new Map<PreparedValueMapping, NativeValueMapping>();
  for (const [mapping, indices] of slots) {
    if (identityCoordinates) {
      mappings.set(
        mapping,
        Object.freeze({
          mapping,
          logicalRange: mapping.sourceRange,
          decodedToLogical: mapping.decodedToSource,
          logicalToDecoded: mapping.sourceToDecoded,
        }),
      );
      continue;
    }
    const decodedToLogical = indices.map((index) =>
      index === null ? null : requireValue(logicalPositions[index], 'document-values'),
    );
    const from = decodedToLogical[0],
      to = decodedToLogical.at(-1);
    if (from == null || to == null) throw new Error('native-value-map-endpoints');
    const logicalToDecoded: (number | null)[] = Array.from({ length: to - from + 1 }, () => null);
    decodedToLogical.forEach((position, decoded) => {
      if (position !== null) logicalToDecoded[position - from] = decoded;
    });
    mappings.set(
      mapping,
      Object.freeze({
        mapping,
        logicalRange: Object.freeze({ from, to }),
        decodedToLogical: Object.freeze(decodedToLogical),
        logicalToDecoded: Object.freeze(logicalToDecoded),
      }),
    );
  }
  const byId: Record<string, NativeDocumentValue> = Object.create(null) as Record<
    string,
    NativeDocumentValue
  >;
  const entries = scopes.values.containers.map((container, index) => {
    const slot = requireValue(storageSlots[index], 'document-values');
    const scope = scopes.byId[container.id] ?? null;
    const entry = Object.freeze({
      container,
      logicalStorage: Object.freeze({
        from: requireValue(logicalPositions[slot], 'document-values'),
        to: requireValue(logicalPositions[slot + 1], 'document-values'),
      }),
      fullMapping: container.mapping
        ? requireValue(mappings.get(container.mapping), 'document-values')
        : null,
      scope,
      semanticMapping: scope ? requireValue(mappings.get(scope.mapping), 'document-values') : null,
    });
    byId[container.id] = entry;
    return entry;
  });
  return Object.freeze({
    scopes,
    syntax: Object.freeze({ ...syntax }),
    sourceIdentity: identity,
    document: state.doc,
    endings,
    entries: Object.freeze(entries),
    byId: Object.freeze(byId),
  });
}
/** Check native currentness by identities without source stringification or renewed command authority. */
function contextFailure(
  state: EditorState,
  context: NativeDocumentValueContext,
): NativeDocumentValueRejection | null {
  const values = context.values;
  if (context.semantics.preparedValues !== values.scopes.values)
    throw new Error('native-document-context-seed-mismatch');
  if (
    values.sourceIdentity !== state.field(editorFreshness).source ||
    values.document !== state.doc ||
    values.endings !== state.field(lineEndings)
  )
    return 'stale-source';
  if (values.scopes.values.kind !== state.facet(documentKind)) return 'stale-context';
  if (state.readOnly) return 'read-only';
  if (context.composing) return 'composing';
  if (!context.enabled) return 'disabled';
  return null;
}
/** Binary storage hit lookup includes protected destinations; it performs no parsing or full-document conversion. */
function entryAt(
  values: PreparedNativeDocumentValues,
  position: number,
): NativeDocumentValue | null {
  if (!Number.isSafeInteger(position) || position < 0) return null;
  let low = 0,
    high = values.entries.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (requireValue(values.entries[middle], 'document-values').logicalStorage.to < position)
      low = middle + 1;
    else high = middle;
  }
  const entry = values.entries[low];
  return entry && entry.logicalStorage.from <= position ? entry : null;
}
/** Admit a half-open logical CM UTF-16 range wholly inside one decoded semantic extent or explicit empty anchor. */
export function nativeDocumentValueAt(
  state: EditorState,
  context: NativeDocumentValueContext,
  logicalRange: SourceRange,
): NativeDocumentValueOutcome {
  const failure = contextFailure(state, context);
  if (failure) return { status: 'rejected', reason: failure };
  if (logicalRange.from > logicalRange.to) return { status: 'rejected', reason: 'unmapped' };
  const first = entryAt(context.values, logicalRange.from),
    last = entryAt(context.values, logicalRange.to);
  if (!first || !last) return { status: 'rejected', reason: 'missing' };
  if (first.container.editability !== 'editable')
    return { status: 'rejected', reason: first.container.editability };
  if (last.container.editability !== 'editable')
    return { status: 'rejected', reason: last.container.editability };
  if (first !== last) return { status: 'rejected', reason: 'cross-value' };
  const scope = first.scope,
    map = first.semanticMapping;
  if (!scope || !map) throw new Error('native-editable-scope-missing');
  const from = map.logicalToDecoded[logicalRange.from - map.logicalRange.from],
    to = map.logicalToDecoded[logicalRange.to - map.logicalRange.from];
  if (from == null || to == null) {
    const full = first.fullMapping;
    const padding =
      full &&
      ((logicalRange.from < map.logicalRange.from &&
        full.logicalToDecoded[logicalRange.from - full.logicalRange.from] != null) ||
        (logicalRange.to > map.logicalRange.to &&
          full.logicalToDecoded[logicalRange.to - full.logicalRange.from] != null));
    return { status: 'rejected', reason: padding ? 'padding' : 'unmapped' };
  }
  const target: NativeDocumentValueTarget = Object.freeze({
    values: context.values,
    entry: first,
    scope,
    decodedRange: Object.freeze({ from, to }),
    freshness: state.field(editorFreshness),
    selection: state.selection,
  });
  issuedTargets.add(target);
  return { status: 'ready', target };
}
/** Revalidate the captured command against the caller's actual current slot immediately before publication, including after callbacks. */
export function currentNativeDocumentValue(
  state: EditorState,
  context: NativeDocumentValueContext,
  target: NativeDocumentValueTarget,
): NativeDocumentValueOutcome {
  if (!issuedTargets.has(target)) return { status: 'rejected', reason: 'foreign' };
  const failure =
    contextFailure(state, context) ??
    freshnessRejection(state.field(editorFreshness), target.freshness);
  if (failure) return { status: 'rejected', reason: failure };
  if (target.selection !== state.selection) return { status: 'rejected', reason: 'stale-context' };
  if (
    target.values !== context.values ||
    context.values.byId[target.entry.container.id] !== target.entry ||
    target.entry.scope !== target.scope
  )
    return { status: 'rejected', reason: 'foreign' };
  const scopes = context.values.scopes;
  const admitted = admitDocumentPromptScope(
    scopes,
    scopes.values.source,
    scopes.values.kind,
    target.scope,
  );
  return admitted.status === 'rejected' ? admitted : { status: 'ready', target };
}

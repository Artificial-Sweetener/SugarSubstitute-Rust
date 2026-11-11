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
/** Canonical grammar scopes delegate storage and coordinates to the immutable document-value owners. */
import type { SourceRange } from '../offsets.js';
import type { PromptDocument, WildcardSyntax } from '../prompt/contracts.js';
import { parsePrompt } from '../prompt/parser.js';
import { parseScenes } from '../prompt/scenes.js';
import { freezeStructure } from '../prompt/text.js';
import { wildcardDelimiters } from '../prompt/wildcards.js';
import { prepareTagQueries } from '../completion/query.js';
import type { PreparedTagQueries } from '../completion/contracts.js';
import { prepareLoraQueries } from '../completion/lora-query.js';
import type { PreparedLoraQueries } from '../completion/lora-contracts.js';
import {
  prepareWildcardQueries,
  type PreparedWildcardQueries,
} from '../completion/wildcard-query.js';
import { prepareSceneQueries, type PreparedSceneQueries } from '../completion/scene-query.js';
import { prepareSceneContexts, type PreparedSceneContexts } from '../scenes/context.js';
import { prepareWildcardGroups, type WildcardGroup } from '../wildcard/groups.js';
import type {
  PreparedDocumentContainer,
  PreparedDocumentValue,
  PreparedDocumentValues,
  PromptDocumentKind,
} from './contracts.js';
import { admitDocumentContainer, documentContainerAtSource } from './containers.js';
import {
  decodedRangeToSource,
  prepareIdentityMapping,
  sourceRangeToDecoded,
  type PreparedValueMapping,
} from './value-mapping.js';

/** All grammar/query/group ranges use decoded scope-local UTF-16; mapping exposes exact-source UTF-16. */
export interface PreparedDocumentPromptScope {
  readonly id: string;
  readonly container: PreparedDocumentContainer;
  readonly value: PreparedDocumentValue | null;
  readonly mapping: PreparedValueMapping;
  readonly document: PromptDocument;
  readonly wildcardGroups: readonly WildcardGroup[];
  readonly tagQueries: PreparedTagQueries;
  readonly loraQueries: PreparedLoraQueries;
  readonly wildcardQueries: PreparedWildcardQueries;
  readonly sceneContexts: PreparedSceneContexts | null;
  readonly sceneQueries: PreparedSceneQueries | null;
}

/** One canonical source/kind/profile derivation; equal canonical inputs retain populated output identity. */
export interface PreparedDocumentPromptScopes {
  readonly values: PreparedDocumentValues;
  readonly profileKey: string;
  readonly scopes: readonly PreparedDocumentPromptScope[];
  readonly byId: Readonly<Record<string, PreparedDocumentPromptScope>>;
}

/** Pure admission never grants editor permissions or refreshes an issued native command. */
export type DocumentPromptScopeOutcome =
  | {
      readonly status: 'ready';
      readonly scope: PreparedDocumentPromptScope;
      readonly decodedRange: SourceRange;
    }
  | {
      readonly status: 'rejected';
      readonly reason:
        | 'stale'
        | 'foreign'
        | 'missing'
        | 'header'
        | 'malformed'
        | 'cross-value'
        | 'padding'
        | 'unmapped';
    };

/** Parse each admitted decoded value once; structural storage and neighboring destinations cannot contribute grammar. */
export function prepareDocumentPromptScopes(
  values: PreparedDocumentValues,
  syntax: WildcardSyntax,
  previous?: PreparedDocumentPromptScopes,
): PreparedDocumentPromptScopes {
  const profileKey = JSON.stringify(wildcardDelimiters(syntax));
  if (previous?.values === values && previous.profileKey === profileKey) return previous;
  const scopes: PreparedDocumentPromptScope[] = [];
  const byId: Record<string, PreparedDocumentPromptScope> = Object.create(null) as Record<
    string,
    PreparedDocumentPromptScope
  >;
  for (const container of values.containers) {
    if (admitDocumentContainer(values, values.source, values.kind, container).status !== 'editable')
      continue;
    const fullMapping = container.mapping;
    if (!fullMapping) throw new Error('missing-editable-container-map');
    const anchor = decodedRangeToSource(fullMapping, container.valueRange);
    if (!anchor) throw new Error('invalid-container-value-map');
    const mapping =
      container.value?.mapping ??
      (values.kind === 'prompt'
        ? fullMapping
        : prepareIdentityMapping(values.sourceOffsets, '', anchor.from));
    const previousScope =
      previous?.values === values && previous.byId[container.id]?.container === container
        ? previous.byId[container.id]
        : undefined;
    const document = parsePrompt(mapping.text, syntax);
    const scope: PreparedDocumentPromptScope = freezeStructure({
      id: container.id,
      container,
      value: container.value,
      mapping,
      document,
      wildcardGroups: prepareWildcardGroups(document.wildcardSpans),
      tagQueries: prepareTagQueries(document),
      loraQueries: prepareLoraQueries(mapping.text),
      wildcardQueries: prepareWildcardQueries(mapping.text),
      sceneContexts:
        values.kind === 'prompt'
          ? (previousScope?.sceneContexts ?? prepareSceneContexts(parseScenes(mapping.text)))
          : null,
      sceneQueries:
        values.kind === 'prompt'
          ? (previousScope?.sceneQueries ?? prepareSceneQueries(mapping.text))
          : null,
    });
    scopes.push(scope);
    byId[scope.id] = scope;
  }
  return freezeStructure({ values, profileKey, scopes, byId });
}

/** Admit only the actual scope/container members of the current canonical snapshot. */
export function admitDocumentPromptScope(
  prepared: PreparedDocumentPromptScopes,
  source: string,
  kind: PromptDocumentKind,
  scope: PreparedDocumentPromptScope,
): DocumentPromptScopeOutcome {
  if (prepared.values.source !== source || prepared.values.kind !== kind)
    return { status: 'rejected', reason: 'stale' };
  if (prepared.byId[scope.id] !== scope) return { status: 'rejected', reason: 'foreign' };
  const admitted = admitDocumentContainer(prepared.values, source, kind, scope.container);
  if (admitted.status !== 'editable') return { status: 'rejected', reason: admitted.reason };
  return {
    status: 'ready',
    scope,
    decodedRange: { from: 0, to: scope.mapping.text.length },
  };
}

/** Resolve complete exact-source UTF-16 containment; padding, storage escapes and cross-value selections reject. */
export function documentPromptScopeAtSource(
  prepared: PreparedDocumentPromptScopes,
  source: string,
  kind: PromptDocumentKind,
  exactRange: SourceRange,
): DocumentPromptScopeOutcome {
  if (prepared.values.source !== source || prepared.values.kind !== kind)
    return { status: 'rejected', reason: 'stale' };
  if (
    !Number.isSafeInteger(exactRange.from) ||
    !Number.isSafeInteger(exactRange.to) ||
    exactRange.from > exactRange.to
  )
    return { status: 'rejected', reason: 'unmapped' };
  const first = documentContainerAtSource(prepared.values, source, kind, exactRange.from);
  const last = documentContainerAtSource(prepared.values, source, kind, exactRange.to);
  if (first.status !== 'editable') return { status: 'rejected', reason: first.reason };
  if (last.status !== 'editable') return { status: 'rejected', reason: last.reason };
  if (first.container !== last.container) return { status: 'rejected', reason: 'cross-value' };
  const scope = prepared.byId[first.container.id];
  if (!scope) throw new Error('missing-editable-document-scope');
  const decodedRange = sourceRangeToDecoded(scope.mapping, exactRange);
  if (!decodedRange) return { status: 'rejected', reason: 'padding' };
  return { status: 'ready', scope, decodedRange };
}

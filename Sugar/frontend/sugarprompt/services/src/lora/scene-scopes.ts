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
/** Source-bound LoRA discovery reparses core-materialized prompts outside input; metadata acquisition remains resource-session owned. */
import { requireValue } from '../../../core/src/required.js';
import {
  parsePrompt,
  type PromptDocument,
  type PreparedSceneContexts,
  type ScenePositionContext,
  type WildcardSyntax,
} from '../../../core/src/index.js';
import { loraResourceNames } from './resource-names.js';
import { assertCompletionBoundary } from '../../../core/src/completion/boundaries.js';
import {
  documentPromptScopeAtSource,
  type PreparedDocumentPromptScopes,
} from '../../../core/src/document/prompt-scopes.js';
import { wildcardDelimiters } from '../../../core/src/prompt/wildcards.js';
/** Ordered actual name outputs are prepared before native position lookup or model scheduling. */
export interface DocumentLoraScopeEntry {
  readonly names: readonly string[];
}
/** Canonical document grammar owns semantic extents; headers, padding and malformed input are unavailable. */
export interface PreparedDocumentLoraScopes {
  readonly documentScopes: PreparedDocumentPromptScopes;
  readonly entries: readonly DocumentLoraScopeEntry[];
  readonly resourceNames: readonly string[];
  /** Exact-source UTF-16 lookup consumes prepared names and canonical inverse maps only. */
  namesAt(position: number): readonly string[] | null;
}
/** Exact authored names for resource acquisition include visible source syntax and every effective scene. */
export interface PreparedSceneLoraScopes {
  readonly contexts: PreparedSceneContexts;
  readonly resourceNames: readonly string[];
  readonly entries: readonly DocumentLoraScopeEntry[];
  /** Logarithmic exact-source UTF-16 lookup reuses scope names; invalid, surrogate-interior and CRLF-interior positions reject. */
  namesAt(position: number): readonly string[];
}
/** Prepare core-owned materialization once per source/profile, retaining names rather than duplicated parsed documents. */
export function prepareSceneLoraScopes(
  document: PromptDocument,
  contexts: PreparedSceneContexts,
  profile: WildcardSyntax = { style: 'curly' },
): PreparedSceneLoraScopes {
  if (document.sourceText !== contexts.document.sourceText)
    throw new Error('Scene LoRA source mismatch');
  const sourceNames = loraResourceNames(document),
    all = new Set(sourceNames),
    names = new Map<ScenePositionContext, readonly string[]>(),
    memo = new Map<string, readonly string[]>();
  for (const context of [contexts.universal, ...contexts.scenes]) {
    const text = context.effectivePromptText;
    let prepared = memo.get(text);
    if (!prepared) {
      prepared =
        text === document.sourceText ? sourceNames : loraResourceNames(parsePrompt(text, profile));
      memo.set(text, prepared);
      if (memo.size > 64) memo.delete(requireValue(memo.keys().next().value, 'scene-scopes'));
    }
    names.set(context, prepared);
    for (const name of prepared) all.add(name);
  }
  const resourceNames = all.size === sourceNames.length ? sourceNames : Object.freeze([...all]);
  return Object.freeze({
    contexts,
    resourceNames,
    entries: Object.freeze(
      [contexts.universal, ...contexts.scenes].map((context) =>
        Object.freeze({ names: requireValue(names.get(context), 'scene-scopes') }),
      ),
    ),
    /** This lookup performs no parsing or metadata traversal. */
    namesAt(position: number): readonly string[] {
      assertCompletionBoundary(document.sourceText, position);
      return requireValue(names.get(contexts.at(position)), 'scene-scopes');
    },
  });
}
/** Structured resource discovery consumes each actual canonical parsed document; prompt materialization retains its existing owner. */
export function prepareDocumentLoraScopes(
  scopes: PreparedDocumentPromptScopes,
  profile: WildcardSyntax = { style: 'curly' },
  previous?: PreparedDocumentLoraScopes,
): PreparedDocumentLoraScopes {
  if (JSON.stringify(wildcardDelimiters(profile)) !== scopes.profileKey)
    throw new Error('document-lora-profile-mismatch');
  if (previous?.documentScopes === scopes) return previous;
  const prompt = scopes.values.kind === 'prompt' ? scopes.scopes[0] : null;
  const scenes = prompt?.sceneContexts
    ? prepareSceneLoraScopes(prompt.document, prompt.sceneContexts, profile)
    : null;
  const names = new Map(scopes.scopes.map((scope) => [scope, loraResourceNames(scope.document)]));
  const entries =
    scenes?.entries ??
    Object.freeze(
      scopes.scopes.map((scope) =>
        Object.freeze({ names: requireValue(names.get(scope), 'scene-scopes') }),
      ),
    );
  const resourceNames =
    scenes?.resourceNames ?? Object.freeze([...new Set(entries.flatMap((entry) => entry.names))]);
  return Object.freeze({
    documentScopes: scopes,
    entries,
    resourceNames,
    /** Canonical admission excludes storage and unsafe exact endpoints without parsing on lookup. */
    namesAt(position: number): readonly string[] | null {
      const admitted = documentPromptScopeAtSource(
        scopes,
        scopes.values.source,
        scopes.values.kind,
        { from: position, to: position },
      );
      if (admitted.status !== 'ready') return null;
      return scenes
        ? scenes.namesAt(position)
        : requireValue(names.get(admitted.scope), 'scene-scopes');
    },
  });
}

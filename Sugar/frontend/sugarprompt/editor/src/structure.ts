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
/** Two bounded slots own canonical source preparation; CodeMirror alone owns editable state. */
import { prepareNativeReorderRecipes } from './reorder-native-recipe.js';
import {
  StateEffect,
  StateField,
  Transaction,
  type EditorState,
  type RangeSet,
} from '@codemirror/state';
import {
  prepareDocumentPromptScopes,
  prepareDocumentSemantics,
  regionNameIdentities,
  type PreparedDocumentPromptScopes,
  type PreparedDocumentSemantics,
  type PreparedSceneContexts,
  type PreparedSceneQueries,
  type PreparedTagQueries,
  type PreparedLoraQueries,
  type PreparedWildcardQueries,
  type PromptStructure,
  type RegionName,
  type WildcardSyntax,
} from '../../core/src/index.js';
import { wildcardDelimiters } from '../../core/src/prompt/wildcards.js';
import {
  prepareDocumentLoraScopes,
  type PreparedDocumentLoraScopes,
} from '../../services/src/lora/scene-scopes.js';
import { documentKind } from './document-kind.js';
import { changesExactSource, lineEndings, readExactSource } from './exact-source.js';
import { editorFreshness } from './freshness.js';
import {
  prepareNativeDocumentValues,
  type PreparedNativeDocumentValues,
} from './document-values.js';
import { type PreparedDocumentValueEdit } from './document-value-edits.js';
import { isIssuedDocumentValueEdit } from './document-value-edit-issuance.js';
import { preparePointerTargets, type PointerTarget } from './pointer-targets.js';
import { prepareRegionTargets, type RegionTarget } from './region-index.js';

/** Native syntax is fixed; inspection configuration never changes its grammar. */
const nativeSyntax: WildcardSyntax = Object.freeze({ style: 'curly' });
const nativeProfileKey = JSON.stringify(wildcardDelimiters(nativeSyntax));
/** Only this preparation owner issues publication members; reconstructed DTOs are foreign. */
const issuedStructures = new WeakMap<PreparedStructure, symbol>();

/** Shared source-only output is immutable; its slot epoch cannot renew command authority. */
interface StructureBase {
  readonly sourceIdentity: symbol;
  readonly slotIdentity: symbol;
  readonly profileKey: string;
  readonly documentSemantics: PreparedDocumentSemantics;
  readonly scopes: PreparedDocumentPromptScopes;
  readonly nativeValues: PreparedNativeDocumentValues;
  readonly regions: readonly RegionTarget[];
  readonly pointerTargets: RangeSet<PointerTarget>;
  readonly loraScopes: PreparedDocumentLoraScopes;
}
/** Prompt-only scenes, peer metadata and whole-prompt queries delegate to the actual canonical scope. */
export interface PreparedPromptStructure extends StructureBase {
  readonly kind: 'prompt';
  readonly structure: PromptStructure;
  readonly regionNames: readonly RegionName[];
  readonly sceneContexts: PreparedSceneContexts;
  readonly tagQueries: PreparedTagQueries;
  readonly loraQueries: PreparedLoraQueries;
  readonly wildcardQueries: PreparedWildcardQueries;
  readonly sceneQueries: PreparedSceneQueries;
}
/** TXT/CSV grammar exists only inside admitted scopes, including explicit empty destinations. */
export interface PreparedStructuredStructure extends StructureBase {
  readonly kind: 'wildcard-txt' | 'wildcard-csv';
}
/** Consumers must narrow prompt-only behavior; structured storage has no whole-source grammar DTO. */
export type PreparedStructure = PreparedPromptStructure | PreparedStructuredStructure;

/** Exactly two current slots with source/kind and inspection retirement epochs; there is no historical memo resurrection. */
interface SemanticStructures {
  readonly native: PreparedStructure | null;
  readonly inspection: PreparedStructure | null;
  readonly identity: symbol;
  readonly inspectionIdentity: symbol;
}
/** Publication purpose selects its slot, never a syntax or permission owner. */
interface StructurePublication {
  readonly purpose: 'native' | 'inspection';
  readonly prepared: PreparedStructure;
}
/** Source/kind retirement creates a new authority epoch even when source bytes later return. */
function emptyStructures(): SemanticStructures {
  return Object.freeze({
    native: null,
    inspection: null,
    identity: Symbol(),
    inspectionIdentity: Symbol(),
  });
}
/** Publish actual source-current derived output without parsing in a transaction. */
export const installSemanticStructure = StateEffect.define<StructurePublication>();
/** Own the native and latest inspection results; presentation/context changes retain source-only objects. */
export const semanticStructures = StateField.define<SemanticStructures>({
  /** Initial preparation belongs to explicit inspection or deferred native settlement. */
  create: emptyStructures,
  /** Retire slots on source/kind changes and reject foreign/stale publications by actual provenance. */
  update(cache, transaction) {
    if (
      changesExactSource(transaction) ||
      transaction.startState.facet(documentKind) !== transaction.state.facet(documentKind)
    )
      return emptyStructures();
    let next = cache;
    for (const effect of transaction.effects) {
      if (!effect.is(installSemanticStructure)) continue;
      const { purpose, prepared } = effect.value;
      if (
        !issuedStructures.has(prepared) ||
        prepared.slotIdentity !== cache.identity ||
        prepared.sourceIdentity !== transaction.state.field(editorFreshness).source ||
        prepared.kind !== transaction.state.facet(documentKind) ||
        prepared.nativeValues.document !== transaction.state.doc ||
        prepared.nativeValues.endings !== transaction.state.field(lineEndings)
      )
        continue;
      if (purpose === 'native' && prepared.profileKey !== nativeProfileKey)
        throw new Error('Native semantic grammar mismatch');
      if (
        purpose === 'inspection' &&
        next.inspection !== prepared &&
        next.native !== prepared &&
        issuedStructures.get(prepared) !== next.inspectionIdentity
      )
        continue;
      if (next[purpose] !== prepared)
        next = Object.freeze({
          ...next,
          [purpose]: prepared,
          inspectionIdentity: purpose === 'inspection' ? Symbol() : next.inspectionIdentity,
        });
    }
    return next;
  },
});

/** Adopt only an actual adapter result on its actual resulting state, preserving all seeded object identities. */
function admitSeed(state: EditorState, profileKey: string, seed: PreparedDocumentValueEdit): void {
  const oldSlots = seed.startState.field(semanticStructures, false);
  const original = [oldSlots?.native, oldSlots?.inspection].find(
    (slot) => slot?.nativeValues === seed.currentValues,
  );
  if (
    !original ||
    !isIssuedDocumentValueEdit(state, profileKey, seed, original.documentSemantics) ||
    seed.postValues.sourceIdentity !== state.field(editorFreshness).source ||
    seed.postValues.document !== state.doc ||
    seed.postValues.endings !== state.field(lineEndings) ||
    seed.preparedValues.kind !== state.facet(documentKind)
  )
    throw new Error('semantic-structure-seed-mismatch');
}
/** Yield between canonical preparation phases; only the complete result receives publication provenance. */
function* prepareProfile(
  state: EditorState,
  profile: WildcardSyntax,
  seed?: PreparedDocumentValueEdit,
): Generator<void, PreparedStructure, void> {
  const profileKey = JSON.stringify(wildcardDelimiters(profile));
  if (seed) admitSeed(state, profileKey, seed);
  const cache = state.field(semanticStructures);
  for (const prepared of [cache.native, cache.inspection])
    if (prepared?.profileKey === profileKey && (!seed || prepared.nativeValues === seed.postValues))
      return prepared;
  const previous = cache.native ?? cache.inspection;
  const documentSemantics =
    seed?.semantics ??
    previous?.documentSemantics ??
    prepareDocumentSemantics(readExactSource(state), state.facet(documentKind));
  yield;
  const scopes =
    seed?.postValues.scopes ??
    prepareDocumentPromptScopes(documentSemantics.preparedValues, profile, previous?.scopes);
  yield;
  const nativeValues = seed?.postValues ?? prepareNativeDocumentValues(state, scopes, profile);
  yield;
  if (profileKey === nativeProfileKey) prepareNativeReorderRecipes(nativeValues);
  yield;
  const base: StructureBase = {
    sourceIdentity: state.field(editorFreshness).source,
    slotIdentity: cache.identity,
    profileKey,
    documentSemantics,
    scopes,
    nativeValues,
    regions: prepareRegionTargets(nativeValues),
    pointerTargets: preparePointerTargets(nativeValues),
    loraScopes: prepareDocumentLoraScopes(scopes, profile),
  };
  yield;
  let prepared: PreparedStructure;
  if (documentSemantics.kind === 'prompt') {
    const scope = scopes.scopes[0];
    if (!scope?.sceneContexts || !scope.sceneQueries)
      throw new Error('canonical-prompt-scope-missing');
    prepared = Object.freeze({
      ...base,
      kind: 'prompt',
      structure: Object.freeze({
        document: scope.document,
        scenes: scope.sceneContexts.document,
      }),
      regionNames: regionNameIdentities(scope.document),
      sceneContexts: scope.sceneContexts,
      tagQueries: scope.tagQueries,
      loraQueries: scope.loraQueries,
      wildcardQueries: scope.wildcardQueries,
      sceneQueries: scope.sceneQueries,
    });
  } else prepared = Object.freeze({ ...base, kind: documentSemantics.kind });
  issuedStructures.set(prepared, cache.inspectionIdentity);
  return prepared;
}
/** Complete explicit command/inspection preparation through the same canonical phase owner. */
function completePreparation(steps: Generator<void, PreparedStructure, void>): PreparedStructure {
  for (;;) {
    const result = steps.next();
    if (result.done) return result.value;
  }
}
/** Expose cooperative native preparation without granting partial structures publication authority. */
export function prepareNativeStructureSteps(
  state: EditorState,
): Generator<void, PreparedStructure, void> {
  return prepareProfile(state, nativeSyntax);
}
/** Prepare the fixed native profile, optionally adopting actual adapter post-edit seeds. */
export function prepareNativeStructure(
  state: EditorState,
  seed?: PreparedDocumentValueEdit,
): PreparedStructure {
  return completePreparation(prepareProfile(state, nativeSyntax, seed));
}
/** Initialize native semantics through supported effects without affecting source, caret or history. */
export function initializeNativeStructure(state: EditorState): EditorState {
  if (state.field(semanticStructures).native) return state;
  return state.update({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: prepareNativeStructure(state),
    }),
    annotations: Transaction.addToHistory.of(false),
  }).state;
}
/** Prepare an isolated read-only profile in the existing inspection slot. */
export function prepareInspectionStructure(
  state: EditorState,
  profile: WildcardSyntax,
  seed?: PreparedDocumentValueEdit,
): PreparedStructure {
  return completePreparation(prepareProfile(state, profile, seed));
}

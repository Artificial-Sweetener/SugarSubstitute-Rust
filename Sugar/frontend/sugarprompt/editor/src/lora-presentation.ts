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
/** Native projection indexes validated service descriptors without owning catalog decisions or editable source. */
import { StateEffect, StateField, type EditorState } from '@codemirror/state';
import type {
  LoraResource,
  LoraResourceSnapshot,
  LoraPageRequest,
} from '../../services/src/index.js';
import { isCatalogStableLoraResource, prepareLoraPage } from '../../services/src/index.js';
import { editorFreshness } from './freshness.js';

/** One accepted catalog metadata index with a source-bound publication token; the service owns descriptor and request authority. */
export interface LoraPresentation {
  readonly sourceIdentity: symbol;
  readonly snapshot: LoraResourceSnapshot;
  readonly byName: ReadonlyMap<string, LoraResource>;
  /** Source-current prepared external intents; input callbacks perform no URL parsing or catalog traversal. */
  readonly pages: ReadonlyMap<string, LoraPageRequest>;
}

/** Publish an already prepared index; stale source identities cannot install external metadata. */
export const installLoraPresentation = StateEffect.define<LoraPresentation>();

/** Default native hosts have no external catalog; absence remains explicit in every recognized chip. */
const unavailable: LoraResourceSnapshot = Object.freeze({
  status: 'unavailable',
  resources: Object.freeze([]),
});

/** Derive exact-name indexing outside input/render callbacks, preserving actual validated record identities. */
export function prepareLoraPresentation(
  state: EditorState,
  snapshot: LoraResourceSnapshot,
): LoraPresentation {
  const pages = new Map<string, LoraPageRequest>();
  for (const resource of snapshot.resources) {
    const page = prepareLoraPage(resource);
    if (page) pages.set(resource.name, page);
  }
  return {
    sourceIdentity: state.field(editorFreshness).source,
    snapshot,
    byName: new Map(snapshot.resources.map((value) => [value.name, value])),
    pages,
  };
}

/** Own accepted catalog records and source-bound publication identity; exact-name metadata survives unrelated source edits. */
export const loraPresentation = StateField.define<LoraPresentation>({
  /** No parser, host request or resource acquisition occurs during state creation. */
  create(state) {
    return prepareLoraPresentation(state, unavailable);
  },
  /** Selection and unrelated transactions reuse the actual index; only source-current publications replace it. */
  update(prepared, transaction) {
    const identity = transaction.state.field(editorFreshness).source;
    // Ready empty coverage belongs to its source; adding a LoRA must await new resource authority.
    if (
      prepared.sourceIdentity !== identity &&
      prepared.snapshot.resources.length === 0 &&
      prepared.snapshot.status !== 'ready'
    )
      prepared = { ...prepared, sourceIdentity: identity };
    for (const effect of transaction.effects)
      if (
        effect.is(installLoraPresentation) &&
        effect.value.sourceIdentity === transaction.state.field(editorFreshness).source
      )
        prepared = effect.value;
    return prepared;
  },
});

/** Reuse catalog-valid exact-name records for current parsed tokens; transient authority outcomes remain source-bound and unknown names stay unresolved. */
export function projectedLora(state: EditorState, name: string): LoraResource {
  const prepared = state.field(loraPresentation);
  const known = prepared.byName.get(name);
  const record =
    prepared.sourceIdentity === state.field(editorFreshness).source ||
    (known && isCatalogStableLoraResource(known))
      ? known
      : undefined;
  return (
    record ?? {
      name,
      resolution:
        prepared.snapshot.status === 'unavailable' ||
        prepared.snapshot.status === 'failed' ||
        prepared.snapshot.status === 'disposed'
          ? 'unavailable'
          : 'pending-authority',
    }
  );
}

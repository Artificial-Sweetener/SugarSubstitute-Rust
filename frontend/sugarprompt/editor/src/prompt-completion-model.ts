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
/** Derived completion metadata belongs to native CodeMirror options, never a parallel selected-row/session owner. */
import { Facet, type EditorState } from '@codemirror/state';
import type {
  TagCompletionCatalog,
  LoraCompletionCatalog,
  LoraCatalogItem,
  SceneTitleCatalog,
} from '../../services/src/index.js';
import type { PreparedDocumentPromptScope } from '../../core/src/index.js';
import type { NativeDocumentValueContext, NativeDocumentValueTarget } from './document-values.js';
import type { PreparedDocumentValueEdit } from './document-value-edits.js';
import type { EditorFreshness } from './freshness.js';

/** One reactive host catalog; native reconfiguration preserves canonical source/history. */
export const tagCompletionCatalog = Facet.define<
  TagCompletionCatalog | null,
  TagCompletionCatalog | null
>({
  /** Reject multiple catalogs competing to rank the same native completion session. */
  combine(values) {
    if (values.length > 1) throw new Error('Multiple tag completion catalogs');
    return values[0] ?? null;
  },
});

/** One prepared workflow-title publication supplies the scene branch of the native completion owner. */
export const sceneTitleCatalog = Facet.define<SceneTitleCatalog | null, SceneTitleCatalog | null>({
  /** Competing title authorities cannot share one native suggestion session. */
  combine(values) {
    if (values.length > 1) throw new Error('Multiple scene title catalogs');
    return values[0] ?? null;
  },
});

/** One prepared model catalog supplies the LoRA branch of the same native completion session. */
export const loraCompletionCatalog = Facet.define<
  LoraCompletionCatalog | null,
  LoraCompletionCatalog | null
>({
  /** Catalog replacement is a context change; competing model catalog owners are rejected. */
  combine(values) {
    if (values.length > 1) throw new Error('Multiple LoRA completion catalogs');
    return values[0] ?? null;
  },
});

/**
 * Ranked display metadata for one row. Showing and filtering a list never builds an edit: the
 * accepted edit is prepared only when the user accepts the row, against the state at that moment.
 */
interface CompletionProposal {
  readonly label: string;
  readonly detail: string;
  readonly freshness: EditorFreshness;
  readonly caret: number;
  readonly ghost: string;
  /** Build this row's accepted edit for one activation against the current state; null refuses. */
  readonly prepareEdit: CompletionEditPreparation;
}

/** Native activation chooses its delimiter policy; Tab is keyboard, a row click is pointer. */
export type CompletionActivation = 'keyboard' | 'pointer';

/** Issue one activation's canonical native edit for the current state, or refuse when it cannot be admitted. */
export type CompletionEditPreparation = (
  state: EditorState,
  activation: CompletionActivation,
) => PreparedDocumentValueEdit | null;
/** One immutable candidate carries its literal display input and its deferred edit preparation. */
export type PreparedCompletion = CompletionProposal &
  (
    | { readonly kind: 'tag' }
    | { readonly kind: 'wildcard' }
    | { readonly kind: 'scene' }
    | { readonly kind: 'trigger'; readonly backendValue: string }
    | { readonly kind: 'lora'; readonly item: LoraCatalogItem }
  );

/** Logical-document UTF-16 anchors locate one source-current native suggestion session. */
export interface PreparedCompletionResult {
  readonly from: number;
  readonly to: number;
  readonly query: CompletionQuery;
  readonly options: readonly PreparedCompletion[];
}

/** Prepared logical-document UTF-16 query bounds distinguish its opener, editable content and final content boundary. */
export interface CompletionQuery {
  readonly scope: PreparedDocumentPromptScope;
  readonly kind: 'tag' | 'lora' | 'wildcard' | 'scene';
  readonly from: number;
  readonly contentFrom: number;
  readonly to: number;
}

/** One actual admitted caret scope binds decoded query preparation to canonical native edit authority. */
export interface PreparedCompletionScope {
  readonly context: NativeDocumentValueContext;
  readonly target: NativeDocumentValueTarget;
}

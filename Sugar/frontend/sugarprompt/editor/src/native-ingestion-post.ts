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
/** One issued source-neutral command post carries canonical paint through the existing native ingestion adapter. */
import { Transaction, type EditorState } from '@codemirror/state';
import type { PreparedDocumentValueEdit } from './document-value-edits.js';
import { prepareNativeStructure, installSemanticStructure } from './structure.js';
import { prepareProjection } from './projection-preparation.js';
import {
  currentCanonicalProjection,
  installProjection,
  projectsRichSyntax,
  richProjection,
} from './rich-projection.js';
import type { PreparedProjection } from './projection-contracts.js';

/** Exact transactions/effects retain their issued artifact, not permission to attach arbitrary metadata. */
interface IssuedPost {
  readonly edit: PreparedDocumentValueEdit;
  readonly startState: EditorState;
  readonly semantic: ReturnType<typeof installSemanticStructure.of>;
  readonly projection: ReturnType<typeof installProjection.of>;
  readonly value: PreparedProjection;
  readonly payload: Readonly<PreparedProjection>;
  readonly all: PreparedProjection['all'];
  readonly allAtomic: PreparedProjection['allAtomic'];
  readonly tokens: PreparedProjection['tokens'];
  readonly decorations: PreparedProjection['decorations'];
  readonly atomic: PreparedProjection['atomic'];
}
const issuedPosts = new WeakMap<Transaction, IssuedPost>();

/** Compile once at command preparation; raw or absent rich presentation keeps its source-neutral semantic post. */
export function prepareNativeIngestionPost(
  state: EditorState,
  edit: PreparedDocumentValueEdit,
): Transaction {
  const semantic = installSemanticStructure.of({
      purpose: 'native',
      prepared: prepareNativeStructure(state, edit),
    }),
    value =
      state.field(richProjection, false) && projectsRichSyntax(state)
        ? prepareProjection(state, semantic.value.prepared)
        : null,
    projection = value ? installProjection.of(value) : null,
    post = state.update({
      effects: projection ? [semantic, projection] : semantic,
      annotations: Transaction.addToHistory.of(false),
    });
  if (projection && value) {
    const payload = Object.freeze({ ...value }),
      painted = post.state.field(richProjection);
    issuedPosts.set(post, {
      edit,
      startState: state,
      semantic,
      projection,
      value,
      payload,
      all: painted.all,
      allAtomic: painted.allAtomic,
      tokens: painted.tokens,
      decorations: painted.decorations,
      atomic: painted.atomic,
    });
  }
  return post;
}

/** Authenticate only this exact issued effect pair; the adapter separately owns source/history/selection continuity. */
export function currentNativeIngestionPost(
  transaction: Transaction,
  edit: PreparedDocumentValueEdit | undefined,
): boolean {
  const issued = issuedPosts.get(transaction);
  if (
    !issued ||
    issued.edit !== edit ||
    transaction.startState !== issued.startState ||
    transaction.effects.length !== 2 ||
    transaction.effects[0] !== issued.semantic ||
    transaction.effects[1] !== issued.projection ||
    issued.projection.value !== issued.value ||
    !Object.keys(issued.payload).every(
      (key) =>
        issued.value[key as keyof PreparedProjection] ===
        issued.payload[key as keyof PreparedProjection],
    )
  )
    return false;
  const canonical = currentCanonicalProjection(transaction.state),
    painted = transaction.state.field(richProjection);
  return (
    canonical !== null &&
    canonical.all === issued.all &&
    canonical.allAtomic === issued.allAtomic &&
    canonical.tokens === issued.tokens &&
    painted.decorations === issued.decorations &&
    painted.atomic === issued.atomic
  );
}

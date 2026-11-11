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
/** Actual native state fixtures keep source/newline/history authority in existing owners. */
import { Compartment, EditorState, Text, type Transaction } from '@codemirror/state';
import { history } from '@codemirror/commands';
import {
  prepareDocumentPromptScopes,
  prepareDocumentSemantics,
  type PromptDocumentKind,
  type WildcardSyntax,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { prepareDocumentValues } from '../../../../../../frontend/sugarprompt/core/src/document/values.ts';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import {
  exactSourceExtension,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import {
  prepareNativeDocumentValues,
  nativeDocumentValueAt,
  type NativeDocumentValueContext,
  type NativeDocumentValueTarget,
} from '../../../../../../frontend/sugarprompt/editor/src/document-values.ts';
import {
  type DecodedDocumentValueEdit,
  type PreparedDocumentValueEdit,
} from '../../../../../../frontend/sugarprompt/editor/src/document-value-edits.ts';
import { prepareDocumentValueEdit } from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-issuance.ts';

/** Reconfigurable actual facets permit source/kind/profile/permission retirement tests without a view double. */
export function valueState(source: string, kind: PromptDocumentKind = 'wildcard-csv') {
  const kindSlot = new Compartment(),
    permission = new Compartment(),
    profile = new Compartment();
  const state = EditorState.create({
    doc: Text.of(encodeSource(source).logical.split('\n')),
    extensions: [
      exactSourceExtension(source),
      editorFreshness,
      history(),
      EditorState.allowMultipleSelections.of(true),
      kindSlot.of(documentKind.of(kind)),
      permission.of(EditorState.readOnly.of(false)),
      profile.of([]),
    ],
  });
  return { state, kindSlot, permission, profile };
}

/** Prepare outside transaction publication, passing the actual canonical snapshot and syntax explicitly. */
export function valueContext(
  state: EditorState,
  syntax: WildcardSyntax = { style: 'curly' },
): NativeDocumentValueContext {
  const values = prepareDocumentValues(readExactSource(state), state.facet(documentKind));
  const scopes = prepareDocumentPromptScopes(values, syntax);
  return {
    values: prepareNativeDocumentValues(state, scopes, syntax),
    semantics: prepareDocumentSemantics(values.source, values.kind, undefined, values),
    enabled: true,
    composing: false,
  };
}

/** Admit a literal logical caret; absence is a fixture invariant, never a fallback target. */
export function valueTarget(
  state: EditorState,
  context: NativeDocumentValueContext,
  position: number,
): NativeDocumentValueTarget {
  const result = nativeDocumentValueAt(state, context, {
    from: position,
    to: position,
  });
  if (result.status !== 'ready') throw new Error('fixture-target-unavailable');
  return result.target;
}

/** Closed decoded request fixture preserves independently supplied literal ranges and selection endpoints. */
export function decodedEdit(
  context: NativeDocumentValueContext,
  target: NativeDocumentValueTarget,
  from: number,
  to: number,
  text: string,
  anchor = from + text.length,
  head = anchor,
): DecodedDocumentValueEdit {
  return {
    context,
    target,
    range: { from, to },
    text,
    selection: { ranges: [{ kind: 'decoded', anchor, head }], mainIndex: 0 },
  };
}

/** Ready/no-op extraction avoids printing authored specimen text in failed assertions. */
export function acceptedEdit(
  state: EditorState,
  intent: DecodedDocumentValueEdit,
): PreparedDocumentValueEdit {
  const outcome = prepareDocumentValueEdit(state, intent);
  if (outcome.status === 'rejected') throw new Error(`fixture-edit-${outcome.reason}`);
  return outcome.edit;
}

/** Apply exactly the supported prepared transaction; no custom source/history state exists. */
export function editedState(edit: PreparedDocumentValueEdit): EditorState {
  return edit.transactions.reduce((state: EditorState, transaction: Transaction) => {
    if (transaction.startState !== state) throw new Error('fixture-transaction-start-mismatch');
    return transaction.state;
  }, edit.startState);
}

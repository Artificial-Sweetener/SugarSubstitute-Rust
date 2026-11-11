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
/** Shared actual-CM structure state, publication and decoded seed fixtures. */
import { Compartment, EditorState, Text, type Extension } from '@codemirror/state';
import { history } from '@codemirror/commands';
import type { PromptDocumentKind } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import { exactSourceExtension } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import {
  installSemanticStructure,
  semanticStructures,
  type PreparedStructure,
  type PreparedPromptStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  nativeDocumentValueAt,
  type NativeDocumentValueContext,
} from '../../../../../../frontend/sugarprompt/editor/src/document-values.ts';
import { type PreparedDocumentValueEdit } from '../../../../../../frontend/sugarprompt/editor/src/document-value-edits.ts';
import { prepareDocumentValueEdit } from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-issuance.ts';

/** Reconfigurable real CM source/history/permission/kind state; additional extensions are explicit proof inputs. */
export function structureState(
  source: string,
  kind: PromptDocumentKind = 'prompt',
  extra: Extension = [],
) {
  const kindSlot = new Compartment(),
    permission = new Compartment();
  const state = EditorState.create({
    doc: Text.of(encodeSource(source).logical.split('\n')),
    extensions: [
      exactSourceExtension(source),
      editorFreshness,
      semanticStructures,
      history(),
      EditorState.allowMultipleSelections.of(true),
      kindSlot.of(documentKind.of(kind)),
      permission.of(EditorState.readOnly.of(false)),
      extra,
    ],
  });
  return { state, kindSlot, permission };
}

/** Preserve complete original prompt assertions while requiring the real discriminant. */
export function promptStructure(prepared: PreparedStructure): PreparedPromptStructure {
  if (prepared.kind !== 'prompt') throw new Error('fixture-prompt-required');
  return prepared;
}

/** Install actual prepared output through its existing supported effect. */
export function publishStructure(
  state: EditorState,
  prepared: PreparedStructure,
  purpose: 'native' | 'inspection' = 'native',
): EditorState {
  return state.update({
    effects: installSemanticStructure.of({ purpose, prepared }),
  }).state;
}

/** Supply existing caller-owned policy observations with the actual current slot output. */
function context(prepared: PreparedStructure): NativeDocumentValueContext {
  return {
    values: prepared.nativeValues,
    semantics: prepared.documentSemantics,
    enabled: true,
    composing: false,
  };
}

/** Independent decoded edit intent retains the actual adapter transaction and output members. */
export function editAt(
  state: ReturnType<typeof structureState>['state'],
  prepared: PreparedStructure,
  position: number,
  text: string,
): PreparedDocumentValueEdit {
  const current = context(prepared);
  const target = nativeDocumentValueAt(state, current, {
    from: position,
    to: position,
  });
  if (target.status !== 'ready') throw new Error('fixture-target-unavailable');
  const edit = prepareDocumentValueEdit(state, {
    context: current,
    target: target.target,
    range: { from: 0, to: 0 },
    text,
    selection: {
      ranges: [{ kind: 'decoded', anchor: text.length, head: 0 }],
      mainIndex: 0,
    },
  });
  if (edit.status === 'rejected') throw new Error('fixture-edit-unavailable');
  return edit.edit;
}

export { context };

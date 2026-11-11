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
/** Shared native source/history fixture owns only state construction for numeric command contracts. */
import { EditorState, Text } from '@codemirror/state';
import { history } from '@codemirror/commands';
import type { PromptDocumentKind } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import { exactSourceExtension } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  semanticStructures,
  prepareNativeStructure,
  installSemanticStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { richProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { loraPresentation } from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import { loraMedia } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import { weightSession } from '../../../../../../frontend/sugarprompt/editor/src/weight-session.ts';
import { sceneErrors } from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';

/** Create the actual source, history and derived owners with an independently selectable caret. */
export function weightStateFor(
  source: string,
  anchor = 0,
  head = anchor,
  kind: PromptDocumentKind = 'prompt',
): EditorState {
  const state = EditorState.create({
    doc: Text.of(encodeSource(source).logical.split('\n')),
    selection: { anchor, head },
    extensions: [
      exactSourceExtension(source),
      editorFreshness,
      history(),
      documentKind.of(kind),
      EditorState.allowMultipleSelections.of(true),
      semanticStructures,
      sceneErrors,
      loraPresentation,
      loraMedia,
      richProjection,
      weightSession,
    ],
  });
  return state.update({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: prepareNativeStructure(state),
    }),
  }).state;
}

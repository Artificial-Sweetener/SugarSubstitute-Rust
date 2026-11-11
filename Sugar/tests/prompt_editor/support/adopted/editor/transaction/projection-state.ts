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
/** Real native projection/history fixtures share one construction owner without a browser or duplicated semantics. */
import { Compartment, EditorState, Text } from '@codemirror/state';
import { history } from '@codemirror/commands';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import { exactSourceExtension } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  richProjection,
  richProjectionExtension,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { loraPresentation } from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import { loraMedia } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import { sceneErrors } from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';
import type { PromptDocumentKind } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';

/** Mount the actual source, derived projection and native history owners; browser proof owns DOM and trusted input. */
export function projectionState(
  source: string,
  presentation = new Compartment(),
  kind: PromptDocumentKind = 'prompt',
): EditorState {
  return EditorState.create({
    doc: Text.of(encodeSource(source).logical.split('\n')),
    extensions: [
      exactSourceExtension(source),
      documentKind.of(kind),
      editorFreshness,
      semanticStructures,
      sceneErrors,
      loraPresentation,
      loraMedia,
      history(),
      presentation.of(richRendering.of(true)),
      richProjectionExtension,
    ],
  });
}

/** Observe actual native replacement ranges using logical-document UTF-16. */
export function projectionRanges(state: EditorState): readonly (readonly number[])[] {
  const observed: number[][] = [];
  state.field(richProjection).decorations.between(0, state.doc.length, (from, to) => {
    observed.push([from, to]);
  });
  return observed;
}

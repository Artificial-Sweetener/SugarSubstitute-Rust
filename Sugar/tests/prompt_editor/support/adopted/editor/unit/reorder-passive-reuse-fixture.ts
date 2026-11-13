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
/** Real prepared canonical rasters support finite passive-slot ownership proof; native layout remains external. */
import assert from 'node:assert/strict';
import {
  initializeNativeStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { projectionState } from '../transaction/projection-state.ts';
import {
  prepareLoraPresentation,
  installLoraPresentation,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import { readLoraResources } from '../../../../../../frontend/sugarprompt/services/src/lora/validation.ts';
import { installLoraMedia } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
/** Prepare a real canonical LoRA with finite injected pixels; normal and readonly widgets consume the same raster identity. */
export function preparedLoraState(
  onPaint: (canvas: HTMLCanvasElement) => void,
  count = 1,
  width = 16,
  height = 8,
) {
  let state = initializeNativeStructure(
    projectionState(
      count === 1
        ? 'a, <lora:fixture:0.70>, c'
        : Array.from({ length: count }, () => '<lora:fixture:0.70>').join(', '),
    ),
  );
  const resources = readLoraResources(
    [
      {
        name: 'fixture',
        resolution: 'found',
        model: {
          promptName: 'fixture',
          backendValue: 'fixture.safetensors',
          basename: 'fixture',
        },
        modelPageUrl: null,
        title: 'Fixture',
        subtitle: null,
        triggerWords: [],
        thumbnail: {
          status: 'ready',
          url: 'https://example.invalid/fixture.png',
          width,
          height,
        },
      },
    ],
    ['fixture'],
  );
  const resource = resources[0];
  assert.ok(resource?.resolution === 'found' && resource.thumbnail.status === 'ready');
  state = state.update({
    effects: installLoraPresentation.of(
      prepareLoraPresentation(state, { status: 'ready', resources }),
    ),
  }).state;
  state = state.update({
    effects: installLoraMedia.of({
      freshness: state.field(editorFreshness),
      media: new Map([
        [
          resource.thumbnail,
          {
            status: 'ready',
            raster: {
              width,
              height,
              /** Paint is an injected finite external callback; no acquisition or decode participates in the native phase proof. */
              paint: onPaint,
              /** The readonly painter never owns prepared raster disposal. */
              dispose() {
                assert.fail('borrowed-raster-disposal');
              },
            },
          },
        ],
      ]),
    }),
  }).state;
  return state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
}

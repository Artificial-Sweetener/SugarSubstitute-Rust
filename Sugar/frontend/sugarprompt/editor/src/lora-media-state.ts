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
/** One native rendering input shares view-owned decoded pixels between source chips and model completion tiles. */
import { StateEffect, StateField, Transaction, type EditorState } from '@codemirror/state';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import type { LoraMediaReference, PreparedLoraMedia } from './lora-media-contracts.js';

/** Immutable derived raster references; only the mounted media pool owns acquisition, cancellation and disposal. */
export type LoraMediaSnapshot = ReadonlyMap<LoraMediaReference, PreparedLoraMedia>;

/** A publication is prepared outside input/paint and rejected if its native source/context has retired. */
export const installLoraMedia = StateEffect.define<{
  readonly freshness: EditorFreshness;
  readonly media: LoraMediaSnapshot;
}>();

/** Descriptor-keyed pixels have no source semantics or editable state authority. */
export const loraMedia = StateField.define<LoraMediaSnapshot>({
  /** A new native view starts with no decoded media. */
  create() {
    return new Map();
  },
  /** Reuse actual prepared objects across native selection and accept only current pool publications. */
  update(media, transaction) {
    for (const effect of transaction.effects)
      if (
        effect.is(installLoraMedia) &&
        effect.value.freshness === transaction.state.field(editorFreshness)
      )
        media = effect.value.media;
    return media;
  },
});

/** Detach derived pixels before their native pool is disposed; the replacement view prepares fresh media from retained host descriptors. */
export function retireLoraMedia(state: EditorState): EditorState {
  if (!state.field(loraMedia).size) return state;
  return state.update({
    effects: installLoraMedia.of({
      freshness: state.field(editorFreshness),
      media: new Map(),
    }),
    annotations: Transaction.addToHistory.of(false),
  }).state;
}

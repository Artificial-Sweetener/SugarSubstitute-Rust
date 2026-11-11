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
/** Native media detachment retires view-owned references while preserving CodeMirror's canonical source, selection and history values. */
import test from 'node:test';
import assert from 'node:assert/strict';
import { EditorState, EditorSelection } from '@codemirror/state';
import { history, historyField } from '@codemirror/commands';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  installLoraMedia,
  loraMedia,
  retireLoraMedia,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import type { LoraMediaReference } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-contracts.ts';

for (const occupied of [false, true])
  test(`${occupied ? 'occupied' : 'empty'} native media retirement preserves actual canonical values and leaves raster disposal to its pool`, () => {
    let state = EditorState.create({
      doc: 'base',
      extensions: [history(), editorFreshness, loraMedia],
    });
    state = state.update({
      changes: { from: 0, insert: 'x' },
      selection: EditorSelection.single(3, 1),
      userEvent: 'input.type',
    }).state;
    let disposed = false;
    if (occupied) {
      const reference: LoraMediaReference = {
        status: 'ready',
        url: 'data:image/png;base64,fixture',
        width: 1,
        height: 1,
      };
      state = state.update({
        effects: installLoraMedia.of({
          freshness: state.field(editorFreshness),
          media: new Map([
            [
              reference,
              {
                status: 'ready',
                raster: {
                  width: 1,
                  height: 1,
                  /** This proof never paints or acquires browser media. */
                  paint() {},
                  /** Native pool teardown, rather than derived-state detachment, owns resource release. */
                  dispose() {
                    disposed = true;
                  },
                },
              },
            ],
          ]),
        }),
      }).state;
    }
    const current = retireLoraMedia(state);
    assert.equal(current === state, !occupied);
    assert.equal(current.doc, state.doc);
    assert.equal(current.selection, state.selection);
    assert.equal(current.field(historyField), state.field(historyField));
    assert.equal(current.field(editorFreshness), state.field(editorFreshness));
    assert.equal(current.field(loraMedia).size, 0);
    assert.equal(disposed, false);
  });

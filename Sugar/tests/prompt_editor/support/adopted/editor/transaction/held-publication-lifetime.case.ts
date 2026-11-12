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
/** Native follow-up intent and held exposure survive derived publication while real ownership boundaries retire them. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { heldTokenFixture, heldTokenPointer } from './held-token-fixture.ts';
import { heldProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-exposure.ts';
import {
  installLoraMedia,
  loraMedia,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import {
  installLoraPresentation,
  loraPresentation,
  prepareLoraPresentation,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import {
  installSceneErrors,
  sceneErrors,
  prepareSceneErrors,
} from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  installProjection,
  richProjection,
  currentInputProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

for (const [source, expected] of [
  ['alpha <lora:real:1>, tail', [7, 11]],
  ['alpha bravo, tail', [6, 11]],
] as const) {
  test(`native completed double-click retains next word intent for ${source.includes('<') ? 'token' : 'segment'}`, () => {
    const fixture = heldTokenFixture(source);
    try {
      const first = fixture.press(heldTokenPointer(8, 2));
      fixture.select(first.style, first.event);
      fixture.document.phase('mouseup', new Event('mouseup'), false);
      const second = fixture.press(heldTokenPointer(8, 1));
      fixture.select(second.style, second.event);
      assert.deepEqual(
        [fixture.state.selection.main.anchor, fixture.state.selection.main.head],
        expected,
      );
      assert.equal(readExactSource(fixture.state), source);
      assert.deepEqual([undoDepth(fixture.state), redoDepth(fixture.state)], [0, 0]);
    } finally {
      fixture.destroy();
    }
    assert.equal(fixture.document.active + fixture.window.active, 0);
  });
}

for (const mode of ['media', 'presentation', 'scene-errors', 'context'] as const) {
  test(`${mode} actual publication preserves only source-neutral held projection authority`, () => {
    const fixture = heldTokenFixture();
    try {
      const first = fixture.press();
      fixture.select(first.style, first.event);
      fixture.select(first.style, heldTokenPointer(22));
      const freshness = fixture.state.field(editorFreshness);
      const held = fixture.state.field(heldProjection);
      assert.ok(held);
      const selection = fixture.state.selection;
      if (mode === 'media') {
        const media = new Map();
        fixture.dispatch({
          effects: installLoraMedia.of({ freshness, media }),
        });
        assert.equal(fixture.state.field(loraMedia), media);
      } else if (mode === 'presentation') {
        const prepared = prepareLoraPresentation(fixture.state, {
          status: 'ready',
          resources: [{ name: 'real', resolution: 'missing' }],
        });
        fixture.dispatch({ effects: installLoraPresentation.of(prepared) });
        assert.equal(fixture.state.field(loraPresentation), prepared);
      } else if (mode === 'scene-errors') {
        const prepared = prepareSceneErrors(freshness.source, ['fixed']);
        fixture.dispatch({ effects: installSceneErrors.of(prepared) });
        assert.equal(fixture.state.field(sceneErrors), prepared);
      } else fixture.dispatch({ effects: invalidateCommandContext.of(null) });
      const prepared = prepareProjection(fixture.state, prepareNativeStructure(fixture.state));
      fixture.dispatch({ effects: installProjection.of(prepared) });
      const installed = fixture.state.field(richProjection);
      assert.equal(fixture.state.selection, selection);
      if (mode === 'context') {
        assert.notEqual(fixture.state.field(editorFreshness), freshness);
        assert.equal(fixture.state.field(heldProjection), null);
        assert.equal(installed.expanded.length, 0);
        assert.ok(first.style.get(heldTokenPointer(1), false, false).eq(selection));
      } else {
        assert.equal(fixture.state.field(editorFreshness), freshness);
        assert.equal(fixture.state.field(heldProjection), held);
        assert.ok(installed.expanded.length > 0);
        assert.equal(currentInputProjection(fixture.state), installed);
        assert.equal(currentInputProjection(fixture.state), installed);
        assert.deepEqual(
          [
            first.style.get(heldTokenPointer(1), false, false).main.anchor,
            first.style.get(heldTokenPointer(1), false, false).main.head,
          ],
          [19, 0],
        );
      }
      assert.equal(readExactSource(fixture.state), 'alpha <lora:real:1>, tail');
      assert.deepEqual([undoDepth(fixture.state), redoDepth(fixture.state)], [0, 0]);
    } finally {
      fixture.destroy();
    }
    assert.equal(fixture.document.active + fixture.window.active, 0);
  });
}

for (const source of ['alpha <lora:real:1>, tail', 'alpha bravo, tail']) {
  for (const mode of ['media', 'presentation', 'scene-errors'] as const) {
    test(`${mode} publication before initial native selection retains ${source.includes('<') ? 'token' : 'segment'} follow-up intent`, () => {
      const fixture = heldTokenFixture(source);
      try {
        const first = fixture.press(heldTokenPointer(8, 2));
        const freshness = fixture.state.field(editorFreshness);
        if (mode === 'media')
          fixture.dispatch({
            effects: installLoraMedia.of({ freshness, media: new Map() }),
          });
        else if (mode === 'presentation')
          fixture.dispatch({
            effects: installLoraPresentation.of(
              prepareLoraPresentation(fixture.state, {
                status: 'ready',
                resources: [],
              }),
            ),
          });
        else
          fixture.dispatch({
            effects: installSceneErrors.of(prepareSceneErrors(freshness.source, ['fixed'])),
          });
        fixture.select(first.style, first.event);
        fixture.document.phase('mouseup', new Event('mouseup'), false);
        const second = fixture.press(heldTokenPointer(8, 1));
        fixture.select(second.style, second.event);
        assert.deepEqual(
          [fixture.state.selection.main.anchor, fixture.state.selection.main.head],
          source.includes('<') ? [7, 11] : [6, 11],
        );
        assert.equal(readExactSource(fixture.state), source);
        assert.deepEqual([undoDepth(fixture.state), redoDepth(fixture.state)], [0, 0]);
      } finally {
        fixture.destroy();
      }
      assert.equal(fixture.document.active + fixture.window.active, 0);
    });
  }
}

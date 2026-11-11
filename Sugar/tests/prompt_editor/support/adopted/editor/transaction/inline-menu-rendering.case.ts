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
/** Actual prepared model targets preserve inline keyboard action endpoints independently of the general display menu. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, StateEffect } from '@codemirror/state';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { readLoraResources } from '../../../../../../frontend/sugarprompt/services/src/lora/validation.ts';
import { loraPageHost } from '../../../../../../frontend/sugarprompt/editor/src/lora-action-hosts.ts';
import {
  installLoraPresentation,
  prepareLoraPresentation,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  prepareContextMenu,
  contextMenuTargetAt,
} from '../../../../../../frontend/sugarprompt/editor/src/context-menu-preparation.ts';
import { projectionState } from './projection-state.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** Prepare an actual source-backed chip and page action without acquiring external metadata or a browser. */
function modelState(readOnly: boolean): EditorState {
  let state = projectionState('🙂\r\n<lora:real:0.50>, tail');
  state = state.update({
    effects: StateEffect.appendConfig.of([
      EditorState.readOnly.of(readOnly),
      loraPageHost.of({
        adapter: { open: async () => ({ status: 'opened' as const }) },
        observe: () => {},
        report: () => {},
      }),
    ]),
  }).state;
  const presentation = prepareLoraPresentation(state, {
    status: 'ready',
    resources: readLoraResources(
      [
        {
          name: 'real',
          resolution: 'found',
          model: {
            promptName: 'real',
            backendValue: 'real.safetensors',
            basename: 'real',
          },
          modelPageUrl: 'https://models.invalid/1',
          title: 'model',
          subtitle: null,
          triggerWords: [],
          thumbnail: { status: 'unavailable' },
        },
      ],
      ['real'],
    ),
  });
  state = state.update({
    effects: installLoraPresentation.of(presentation),
  }).state;
  return state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
}

for (const readOnly of [false, true])
  test(`inline model metadata retains the last keyboard action without a general rendering row, readOnly=${readOnly}`, () => {
    const state = modelState(readOnly),
      source = readExactSource(state),
      selection = state.selection,
      target = contextMenuTargetAt(state, 3);
    assert.ok(target?.inline);
    const menu = prepareContextMenu(state, target, 'local');
    assert.ok(menu);
    assert.equal(menu.kind, 'inline');
    assert.equal(menu.items.at(-1), menu.actions?.pageItem);
    assert.equal(
      menu.items.some((item) => item.kind === 'rich-rendering'),
      false,
    );
    assert.equal(readExactSource(state), source);
    assert.equal(state.selection, selection);
    assert.deepEqual([undoDepth(state), redoDepth(state)], [0, 0]);
  });

test('the general prompt menu retains one checked rendering control', () => {
  const state = modelState(false),
    target = contextMenuTargetAt(state, state.doc.length);
  assert.ok(target);
  assert.equal(target.inline, null);
  const menu = prepareContextMenu(state, target, 'local');
  assert.ok(menu);
  assert.equal(menu.items.at(-1), menu.renderingItem);
  assert.equal(menu.items.filter((item) => item.kind === 'rich-rendering').length, 1);
  assert.equal(menu.renderingItem.checked, true);
  assert.deepEqual([undoDepth(state), redoDepth(state)], [0, 0]);
});

for (const readOnly of [false, true])
  test(`aggregate rich inline caret uses general field scope and preserves rendering, readOnly=${readOnly}`, () => {
    const state = modelState(readOnly),
      target = contextMenuTargetAt(state, 3)!;
    assert.ok(target.inline);
    const local = prepareContextMenu(state, target, 'local')!,
      field = prepareContextMenu(state, target, 'field')!;
    assert.equal(local.items.at(-1)?.kind, 'page');
    assert.equal(field.surface, 'field');
    assert.equal(field.actions?.kind === 'inline', false);
    assert.equal(field.items.at(-1), field.renderingItem);
    assert.deepEqual(
      field.items.map((item) => item.kind),
      ['rich-rendering'],
    );
    assert.equal(
      field.items.some(
        (item) => item.kind === 'text' || item.kind === 'page' || item.kind === 'action',
      ),
      false,
    );
    assert.equal(field.text.selection.main.anchor, state.selection.main.anchor);
    assert.equal(readExactSource(state), '🙂\r\n<lora:real:0.50>, tail');
    assert.deepEqual([undoDepth(state), redoDepth(state)], [0, 0]);
  });

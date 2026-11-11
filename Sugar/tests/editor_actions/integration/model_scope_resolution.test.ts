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
/** Prove scope discovery follows native promoted contracts and prepared metadata. */
import { afterEach, expect, test } from '@jest/globals';
import { nativeModelFieldFixture } from '../../model_library/support/native_model_field.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import { editorModelScopes } from '../../../frontend/comfyui/ui/editor_actions/infrastructure/EditorModelScopeAdapter.js';
import { readNativeEditorCard } from '../../../frontend/comfyui/ui/editor_actions/infrastructure/NativeEditorCardAdapter.js';

afterEach(() => document.body.replaceChildren());

test('combines prepared Comfy tensor architecture with the active provider family', () => {
  const { first, second, snapshot, cards } = panelModels();
  const detected = {
    ...snapshot,
    items: [
      {
        ...first.item,
        architecture: { base: 'SDXL', comfyClass: 'SDXL', source: 'comfy_tensor_shapes' as const },
      },
      second.item,
    ],
  };
  expect(editorModelScopes(second.card, cards, detected).map((scope) => scope.key)).toEqual([
    'sdxl',
    'illustrious',
  ]);
  expect(
    editorModelScopes(second.card, cards, {
      ...detected,
      items: [{ ...detected.items[0]!, baseModel: '', title: '' }],
    }).map((scope) => scope.key),
  ).toEqual(['sdxl']);
});

/** Mount two model cards in separate sections of the same editor panel. */
function panelModels() {
  const panel = document.createElement('section');
  panel.className = 'sugarcubes-workflow-masonry';
  document.body.append(panel);
  const models = ['first.safetensors', 'second.safetensors'].map((value, index) => {
    const native = nativeModelFieldFixture();
    const mount = native.card.parentElement;
    if (!mount) throw new Error('Missing model mount');
    mount.className = 'sugarcubes-cube-face__native-card-mount sugarcubes-native-node-card';
    mount.dataset.masonrySection = `section-${String(index)}`;
    mount.dataset.scProjectionOrder = String(index);
    panel.append(mount);
    native.props.modelValue = value;
    native.props.widget.options.values = [value];
    const card = readNativeEditorCard(native.card);
    const item = modelPickerChoices([], 'checkpoints', [value])[0];
    if (!card || !item) throw new Error('Missing model fixture');
    return { native, card, item: { ...item, baseModel: index === 0 ? 'Illustrious' : 'FLUX' } };
  });
  const first = models[0],
    second = models[1];
  if (!first || !second) throw new Error('Missing panel models');
  const snapshot = { ...first.native.snapshot, items: models.map((model) => model.item) };
  return { panel, first, second, snapshot, cards: models.map((model) => model.card) };
}

test('uses the first panel model for every section regardless of clicked card or discovery order', () => {
  const { first, second, snapshot, cards } = panelModels();
  expect(
    editorModelScopes(second.card, [...cards].reverse(), snapshot).map((scope) => scope.key),
  ).toEqual(['illustrious']);
  expect(editorModelScopes(first.card, cards, snapshot).map((scope) => scope.key)).toEqual([
    'illustrious',
  ]);
});

test('follows projection reorder and selected model changes without using a stale family', () => {
  const { first, second, snapshot, cards } = panelModels();
  first.native.card.parentElement!.dataset.scProjectionOrder = '1';
  second.native.card.parentElement!.dataset.scProjectionOrder = '0';
  expect(editorModelScopes(first.card, cards, snapshot).map((scope) => scope.key)).toEqual([
    'flux',
  ]);
  second.native.props.modelValue = first.item.value;
  expect(editorModelScopes(first.card, cards, snapshot).map((scope) => scope.key)).toEqual([
    'illustrious',
  ]);
});

test('does not substitute another model when the active model metadata is unavailable', () => {
  const { second, snapshot, cards } = panelModels();
  expect(editorModelScopes(second.card, cards, { ...snapshot, items: [second.item] })).toEqual([]);
});

test('reads refreshed provider metadata and keeps independent Cube faces isolated', () => {
  const { first, second, snapshot, cards, panel } = panelModels();
  expect(
    editorModelScopes(second.card, cards, {
      ...snapshot,
      items: [{ ...first.item, baseModel: 'Other' }, second.item],
    }),
  ).toEqual([]);
  expect(editorModelScopes(second.card, cards, snapshot).map((scope) => scope.key)).toEqual([
    'illustrious',
  ]);
  panel.className = 'sugarcubes-cube-face';
  const other = document.createElement('div');
  other.className = 'sugarcubes-cube-face';
  document.body.append(other);
  other.append(second.native.card.parentElement!);
  expect(editorModelScopes(second.card, cards, snapshot).map((scope) => scope.key)).toEqual([
    'flux',
  ]);
});

test('keeps the active model when its combo is collapsed and uses current declared values', () => {
  const { first, second, snapshot, cards } = panelModels();
  const widget = {
    ...first.native.props.widget,
    type: 'combo',
    value: first.item.value,
  };
  first.native.nodeData.widgets = [widget];
  Object.assign(first.native.card.parentElement!, {
    _vnode: {
      type: { __name: 'LGraphNode' },
      component: { props: { nodeData: first.native.nodeData }, subTree: { el: first.native.card } },
    },
  });
  const collapsed = readNativeEditorCard(first.native.card);
  if (!collapsed) throw new Error('Missing collapsed model card');
  const current = [collapsed, ...cards.filter((card) => card !== first.card)];
  expect(editorModelScopes(second.card, current, snapshot).map((scope) => scope.key)).toEqual([
    'illustrious',
  ]);
  widget.value = second.item.value;
  expect(editorModelScopes(second.card, current, snapshot).map((scope) => scope.key)).toEqual([
    'flux',
  ]);
  first.native.card.parentElement!.remove();
  expect(editorModelScopes(second.card, current, snapshot).map((scope) => scope.key)).toEqual([
    'flux',
  ]);
});

test('matches provider metadata through normalized backend paths and stems', () => {
  const { first, second, snapshot, cards } = panelModels();
  first.native.props.modelValue = 'Folder\\FIRST.SAFETENSORS';
  expect(editorModelScopes(second.card, cards, snapshot).map((scope) => scope.key)).toEqual([
    'illustrious',
  ]);
});

test('resolves family scopes for aliased Cube checkpoint fields through the model adapter', () => {
  const native = nativeModelFieldFixture();
  native.card.parentElement?.classList.add(
    'sugarcubes-cube-face__native-card-mount',
    'sugarcubes-native-node-card',
  );
  native.nodeData.type = 'projected-subgraph';
  native.props.widget.name = 'Checkpoint';
  native.nodeData.widgets = [
    Object.assign(
      { ...native.props.widget },
      {
        sugarcubesNativeWidgetOrigins: [
          { classType: 'CheckpointLoaderSimple', inputName: 'ckpt_name' },
        ],
      },
    ),
  ];
  const item = modelPickerChoices([], 'checkpoints', ['Flux\\alpha.safetensors'])[0];
  if (!item) throw new Error('Missing fixture model');
  const snapshot = {
    ...native.snapshot,
    items: [{ ...item, baseModel: 'SDXL 1.0', title: 'Illustrious XL', tags: ['NoobAI'] }],
  };
  const card = readNativeEditorCard(native.card);
  if (!card) throw new Error('Missing native card');
  expect(editorModelScopes(card, [card], snapshot).map((scope) => scope.key)).toEqual([
    'sdxl',
    'illustrious',
    'noobai',
  ]);
  native.card.parentElement?.remove();
});

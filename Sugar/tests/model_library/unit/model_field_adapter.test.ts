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
/** Safeguard native dropdown values, custom widget boundaries, and catalog recognition. */
import { nativeModelFieldFixture } from '../support/native_model_field.js';
import { readComfyModelFields } from '../../../frontend/comfyui/ui/model_library/ComfyModelFieldAdapter.js';
import type { ModelCatalogSnapshot } from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';
import { recognizeModelField } from '../../../frontend/comfyui/ui/model_library/ModelFieldRecognition.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import { registerNativeWidgetMount } from '../../../frontend/comfyui/ui/surface/NativeWidgetMountBoundary.js';

afterEach(() => document.body.replaceChildren());

test('enhances registered relation choices through their own callback without duplicating native fields', () => {
  const native = nativeModelFieldFixture();
  const mount = document.createElement('div');
  mount.className = 'sugarcubes-native-widget';
  const frame = document.createElement('div');
  frame.setAttribute('data-capture-wheel', 'true');
  const trigger = document.createElement('button');
  trigger.dataset.testid = 'widget-select-default-trigger';
  frame.append(trigger);
  mount.append(frame);
  native.card.append(mount);
  const reference = 'link:["cube/peer","model_name"]';
  const values = ['Flux\\alpha.safetensors', reference, 'blocked.safetensors'];
  const props = {
    widget: {
      name: 'ckpt_name',
      label: 'ckpt_name link or value',
      options: {
        values,
        allowedValues: ['Flux\\alpha.safetensors', reference],
        getOptionLabel: (value: string) => (value === reference ? '🔗 Other Cube' : value),
      },
    },
    modelValue: 'Flux\\alpha.safetensors',
  };
  const writes: string[] = [];
  registerNativeWidgetMount(mount, {
    seed: null,
    vnode: {
      type: { __name: 'WidgetSelectDefault' },
      component: {
        props,
        emit: (_event: string, value: string) => {
          writes.push(value);
          props.modelValue = value;
        },
        proxy: { $watch: () => () => undefined },
        subTree: { el: frame },
      },
    },
  });
  const fields = readComfyModelFields(native.card, native.snapshot);
  expect(fields).toHaveLength(2);
  const binding = fields.find((field) => field.anchor === frame);
  expect(binding?.inputName).toBe('ckpt_name');
  expect(binding?.kind).toBe('checkpoints');
  binding?.writeValue(reference);
  binding?.writeValue('injected.safetensors');
  binding?.writeValue('blocked.safetensors');
  expect(writes).toEqual([reference]);
  expect(native.emit).not.toHaveBeenCalled();
  expect(modelPickerChoices([], 'checkpoints', values, binding?.optionLabel)[1]?.title).toBe(
    '🔗 Other Cube',
  );
  expect(modelPickerChoices([], 'checkpoints', values, binding?.optionLabel)[1]?.folder).toBe('');
});

test('uses promoted leaf contracts while retaining the native name and legal callback path', () => {
  const native = nativeModelFieldFixture();
  native.props.widget.name = 'Detector';
  native.props.widget.options.values = ['bbox/face_yolov8n.pt', 'None'];
  native.props.modelValue = 'bbox/face_yolov8n.pt';
  native.nodeData.widgets = [
    Object.assign(
      { ...native.props.widget },
      {
        sugarcubesNativeWidgetOrigins: [
          { classType: 'SimpleSyrup.LoadUltralyticsModel', inputName: 'model_name' },
        ],
      },
    ),
  ];
  expect(native.props.widget).not.toHaveProperty('sugarcubesNativeWidgetOrigins');
  const snapshot = {
    ...native.snapshot,
    fieldContracts: [
      {
        classType: 'SimpleSyrup.LoadUltralyticsModel',
        inputName: 'model_name',
        kind: 'ultralytics',
      },
    ],
  };
  const field = readComfyModelFields(native.card, snapshot)[0];
  expect(field?.kind).toBe('ultralytics');
  expect(field?.inputName).toBe('Detector');
  field?.writeValue('None');
  field?.writeValue('unlisted.pt');
  expect(native.emit).toHaveBeenCalledTimes(1);
  expect(native.emit).toHaveBeenCalledWith('update:modelValue', 'None');
  field?.subscribe(() => undefined)();
  expect(native.stop).toHaveBeenCalledTimes(1);
});

test.each([
  [],
  null,
  [{ classType: '', inputName: 'ckpt_name' }],
  [
    { classType: 'CheckpointLoaderSimple', inputName: 'ckpt_name' },
    { classType: 'Unknown', inputName: 'model_name' },
  ],
])('retains native ownership for unresolved or conflicting promoted origins: %j', (origins) => {
  const native = nativeModelFieldFixture();
  Object.assign(native.props.widget, { sugarcubesNativeWidgetOrigins: origins });
  expect(readComfyModelFields(native.card, native.snapshot)).toEqual([]);
  expect(native.emit).not.toHaveBeenCalled();
});

test('uses the existing native reactive write and watch path without coercing filenames', () => {
  const native = nativeModelFieldFixture();
  const field = readComfyModelFields(native.card, native.snapshot)[0];
  expect(field).toBeDefined();
  if (!field) throw new Error('Expected the declared native checkpoint field.');
  expect(field.readValue()).toBe('Flux\\alpha.safetensors');
  field.writeValue('None');
  expect(native.emit).toHaveBeenCalledWith('update:modelValue', 'None');
  expect(field.readValue()).toBe('None');
  field.writeValue('injected.safetensors');
  expect(native.emit).toHaveBeenCalledTimes(1);
  const release = field.subscribe(() => undefined);
  release();
  expect(native.stop).toHaveBeenCalledTimes(1);
});

test('does not replace a custom dropdown despite familiar model labels and choices', () => {
  const custom = nativeModelFieldFixture('MyCustomDropdown');
  expect(readComfyModelFields(custom.card, custom.snapshot)).toEqual([]);
  expect(custom.emit).not.toHaveBeenCalled();
});

test('reads fresh legal choices after verified acquisition instead of retaining the initial widget options', () => {
  const native = nativeModelFieldFixture();
  const field = readComfyModelFields(native.card, native.snapshot)[0];
  if (!field) throw new Error('Expected the declared native checkpoint field.');
  native.props.widget.options = { values: ['fresh.safetensors'] };
  expect(field.options).toEqual(['fresh.safetensors']);
  field.writeValue('Flux\\alpha.safetensors');
  expect(native.emit).not.toHaveBeenCalled();
  field.writeValue('fresh.safetensors');
  expect(native.emit).toHaveBeenCalledWith('update:modelValue', 'fresh.safetensors');
});

test.each([null, undefined])(
  'recognizes an empty native model field with %s as its unset host value',
  (unset) => {
    const native = nativeModelFieldFixture();
    native.props.modelValue = unset;
    native.props.widget.options.values = [];
    const field = readComfyModelFields(native.card, native.snapshot)[0];
    expect(field?.readValue()).toBe('');
    expect(native.emit).not.toHaveBeenCalled();
    native.props.widget.options.values = ['fresh.safetensors'];
    const refreshed = readComfyModelFields(native.card, native.snapshot)[0];
    expect(refreshed?.readValue()).toBe('');
    refreshed?.writeValue('fresh.safetensors');
    expect(native.emit).toHaveBeenCalledWith('update:modelValue', 'fresh.safetensors');
  },
);

test('defers enhancement while the native dropdown is open or owns focus', () => {
  const native = nativeModelFieldFixture();
  const field = readComfyModelFields(native.card, native.snapshot)[0];
  if (!field) throw new Error('Expected the native checkpoint field.');
  expect(field.enhancementReady()).toBe(true);
  native.trigger.setAttribute('aria-expanded', 'true');
  expect(field.enhancementReady()).toBe(false);
  native.trigger.setAttribute('aria-expanded', 'false');
  native.trigger.focus();
  expect(field.enhancementReady()).toBe(false);
  native.trigger.blur();
  expect(field.enhancementReady()).toBe(true);
});

test('retains ownership of ambiguous and ordinary dropdowns', () => {
  const options = ['shared.safetensors'];
  const items = [
    ...modelPickerChoices([], 'loras', options),
    ...modelPickerChoices([], 'checkpoints', options),
  ];
  const snapshot: ModelCatalogSnapshot = {
    revision: 0,
    items,
    scanning: false,
    fieldContracts: [],
  };
  expect(recognizeModelField(snapshot, 'Custom', 'model', options)).toBeNull();
  expect(
    recognizeModelField({ ...snapshot, items: items.slice(0, 1) }, 'Custom', 'model', options),
  ).toBe('loras');
  expect(recognizeModelField(snapshot, 'KSampler', 'sampler_name', ['euler'])).toBeNull();
});

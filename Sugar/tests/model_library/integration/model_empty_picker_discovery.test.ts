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
/** Prove discovery uses warm native choices and survives their verified publication refresh. */
import { nextTick } from 'vue';
import { jest } from '@jest/globals';
import { nativeModelFieldFixture } from '../support/native_model_field.js';
import { discoveryPorts, discoveryPlan } from '../support/model_discovery.js';
import { ModelFixtureResizeObserver } from '../support/resize_observer.js';
import { ModelPickerSessions } from '../../../frontend/comfyui/ui/model_library/ModelPickerSessions.js';
import { ModelEmptyPickerDiscovery } from '../../../frontend/comfyui/ui/model_library/ModelEmptyPickerDiscovery.js';
import { readComfyModelFields } from '../../../frontend/comfyui/ui/model_library/ComfyModelFieldAdapter.js';
import { ModelPickerHost } from '../../../frontend/comfyui/ui/model_library/ModelPickerHost.js';
import { ModelCatalogClient } from '../../../frontend/comfyui/ui/model_library/ModelCatalogClient.js';
import { ModelUpdateClient } from '../../../frontend/comfyui/ui/model_library/ModelUpdateClient.js';
import { ModelDetectorClient } from '../../../frontend/comfyui/ui/model_library/ModelDetectorClient.js';
import { compatibleModel } from '../support/model_compatibility.js';
import { mountModelPicker } from '../../../frontend/comfyui/ui/model_library/vue/ModelPickerVueRuntime.js';

/** Drain native-boundary and Vue update promises without provider or file operations. */
async function settle(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  await nextTick();
}

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
  jest.restoreAllMocks();
  jest.useRealTimers();
});

test('offers compatible discovery in a populated category and stops offering it after a matching model appears', async () => {
  const native = nativeModelFieldFixture();
  const binding = readComfyModelFields(native.card, native.snapshot)[0];
  if (!binding) throw new Error('Missing populated native field.');
  const ports = discoveryPorts();
  let snapshot = native.snapshot;
  const ready = jest.fn<(action: (() => void) | null) => void>();
  const owner = new ModelEmptyPickerDiscovery({
    sessions: new ModelPickerSessions(document, () => native.snapshot, { graph: native.graph }),
    document,
    ...ports,
    cubes: () => [],
    compatibility: () => ({
      architecture: 'SDXL',
      family: 'Illustrious',
      addon: false,
      label: 'Illustrious',
    }),
    snapshot: () => snapshot,
    reportError: jest.fn(),
  });
  const release = owner.bind(native.card, binding, ready);
  await settle();
  expect(ports.discovery.context).toHaveBeenCalledWith(
    'checkpoints',
    'Illustrious',
    expect.any(AbortSignal),
  );
  expect(ready).toHaveBeenCalledWith(expect.any(Function));
  ready.mock.calls[0]?.[0]?.();
  await settle();
  expect(document.querySelector('[role=dialog]')).not.toBeNull();
  expect(native.emit).not.toHaveBeenCalled();
  owner.close();
  release();
  snapshot = {
    ...snapshot,
    items: [compatibleModel('Flux\\alpha.safetensors', 'checkpoints', 'SDXL', 'Illustrious')],
  };
  ports.discovery.context.mockClear();
  owner.bind(native.card, binding, ready)();
  expect(ports.discovery.context).not.toHaveBeenCalled();
  expect(ports.discovery.acquire).not.toHaveBeenCalled();
});

test('replaces a scoped empty combo with modal discovery while preserving the selected host value', async () => {
  const native = nativeModelFieldFixture();
  const binding = readComfyModelFields(native.card, native.snapshot)[0];
  if (!binding) throw new Error('Missing native field.');
  const ports = discoveryPorts();
  const context = {
    architecture: 'SDXL',
    family: 'Illustrious',
    addon: false,
    label: 'Illustrious',
  };
  const wrong = compatibleModel(binding.readValue(), 'checkpoints', 'FLUX', 'Flux1');
  const root = document.createElement('div');
  native.card.append(root);
  const picker = mountModelPicker(
    root,
    binding,
    [wrong],
    null,
    null,
    null,
    null,
    null,
    () => context,
  );
  const owner = new ModelEmptyPickerDiscovery({
    sessions: new ModelPickerSessions(document, () => native.snapshot, { graph: native.graph }),
    document,
    ...ports,
    cubes: () => [],
    snapshot: () => ({ ...native.snapshot, items: [wrong] }),
    compatibility: () => context,
    reportError: jest.fn(),
  });
  const release = owner.bind(native.card, binding, picker.updateDiscovery);
  try {
    await settle();
    const find = root.querySelector<HTMLButtonElement>('.sc-model-find');
    expect(find?.textContent).toBe('Find models');
    expect(root.querySelector('[role=combobox]')).toBeNull();
    expect(document.querySelector('.sc-model-popup')).toBeNull();
    find?.click();
    await settle();
    expect(document.querySelector('[role=dialog]')).not.toBeNull();
    expect(document.querySelector('.sc-model-popup')).toBeNull();
    expect(binding.readValue()).toBe(wrong.value);
    expect(native.emit).not.toHaveBeenCalled();
    picker.update([compatibleModel('installed.safetensors', 'checkpoints', 'SDXL', 'Illustrious')]);
    picker.updateDiscovery(null);
    await nextTick();
    expect(root.querySelector('[role=combobox]')).not.toBeNull();
    expect(root.querySelector('.sc-model-find')).toBeNull();
  } finally {
    release();
    owner.close();
    picker.dispose();
  }
});

test('offers discovery only for warm, empty, server-supported native fields and cancels stale context replies', async () => {
  const native = nativeModelFieldFixture();
  native.props.modelValue = '';
  native.props.widget.options.values = [];
  const binding = readComfyModelFields(native.card, native.snapshot)[0];
  if (!binding) throw new Error('Missing empty native field.');
  const ports = discoveryPorts();
  let snapshot = { ...native.snapshot, scanning: true };
  const reportError = jest.fn();
  const ready = jest.fn<(action: (() => void) | null) => void>();
  const owner = new ModelEmptyPickerDiscovery({
    sessions: new ModelPickerSessions(document, () => native.snapshot, { graph: native.graph }),
    document,
    ...ports,
    cubes: () => [],
    compatibility: () => null,
    snapshot: () => snapshot,
    reportError,
  });
  owner.bind(native.card, binding, ready)();
  expect(ports.discovery.context).not.toHaveBeenCalled();
  snapshot = { ...native.snapshot, scanning: false };
  ports.discovery.context.mockResolvedValueOnce(null);
  const unsupported = owner.bind(native.card, binding, ready);
  await settle();
  expect(ready).not.toHaveBeenCalled();
  unsupported();
  ready.mockClear();
  const pending = owner.bind(native.card, binding, ready);
  pending();
  await settle();
  expect(ready).toHaveBeenCalledTimes(1);
  expect(ready).toHaveBeenCalledWith(null);
  expect(reportError).not.toHaveBeenCalled();
});

test('closes pending empty search when discovery becomes ready and restores a closed combo after installation', async () => {
  Object.defineProperty(window, 'ResizeObserver', {
    value: ModelFixtureResizeObserver,
    configurable: true,
  });
  const native = nativeModelFieldFixture();
  const binding = readComfyModelFields(native.card, native.snapshot)[0];
  if (!binding) throw new Error('Missing model field.');
  const root = document.createElement('div');
  native.card.append(root);
  const wrong = compatibleModel(binding.readValue(), 'checkpoints', 'FLUX', 'Flux1');
  const correct = compatibleModel('installed.safetensors', 'checkpoints', 'SDXL', 'Illustrious');
  const picker = mountModelPicker(root, binding, [wrong], null, null, null, null, null, () => ({
    architecture: 'SDXL',
    family: 'Illustrious',
    addon: false,
    label: 'Illustrious',
  }));
  try {
    root.querySelector<HTMLInputElement>('input')?.click();
    await nextTick();
    expect(document.querySelector('.sc-model-popup')).not.toBeNull();
    picker.updateDiscovery(() => undefined);
    await nextTick();
    expect(root.querySelector('.sc-model-find')).not.toBeNull();
    expect(document.querySelector('.sc-model-popup')).toBeNull();
    picker.update([correct]);
    picker.updateDiscovery(null);
    await nextTick();
    expect(root.querySelector('[role=combobox]')?.getAttribute('aria-expanded')).toBe('false');
    expect(document.querySelector('.sc-model-popup')).toBeNull();
    expect(binding.readValue()).toBe(wrong.value);
    expect(native.emit).not.toHaveBeenCalled();
  } finally {
    picker.dispose();
  }
});

test('offers discovery for an empty host list even when its persisted selection is stale', async () => {
  const native = nativeModelFieldFixture();
  native.props.widget.options.values = [];
  const binding = readComfyModelFields(native.card, native.snapshot)[0];
  if (!binding) throw new Error('Missing empty native field.');
  const ports = discoveryPorts();
  const ready = jest.fn<(action: (() => void) | null) => void>();
  const owner = new ModelEmptyPickerDiscovery({
    sessions: new ModelPickerSessions(document, () => native.snapshot, { graph: native.graph }),
    document,
    ...ports,
    cubes: () => [],
    compatibility: () => null,
    snapshot: () => native.snapshot,
    reportError: jest.fn(),
  });
  const release = owner.bind(native.card, binding, ready);
  await settle();
  expect(ready).toHaveBeenCalledWith(expect.any(Function));
  expect(native.props.modelValue).toBe('Flux\\alpha.safetensors');
  expect(native.emit).not.toHaveBeenCalled();
  release();
  owner.close();
});

test('remounting the field leaves its modal alive and resolves new legal choices before emitting', async () => {
  const native = nativeModelFieldFixture();
  native.props.modelValue = '';
  native.props.widget.options.values = [];
  const binding = readComfyModelFields(native.card, native.snapshot)[0];
  if (!binding) throw new Error('Missing native field.');
  const ports = discoveryPorts();
  const reportError = jest.fn();
  const owner = new ModelEmptyPickerDiscovery({
    sessions: new ModelPickerSessions(document, () => native.snapshot, { graph: native.graph }),
    document,
    ...ports,
    cubes: () => [],
    compatibility: () => null,
    snapshot: () => native.snapshot,
    reportError,
  });
  let open: (() => void) | null = null;
  const dispose = owner.bind(native.card, binding, (action) => {
    open = action;
  });
  await settle();
  const action: unknown = open;
  if (typeof action !== 'function') throw new Error('Missing eligible discovery action.');
  action();
  await settle();
  dispose();
  owner.reconcile();
  expect(document.querySelector('[role=dialog]')).not.toBeNull();
  ports.discovery.acquire.mockImplementation(async () => {
    native.props.modelValue = null;
    native.props.widget.options = { values: ['folder\\portrait.pth'] };
    return 'folder/portrait.pth';
  });
  document.querySelector<HTMLButtonElement>('[role=checkbox]')?.click();
  await nextTick();
  const download = [...document.querySelectorAll('button')].find(
    (button) => button.textContent === 'Download and use',
  );
  download?.click();
  await settle();
  expect(native.emit).toHaveBeenCalledWith('update:modelValue', 'folder\\portrait.pth');
  expect(document.querySelector('[role=dialog]')).toBeNull();
  expect(reportError).not.toHaveBeenCalled();
  owner.close();
});

test('removes discovery and cancels its modal when the originating node is removed', async () => {
  const native = nativeModelFieldFixture();
  native.props.modelValue = '';
  native.props.widget.options.values = [];
  const binding = readComfyModelFields(native.card, native.snapshot)[0];
  if (!binding) throw new Error('Missing native field.');
  const ports = discoveryPorts();
  const owner = new ModelEmptyPickerDiscovery({
    sessions: new ModelPickerSessions(document, () => native.snapshot, { graph: native.graph }),
    document,
    ...ports,
    cubes: () => [],
    compatibility: () => null,
    snapshot: () => native.snapshot,
    reportError: jest.fn(),
  });
  let open: (() => void) | null = null;
  const dispose = owner.bind(native.card, binding, (action) => {
    open = action;
  });
  await settle();
  const action: unknown = open;
  if (typeof action !== 'function') throw new Error('Missing discovery action.');
  action();
  await settle();
  native.card.remove();
  owner.reconcile();
  await settle();
  expect(document.querySelector('[role=dialog]')).toBeNull();
  expect(ports.discovery.acquire).not.toHaveBeenCalled();
  dispose();
  owner.close();
});

test('introduces Find models after cold-to-warm readiness and restores native fields on Nodes 1.0', async () => {
  jest.useFakeTimers();
  Object.defineProperty(window, 'ResizeObserver', {
    value: ModelFixtureResizeObserver,
    configurable: true,
  });
  const native = nativeModelFieldFixture();
  native.props.modelValue = '';
  native.props.widget.options.values = [];
  const ports = discoveryPorts();
  const values = new Map<string, unknown>([['Comfy.VueNodes.Enabled', true]]);
  const events = new EventTarget();
  const app = {
    ui: {
      settings: {
        addSetting: () => undefined,
        getSettingValue: (id: string) => values.get(id),
        addEventListener: events.addEventListener.bind(events),
        removeEventListener: events.removeEventListener.bind(events),
      },
    },
  };
  const fetch = async (): Promise<Response> => {
    throw new Error('Unexpected fixture transport');
  };
  const catalog = new ModelCatalogClient(fetch);
  jest
    .spyOn(catalog, 'load')
    .mockResolvedValueOnce({ ...native.snapshot, scanning: true })
    .mockResolvedValue(native.snapshot);
  jest.spyOn(catalog, 'watch').mockReturnValue(() => undefined);
  jest.spyOn(catalog, 'prioritize').mockResolvedValue(undefined);
  const host = new ModelPickerHost({
    app,
    document,
    catalog,
    feedback: jest.fn(),
    updates: new ModelUpdateClient(fetch),
    detectors: new ModelDetectorClient(fetch),
    outputs: {
      choices: () => [],
      activeChoice: () => null,
      image: async () => {
        throw new Error('No fixture image');
      },
    },
    discovery: { ...ports, cubes: () => [] },
    reportError: jest.fn(),
  });
  try {
    host.setup();
    await jest.advanceTimersByTimeAsync(20);
    expect(ports.discovery.context).not.toHaveBeenCalled();
    await jest.advanceTimersByTimeAsync(520);
    const find = document.querySelector<HTMLButtonElement>('.sc-model-find');
    expect(find?.textContent).toBe('Find models');
    ports.discovery.discover.mockResolvedValue(discoveryPlan(false, 'checkpoints'));
    find?.click();
    await jest.advanceTimersByTimeAsync(0);
    expect(document.querySelector('[role=dialog]')).not.toBeNull();
    values.set('Comfy.VueNodes.Enabled', false);
    events.dispatchEvent(new Event('Comfy.VueNodes.Enabled.change'));
    await jest.advanceTimersByTimeAsync(0);
    expect(document.querySelector('[role=dialog]')).toBeNull();
    expect(native.frame.hasAttribute('data-sc-model-native')).toBe(false);
    expect(native.props.modelValue).toBe('');
    expect(native.emit).not.toHaveBeenCalled();
  } finally {
    host.dispose();
  }
});

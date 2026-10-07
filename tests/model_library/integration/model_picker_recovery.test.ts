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
/** Prove background outages preserve editing and recover within the supported renderer lifetime. */
import { afterEach, beforeEach, expect, jest, test } from '@jest/globals';
import { ModelPickerHost } from '../../../frontend/comfyui/ui/model_library/ModelPickerHost.js';
import { ModelCatalogClient } from '../../../frontend/comfyui/ui/model_library/ModelCatalogClient.js';
import { ModelUpdateClient } from '../../../frontend/comfyui/ui/model_library/ModelUpdateClient.js';
import { ModelDetectorClient } from '../../../frontend/comfyui/ui/model_library/ModelDetectorClient.js';
import { modelBackgroundReporter } from '../../../frontend/comfyui/ui/model_library/ModelBackgroundDiagnostics.js';
import { nativeModelFieldFixture } from '../support/native_model_field.js';
import { ModelFixtureResizeObserver } from '../support/resize_observer.js';

const hosts: ModelPickerHost[] = [];

beforeEach(() => {
  jest.useFakeTimers();
  Object.defineProperty(window, 'ResizeObserver', {
    value: ModelFixtureResizeObserver,
    configurable: true,
  });
});
afterEach(() => {
  for (const host of hosts.splice(0)) host.dispose();
  document.body.replaceChildren();
  document.head.replaceChildren();
  jest.restoreAllMocks();
  jest.useRealTimers();
});

/** Keep the real host and Vue picker while controlling only the external catalog service. */
function fixture(paired = false) {
  const native = nativeModelFieldFixture('WidgetSelectDefault', paired);
  const values = new Map<string, unknown>([['Comfy.VueNodes.Enabled', true]]);
  const events = new EventTarget();
  const app = {
    graph: null as unknown,
    ui: {
      settings: {
        addSetting: () => undefined,
        getSettingValue: (id: string) => values.get(id),
        addEventListener: events.addEventListener.bind(events),
        removeEventListener: events.removeEventListener.bind(events),
      },
    },
  };
  const fetchApi = async (): Promise<Response> => {
    throw new Error('Unexpected fixture network request');
  };
  const catalog = new ModelCatalogClient(fetchApi);
  const load = jest.spyOn(catalog, 'load');
  const stop = jest.fn();
  jest.spyOn(catalog, 'watch').mockReturnValue(stop);
  jest.spyOn(catalog, 'prioritize').mockResolvedValue(undefined);
  const warn = jest.fn();
  const feedback = jest.fn();
  const host = new ModelPickerHost({
    app,
    document,
    catalog,
    feedback,
    updates: new ModelUpdateClient(fetchApi),
    detectors: new ModelDetectorClient(fetchApi),
    outputs: {
      choices: () => [],
      activeChoice: () => null,
      image: async () => {
        throw new Error('No fixture outputs');
      },
    },
    reportError: modelBackgroundReporter({ warn }, 'model picker'),
  });
  hosts.push(host);
  return { native, app, host, load, values, events, warn, feedback, stop };
}

/** Advance retry deadlines and paint one browser frame without replaying idle popup geometry frames. */
function recoveryClock(): (milliseconds: number) => Promise<void> {
  const frames = new Map<number, FrameRequestCallback>();
  let nextFrame = 0;
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    const id = ++nextFrame;
    frames.set(id, callback);
    return id;
  });
  jest.spyOn(window, 'cancelAnimationFrame').mockImplementation((id) => {
    frames.delete(id);
  });
  return async (milliseconds: number): Promise<void> => {
    await jest.advanceTimersByTimeAsync(milliseconds);
    for (const [id, callback] of [...frames]) {
      if (!frames.delete(id)) continue;
      callback(performance.now());
    }
    await jest.advanceTimersByTimeAsync(0);
  };
}

test('unrelated popup search and closure preserve the mounted model picker without card discovery', async () => {
  const proof = fixture();
  proof.load.mockResolvedValue(proof.native.snapshot);
  proof.host.setup();
  await jest.advanceTimersByTimeAsync(40);
  const mount = proof.native.card.querySelector('.sc-model-mount');
  const discovery = jest.spyOn(proof.native.card, 'querySelectorAll');
  const popup = document.createElement('div');
  popup.setAttribute('role', 'listbox');
  const search = document.createElement('input');
  popup.append(search);
  document.body.append(popup);
  await jest.advanceTimersByTimeAsync(20);
  search.dispatchEvent(new Event('input', { bubbles: true }));
  popup.remove();
  await jest.advanceTimersByTimeAsync(20);
  expect(discovery).not.toHaveBeenCalled();
  expect(proof.native.card.querySelector('.sc-model-mount')).toBe(mount);
});

test('contributes one Cube scope menu for paired checkpoint and VAE fields', async () => {
  const proof = fixture(true);
  const subgraph = {
    id: 'subgraph',
    name: 'Example',
    _nodes: [{ id: 'checkpoint' }],
    inputs: [],
    outputs: [],
  };
  const cube = {
    subgraph,
    properties: { sugarcubes_kind: 'cube', sugarcubes_cube: { instance_id: 'cube-1' } },
    pos: [0, 0],
    size: [400, 300],
    inputs: [],
    outputs: [],
    isSubgraphNode: () => true,
    connect: () => undefined,
    serialize: () => ({}),
  };
  proof.app.graph = { id: 'root', _nodes: [cube] };
  proof.native.card.classList.add('sugarcubes-cube-face');
  proof.native.card.dataset.cubeNodeLocator = 'subgraph:checkpoint';
  proof.load.mockResolvedValue(proof.native.snapshot);
  proof.host.setup();
  await jest.advanceTimersByTimeAsync(20);
  expect(proof.native.card.querySelectorAll('.sc-model-mount')).toHaveLength(2);
  expect(
    proof.host
      .contextContributions(proof.native.card)
      .flatMap((item) => item.entries)
      .filter((entry) => entry.label === 'Cube models'),
  ).toHaveLength(1);
});

test('follows Fluent preference without remounting, closing search, or writing a widget value', async () => {
  const proof = fixture();
  proof.load.mockResolvedValue(proof.native.snapshot);
  proof.host.setup();
  await jest.advanceTimersByTimeAsync(20);
  const mount = proof.native.card.querySelector<HTMLElement>('.sc-model-mount');
  const input = mount?.querySelector('input');
  expect(mount?.dataset.scModelAppearance).toBe('native');
  const field = mount?.querySelector<HTMLElement>('.sc-model-field');
  const arrow = mount?.querySelector<HTMLElement>('.sc-model-field-arrow');
  if (!field || !arrow || !input) throw new Error('Expected a mounted model combo');
  expect(getComputedStyle(field).height).toBe('24px');
  expect(getComputedStyle(field).borderRadius).toBe('6px');
  expect(getComputedStyle(arrow).width).toBe('24px');
  expect(getComputedStyle(input).paddingLeft).toBe('8px');
  input.click();
  await jest.advanceTimersByTimeAsync(0);
  const popup = document.querySelector<HTMLElement>('.sc-model-popup');
  expect(popup?.dataset.scModelAppearance).toBe('native');
  proof.events.dispatchEvent(new Event('SugarCubes.Appearance.Fluent.change'));
  proof.values.set('SugarCubes.Appearance.Fluent', true);
  await jest.advanceTimersByTimeAsync(20);
  expect(mount?.dataset.scModelAppearance).toBe('fluent');
  expect(popup?.dataset.scModelAppearance).toBe('fluent');
  expect(getComputedStyle(field).height).toBe('32px');
  expect(getComputedStyle(arrow).width).toBe('30px');
  proof.events.dispatchEvent(new Event('SugarCubes.Appearance.Fluent.change'));
  proof.values.set('SugarCubes.Appearance.Fluent', false);
  await jest.advanceTimersByTimeAsync(20);
  expect(mount?.dataset.scModelAppearance).toBe('native');
  expect(proof.native.card.querySelector('input')).toBe(input);
  expect(document.querySelector('.sc-model-popup')).toBe(popup);
  expect(proof.native.emit).not.toHaveBeenCalled();
});

test('leaves native choices editable after initial failure and retries without a modal or toast', async () => {
  const proof = fixture();
  const failure = new TypeError('Failed to fetch');
  proof.load.mockRejectedValueOnce(failure).mockResolvedValue(proof.native.snapshot);
  proof.host.setup();
  await jest.advanceTimersByTimeAsync(0);
  expect(proof.native.frame.hasAttribute('data-sc-model-native')).toBe(false);
  expect(proof.native.trigger.isConnected).toBe(true);
  expect(proof.warn).toHaveBeenCalledWith('SugarCubes model background request failed', {
    operation: 'model picker',
    error: failure,
  });
  expect(proof.feedback).not.toHaveBeenCalled();
  expect(document.querySelector('[role="dialog"],.sugarcubes-modal')).toBeNull();
  await jest.advanceTimersByTimeAsync(1_020);
  expect(proof.load).toHaveBeenCalledTimes(2);
  expect(proof.native.frame.hasAttribute('data-sc-model-native')).toBe(true);
  expect(proof.native.card.querySelector('input')?.value).toBe('alpha');
  expect(proof.native.props.modelValue).toBe('Flux\\alpha.safetensors');
  expect(proof.native.emit).not.toHaveBeenCalled();
});

test('keeps a mounted picker and its open choices during failed background refreshes', async () => {
  const advance = recoveryClock();
  const proof = fixture();
  proof.load
    .mockResolvedValueOnce({ ...proof.native.snapshot, scanning: true })
    .mockRejectedValueOnce(new TypeError('Failed to fetch'))
    .mockResolvedValue(proof.native.snapshot);
  proof.host.setup();
  await advance(20);
  const input = proof.native.card.querySelector('input');
  if (!input) throw new Error('Expected a mounted picker');
  input.click();
  await advance(0);
  expect(document.querySelectorAll('.sc-model-tile')).toHaveLength(2);
  await advance(500);
  expect(proof.native.card.querySelector('input')).toBe(input);
  expect(document.querySelectorAll('.sc-model-tile')).toHaveLength(2);
  expect(proof.feedback).not.toHaveBeenCalled();
  expect(document.querySelector('[role="dialog"],.sugarcubes-modal')).toBeNull();
  await advance(1_020);
  expect(proof.load).toHaveBeenCalledTimes(3);
  expect(proof.native.card.querySelector('input')).toBe(input);
  expect(proof.native.props.modelValue).toBe('Flux\\alpha.safetensors');
});

test('waits for an open native dropdown to close and release focus before enhancing after recovery', async () => {
  const proof = fixture();
  proof.load
    .mockRejectedValueOnce(new TypeError('Failed to fetch'))
    .mockResolvedValue(proof.native.snapshot);
  proof.host.setup();
  await jest.advanceTimersByTimeAsync(0);
  proof.native.trigger.setAttribute('aria-expanded', 'true');
  proof.native.trigger.focus();
  await jest.advanceTimersByTimeAsync(1_020);
  expect(proof.native.frame.hasAttribute('data-sc-model-native')).toBe(false);
  expect(proof.native.card.querySelector('.sc-model-mount')).toBeNull();
  proof.native.trigger.setAttribute('aria-expanded', 'false');
  await jest.advanceTimersByTimeAsync(20);
  expect(proof.native.frame.hasAttribute('data-sc-model-native')).toBe(false);
  proof.native.trigger.blur();
  await jest.advanceTimersByTimeAsync(20);
  expect(proof.native.card.querySelectorAll('.sc-model-mount')).toHaveLength(1);
  expect(proof.native.props.modelValue).toBe('Flux\\alpha.safetensors');
  expect(proof.native.emit).not.toHaveBeenCalled();
  expect(proof.feedback).not.toHaveBeenCalled();
});

test('does not enhance a deferred native field after disposal', async () => {
  const proof = fixture();
  proof.native.trigger.setAttribute('aria-expanded', 'true');
  proof.load.mockResolvedValue(proof.native.snapshot);
  proof.host.setup();
  await jest.advanceTimersByTimeAsync(20);
  expect(proof.native.card.querySelector('.sc-model-mount')).toBeNull();
  proof.host.dispose();
  proof.native.trigger.setAttribute('aria-expanded', 'false');
  proof.native.trigger.dispatchEvent(new FocusEvent('focusout', { bubbles: true }));
  await jest.advanceTimersByTimeAsync(20);
  expect(proof.native.card.querySelector('.sc-model-mount')).toBeNull();
});

test('pauses a cached page and remounts its exact legal model value on restoration', async () => {
  const proof = fixture();
  proof.load.mockResolvedValue(proof.native.snapshot);
  proof.host.setup();
  await jest.advanceTimersByTimeAsync(20);
  expect(proof.native.card.querySelectorAll('.sc-model-mount')).toHaveLength(1);
  window.dispatchEvent(new PageTransitionEvent('pagehide', { persisted: true }));
  expect(proof.native.frame.hasAttribute('data-sc-model-native')).toBe(false);
  expect(proof.native.card.querySelector('.sc-model-mount')).toBeNull();
  expect(proof.stop).toHaveBeenCalledTimes(1);
  proof.events.dispatchEvent(new Event('Comfy.VueNodes.Enabled.change'));
  await jest.advanceTimersByTimeAsync(20);
  expect(proof.load).toHaveBeenCalledTimes(1);
  window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true }));
  await jest.advanceTimersByTimeAsync(20);
  expect(proof.load).toHaveBeenCalledTimes(2);
  expect(proof.native.card.querySelectorAll('.sc-model-mount')).toHaveLength(1);
  expect(proof.native.props.modelValue).toBe('Flux\\alpha.safetensors');
  expect(proof.native.emit).not.toHaveBeenCalled();
  proof.host.dispose();
  window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true }));
  await jest.advanceTimersByTimeAsync(20);
  expect(proof.native.card.querySelector('.sc-model-mount')).toBeNull();
  expect(proof.load).toHaveBeenCalledTimes(2);
});

test.each(['disabled', 'nodes1'] as const)(
  'keeps the native model control on cached-page restoration when %s',
  async (state) => {
    const proof = fixture();
    proof.load.mockResolvedValue(proof.native.snapshot);
    proof.host.setup();
    await jest.advanceTimersByTimeAsync(20);
    window.dispatchEvent(new PageTransitionEvent('pagehide', { persisted: true }));
    if (state === 'disabled') proof.host.setEnabled(false);
    else {
      proof.values.set('Comfy.VueNodes.Enabled', false);
      proof.events.dispatchEvent(new Event('Comfy.VueNodes.Enabled.change'));
    }
    window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true }));
    await jest.advanceTimersByTimeAsync(20);
    expect(proof.native.frame.hasAttribute('data-sc-model-native')).toBe(false);
    expect(proof.native.card.querySelector('.sc-model-mount')).toBeNull();
    expect(proof.load).toHaveBeenCalledTimes(1);
    expect(proof.native.props.modelValue).toBe('Flux\\alpha.safetensors');
    expect(proof.native.emit).not.toHaveBeenCalled();
  },
);

test.each(['disable', 'dispose', 'renderer'] as const)(
  'cancels pending recovery on %s',
  async (action) => {
    const proof = fixture();
    proof.load.mockRejectedValue(new TypeError('Failed to fetch'));
    proof.host.setup();
    await jest.advanceTimersByTimeAsync(0);
    if (action === 'disable') proof.host.setEnabled(false);
    else if (action === 'dispose') proof.host.dispose();
    else {
      proof.values.set('Comfy.VueNodes.Enabled', false);
      proof.events.dispatchEvent(new Event('Comfy.VueNodes.Enabled.change'));
    }
    await jest.advanceTimersByTimeAsync(60_000);
    expect(proof.load).toHaveBeenCalledTimes(1);
    expect(proof.native.trigger.isConnected).toBe(true);
    expect(document.getElementById('sugarcubes-model-picker')).toBeNull();
  },
);

test('bounds retry delay and reports only once per uninterrupted outage', async () => {
  const proof = fixture();
  proof.load.mockRejectedValue(new TypeError('Failed to fetch'));
  proof.host.setup();
  await jest.advanceTimersByTimeAsync(61_000);
  expect(proof.load).toHaveBeenCalledTimes(7);
  expect(proof.warn).toHaveBeenCalledTimes(1);
  await jest.advanceTimersByTimeAsync(30_000);
  expect(proof.load).toHaveBeenCalledTimes(8);
});

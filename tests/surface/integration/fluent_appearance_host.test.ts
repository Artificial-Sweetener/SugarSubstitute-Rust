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
/** Prove reversible preference, renderer suspension, and native control preservation. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  FluentAppearanceHost,
  FLUENT_APPEARANCE_SETTING,
} from '../../../frontend/comfyui/ui/surface/appearance/FluentAppearanceHost.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';
import { NativeFluentCardAdapter } from '../../../frontend/comfyui/ui/surface/appearance/NativeFluentCardAdapter.js';
import {
  NativeCardPresentationOwner,
  prepareNativeCardPresentation,
} from '../../../frontend/comfyui/ui/surface/NativeCardPresentationOwner.js';
import { prepareNativePresentation } from '../../../frontend/comfyui/ui/surface/NativePresentationReady.js';

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
  jest.restoreAllMocks();
});

test('finishes requested native presentation before its first animation frame without replacing controls', () => {
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation(() => 1);
  jest.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => undefined);
  const app = {
    ui: {
      settings: {
        addSetting: () => undefined,
        getSettingValue: (key: string) => key === 'Comfy.VueNodes.Enabled',
      },
    },
  };
  const host = new FluentAppearanceHost({ document, app, reportError: jest.fn() });
  host.setup();
  host.setEnabled(true);
  const surface = document.createElement('section');
  surface.className = 'sugarcubes-workflow-masonry';
  const card = document.createElement('div');
  card.className = 'lg-node';
  const input = document.createElement('input');
  card.append(input);
  surface.append(card);
  document.body.append(surface);
  expect(card.hasAttribute('data-sc-fluent-node')).toBe(false);
  prepareNativePresentation(surface);
  expect(card.hasAttribute('data-sc-fluent-node')).toBe(true);
  expect(surface.hasAttribute('data-sc-fluent-surface')).toBe(true);
  expect(surface.hasAttribute('data-fluent-theme')).toBe(false);
  expect(card.querySelector('input')).toBe(input);
  host.dispose();
  prepareNativePresentation(surface);
  expect(card.hasAttribute('data-sc-fluent-node')).toBe(false);
});

test('initial styling synchronizes native sockets while popup activity leaves cards and geometry untouched', async () => {
  const card = document.createElement('div');
  card.className = 'lg-node';
  card.dataset.nodeId = '4';
  document.body.append(card);
  const app = {
    ui: {
      settings: {
        addSetting: () => undefined,
        getSettingValue: (key: string) => key === 'Comfy.VueNodes.Enabled',
      },
    },
  };
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((frame) => {
    frames.push(frame);
    return frames.length;
  });
  const synchronize = jest.fn<() => void>();
  const host = new FluentAppearanceHost({
    document,
    app,
    reportError: jest.fn(),
    getSlotLayoutSync: async () => synchronize,
  });
  host.setup();
  host.setEnabled(true);
  for (let index = 0; index < 3; index++) {
    await Promise.resolve();
    frames.splice(0).forEach((frame) => frame(0));
  }
  const annotate = jest.spyOn(NativeFluentCardAdapter.prototype, 'annotate');
  expect(synchronize).toHaveBeenCalledTimes(1);
  const popup = document.createElement('div');
  popup.setAttribute('role', 'listbox');
  const search = document.createElement('input');
  popup.append(search);
  document.body.append(popup);
  for (let index = 0; index < 3; index++) {
    await Promise.resolve();
    frames.splice(0).forEach((frame) => frame(0));
  }
  search.dispatchEvent(new Event('input', { bubbles: true }));
  popup.remove();
  for (let index = 0; index < 3; index++) {
    await Promise.resolve();
    frames.splice(0).forEach((frame) => frame(0));
  }
  expect(annotate).not.toHaveBeenCalled();
  expect(synchronize).toHaveBeenCalledTimes(1);
  host.setEnabled(false);
  frames.splice(0).forEach((frame) => frame(0));
  await Promise.resolve();
  expect(synchronize).toHaveBeenCalledTimes(2);
  host.dispose();
});

test('keeps controls and explicit custom styling intact while restoring the current host palette', async () => {
  const root = document.createElement('div');
  root.className = 'lg-node';
  root.style.position = 'relative';
  root.style.overflow = 'auto';
  const custom = document.createElement('input');
  custom.style.backgroundColor = 'rgb(12, 34, 56)';
  custom.value = '<img src=x>';
  root.append(custom);
  document.body.append(root);
  const values = new Map<string, unknown>([['Comfy.VueNodes.Enabled', true]]);
  const events = new EventTarget();
  let registration: unknown;
  const app = {
    ui: {
      settings: {
        addSetting: (setting: unknown) => {
          registration = setting;
        },
        getSettingValue: (id: string) => values.get(id),
        addEventListener: events.addEventListener.bind(events),
        removeEventListener: events.removeEventListener.bind(events),
      },
    },
  };
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback);
    return frames.length;
  });
  const errors = jest.fn();
  const host = new FluentAppearanceHost({ document, app, reportError: errors });
  host.setup();
  expect(isRecord(registration) && registration.id).toBe(FLUENT_APPEARANCE_SETTING);
  expect(root.hasAttribute('data-sc-fluent-node')).toBe(false);
  custom.focus();
  host.setEnabled(true);
  expect(document.querySelector('.sc-fluent-scrollbar-layer')).toBeNull();
  frames.splice(0).forEach((frame) => frame(0));
  expect(root.querySelector('.sc-fluent-scrollbar-layer')).not.toBeNull();
  expect(document.activeElement).toBe(custom);
  expect(custom.style.backgroundColor).toBe('rgb(12, 34, 56)');
  expect(root.hasAttribute('data-sc-fluent-node')).toBe(true);
  expect(custom.hasAttribute('data-sc-fluent-control')).toBe(false);
  document.documentElement.style.colorScheme = 'light';
  events.dispatchEvent(new Event('Comfy.ColorPalette.change'));
  await Promise.resolve();
  expect(document.getElementById('sugarcubes-fluent-appearance')?.textContent).toContain(
    '--sc-card-fill:#fbfbfb',
  );
  document.documentElement.style.colorScheme = 'dark';
  events.dispatchEvent(new Event('Comfy.ColorPalette.change'));
  await Promise.resolve();
  expect(document.getElementById('sugarcubes-fluent-appearance')?.textContent).toContain(
    '--sc-card-fill:#2b2b2b',
  );
  root.style.setProperty('--node-component-surface', '#fedcba');
  values.set('Comfy.VueNodes.Enabled', false);
  events.dispatchEvent(new Event('Comfy.VueNodes.Enabled.change'));
  await Promise.resolve();
  expect(document.querySelector('.sc-fluent-scrollbar-layer')).toBeNull();
  expect(root.hasAttribute('data-sc-fluent-node')).toBe(false);
  values.set('Comfy.VueNodes.Enabled', true);
  events.dispatchEvent(new Event('Comfy.VueNodes.Enabled.change'));
  await Promise.resolve();
  frames.splice(0).forEach((frame) => frame(0));
  expect(root.hasAttribute('data-sc-fluent-node')).toBe(true);
  host.setEnabled(false);
  expect(document.querySelector('.sc-fluent-scrollbar-layer')).toBeNull();
  expect(root.style.getPropertyValue('--node-component-surface')).toBe('#fedcba');
  expect(custom.value).toBe('<img src=x>');
  expect(root.querySelector('img')).toBeNull();
  expect(document.activeElement).toBe(custom);
  expect(errors).not.toHaveBeenCalled();
  host.dispose();
  document.documentElement.style.removeProperty('color-scheme');
});

/** Model native Vue roots without substituting the appearance observer or text owner. */
function nativeLabelVNode(
  name: string,
  element: HTMLElement,
  children: UnknownRecord[],
  props: UnknownRecord = {},
): UnknownRecord {
  return { type: { __name: name }, component: { props, subTree: { el: element, children } } };
}

test('formats projected card labels and restores source text across presentation and renderer changes', async () => {
  const masonry = document.createElement('section');
  masonry.className = 'sugarcubes-cube-face__node-card';
  const mount = document.createElement('div');
  mount.className = 'sugarcubes-native-node-card';
  const root = document.createElement('div');
  root.className = 'lg-node';
  const header = document.createElement('div');
  const title = document.createElement('div');
  title.dataset.testid = 'node-title';
  const editable = document.createElement('div');
  editable.className = 'editable-text';
  const text = document.createElement('span');
  text.textContent = 'mahiro CFG';
  editable.append(text);
  title.append(editable);
  header.append(title);
  const fields = document.createElement('div');
  const layout = document.createElement('div');
  const label = document.createElement('div');
  label.dataset.testid = 'widget-layout-field-label';
  label.textContent = 'sampler_name';
  layout.append(label);
  fields.append(layout);
  root.append(header, fields);
  mount.append(root);
  masonry.append(mount);
  document.body.append(masonry);
  const nodeData = { title: 'mahiro CFG', widgets: {} };
  Object.assign(mount, {
    _vnode: nativeLabelVNode(
      'LGraphNode',
      root,
      [
        nativeLabelVNode('NodeHeader', header, []),
        nativeLabelVNode('NodeWidgets', fields, [
          nativeLabelVNode(
            'WidgetSelect',
            layout,
            [nativeLabelVNode('WidgetLayoutField', layout, [])],
            { widget: { name: 'sampler_name' } },
          ),
        ]),
      ],
      { nodeData },
    ),
  });
  const events = new EventTarget();
  let vue = true;
  const app = {
    ui: {
      settings: {
        addSetting: () => undefined,
        getSettingValue: (id: string) => id === 'Comfy.VueNodes.Enabled' && vue,
        addEventListener: events.addEventListener.bind(events),
        removeEventListener: events.removeEventListener.bind(events),
      },
    },
  };
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback);
    return frames.length;
  });
  const errors = jest.fn();
  const host = new FluentAppearanceHost({ document, app, reportError: errors });
  host.setup();
  host.setEnabled(true);
  frames.splice(0).forEach((frame) => frame(0));
  expect(text.textContent).toBe('Mahiro CFG');
  expect(label.textContent).toBe('Sampler');
  expect(nodeData.title).toBe('mahiro CFG');
  await Promise.resolve();
  frames.splice(0).forEach((frame) => frame(0));
  await Promise.resolve();
  frames.splice(0).forEach((frame) => frame(0));
  await Promise.resolve();
  expect(frames).toHaveLength(0);
  if (!(text.firstChild instanceof Text)) throw new Error('Native title text missing');
  text.firstChild.data = 'the_lord_of_the_rings';
  await Promise.resolve();
  frames.splice(0).forEach((frame) => frame(0));
  expect(text.textContent).toBe('The Lord of the Rings');
  const replacement = document.createTextNode('cfg_scale');
  label.replaceChildren(replacement);
  await Promise.resolve();
  frames.splice(0).forEach((frame) => frame(0));
  expect(label.textContent).toBe('CFG Scale');
  document.body.append(mount);
  await Promise.resolve();
  frames.splice(0).forEach((frame) => frame(0));
  expect(text.textContent).toBe('the_lord_of_the_rings');
  expect(label.textContent).toBe('cfg_scale');
  replacement.data = 'strength_model';
  await Promise.resolve();
  frames.splice(0).forEach((frame) => frame(0));
  expect(label.textContent).toBe('strength_model');
  masonry.append(mount);
  await Promise.resolve();
  frames.splice(0).forEach((frame) => frame(0));
  expect(text.textContent).toBe('The Lord of the Rings');
  expect(label.textContent).toBe('Strength Model');
  vue = false;
  events.dispatchEvent(new Event('Comfy.VueNodes.Enabled.change'));
  await Promise.resolve();
  expect(text.textContent).toBe('the_lord_of_the_rings');
  expect(label.textContent).toBe('strength_model');
  vue = true;
  events.dispatchEvent(new Event('Comfy.VueNodes.Enabled.change'));
  await Promise.resolve();
  frames.splice(0).forEach((frame) => frame(0));
  expect(label.textContent).toBe('Strength Model');
  host.setEnabled(false);
  expect(label.textContent).toBe('strength_model');
  expect(errors).not.toHaveBeenCalled();
  host.dispose();
});

test('retains styled borrowed controls while still reconciling actual native content changes', async () => {
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback);
    return frames.length;
  });
  const app = {
    ui: {
      settings: {
        addSetting() {},
        getSettingValue: (key: string) => key === 'Comfy.VueNodes.Enabled',
      },
    },
  };
  const host = new FluentAppearanceHost({ document, app, reportError: jest.fn() });
  host.setup();
  host.setEnabled(true);
  const source = document.createElement('section');
  source.className = 'sugarcubes-workflow-masonry';
  const cell = document.createElement('div');
  cell.className = 'sugarcubes-cube-face__node-card';
  const root = document.createElement('div');
  root.className = 'lg-node';
  root.append(document.createElement('input'));
  cell.append(root);
  source.append(cell);
  document.body.append(source);
  prepareNativePresentation(source);
  const annotate = jest.spyOn(NativeFluentCardAdapter.prototype, 'annotate');
  const owner = new NativeCardPresentationOwner(() => undefined);
  owner.reset([cell]);
  const transfer = prepareNativeCardPresentation(cell, () => undefined)!;
  const destination = document.createElement('section');
  destination.className = 'sugarcubes-workflow-masonry';
  document.body.append(destination);
  transfer.move(destination);
  prepareNativePresentation(destination);
  await Promise.resolve();
  frames.splice(0).forEach((frame) => frame(0));
  expect(annotate).not.toHaveBeenCalled();
  root.append(document.createElement('textarea'));
  await Promise.resolve();
  frames.splice(0).forEach((frame) => frame(0));
  expect(annotate).toHaveBeenCalledWith(root);
  transfer.restore();
  owner.dispose();
  host.dispose();
});

test('owns one stable graph token scope across native card and masonry presentation', () => {
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation(() => 1);
  const graph = document.createElement('div');
  graph.id = 'graph-canvas-container';
  const card = document.createElement('div');
  card.className = 'lg-node';
  const masonry = document.createElement('section');
  masonry.className = 'sugarcubes-workflow-masonry';
  graph.append(card, masonry);
  document.body.append(graph);
  const app = {
    ui: {
      settings: {
        addSetting() {},
        getSettingValue: (key: string) => key === 'Comfy.VueNodes.Enabled',
      },
    },
  };
  const host = new FluentAppearanceHost({ document, app, reportError: jest.fn() });
  host.setup();
  host.setEnabled(true);
  prepareNativePresentation(masonry);
  expect([...document.querySelectorAll('[data-sc-fluent-theme-root]')]).toEqual([graph]);
  expect(document.getElementById('sugarcubes-fluent-appearance')?.textContent).toContain(
    '[data-sc-fluent-theme-root]',
  );
  expect(card.hasAttribute('data-sc-fluent-node')).toBe(true);
  masonry.append(card);
  prepareNativePresentation(masonry);
  expect([...document.querySelectorAll('[data-sc-fluent-theme-root]')]).toEqual([graph]);
  host.setEnabled(false);
  expect(document.querySelector('[data-sc-fluent-theme-root]')).toBeNull();
  host.dispose();
});

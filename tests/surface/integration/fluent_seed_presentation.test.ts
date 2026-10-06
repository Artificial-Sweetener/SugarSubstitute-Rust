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
/** Prove shared seed and numeric presentation through real appearance discovery and lifecycle. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { FluentAppearanceHost } from '../../../frontend/comfyui/ui/surface/appearance/FluentAppearanceHost.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
  jest.restoreAllMocks();
});

/** Represent host Vue boundaries while preserving the actual SugarCubes owners under test. */
function vnode(
  name: string,
  el: HTMLElement,
  children: UnknownRecord[],
  props: UnknownRecord = {},
): UnknownRecord {
  return { type: { __name: name }, component: { props, subTree: { el, children } } };
}

test.each(['native', 'cube', 'masonry'])(
  'presents seed controls in %s and restores native UI across theme/renderer changes',
  async (view) => {
    const surface = document.createElement('section');
    if (view === 'masonry') surface.className = 'sugarcubes-workflow-masonry';
    const mount = document.createElement('div');
    mount.className = 'sugarcubes-native-node-card';
    if (view !== 'native') surface.classList.add('sugarcubes-cube-face__node-card');
    const root = document.createElement('article');
    root.className = 'lg-node';
    const fields = document.createElement('div');
    const row = document.createElement('div');
    const layout = document.createElement('div');
    const label = document.createElement('span');
    label.textContent = 'seed';
    label.dataset.testid = 'widget-layout-field-label';
    const container = document.createElement('div');
    const spinner = document.createElement('div');
    const input = document.createElement('input');
    input.setAttribute('role', 'spinbutton');
    input.value = '709312264123456';
    const increment = document.createElement('button');
    increment.dataset.testid = 'increment';
    const decrement = document.createElement('button');
    decrement.dataset.testid = 'decrement';
    const nativeMode = document.createElement('button');
    nativeMode.dataset.testid = 'value-control';
    spinner.append(input, increment, decrement, nativeMode);
    container.append(spinner);
    layout.append(label, container);
    row.append(layout);
    fields.append(row);
    root.append(fields);
    mount.append(root);
    surface.append(mount);
    document.body.append(surface);
    const modeCallback = jest.fn();
    const control = {
      name: 'control_after_generate',
      type: 'combo',
      value: 'randomize',
      callback: modeCallback,
      options: { values: ['fixed', 'increment', 'decrement', 'randomize'], canvasOnly: true },
    };
    const widget = {
      name: 'seed',
      type: 'number',
      value: 709312264123456,
      linkedWidgets: [control],
    };
    const props = { modelValue: widget.value, step: 1 };
    const numeric = vnode('ScrubableNumberInput', spinner, [], props);
    const stop = jest.fn();
    Object.assign(numeric.component ?? {}, {
      emit: (event: string, value: number) => {
        if (event === 'update:modelValue') {
          widget.value = value;
          props.modelValue = value;
        }
      },
      proxy: { $watch: () => stop },
    });
    const editor = vnode(
      'WidgetInputNumber',
      layout,
      [vnode('WidgetLayoutField', layout, [numeric])],
      {
        widget: {
          name: 'seed',
          nodeLocatorId: 'root:1',
          options: { precision: 0, min: 0, max: 1e15 },
        },
      },
    );
    Object.assign(mount, {
      _vnode: vnode('LGraphNode', root, [vnode('NodeWidgets', fields, [editor])]),
    });
    const frames: FrameRequestCallback[] = [];
    jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
    let vue = true;
    const events = new EventTarget();
    const app = {
      graph: { id: 'root', _nodes: [{ id: 1, widgets: [widget, control] }] },
      ui: {
        settings: {
          addSetting: () => undefined,
          getSettingValue: (key: string) => (key === 'Comfy.VueNodes.Enabled' ? vue : false),
          addEventListener: events.addEventListener.bind(events),
          removeEventListener: events.removeEventListener.bind(events),
        },
      },
    };
    const errors = jest.fn();
    const host = new FluentAppearanceHost({ document, app, reportError: errors });
    host.setup();
    host.setEnabled(true);
    frames.splice(0).forEach((frame) => frame(0));
    const toggle = root.querySelector<HTMLButtonElement>('[data-sc-seed-toggle]');
    const arrow = root.querySelector<HTMLButtonElement>('[data-sc-seed-menu]');
    if (!toggle || !arrow) throw new Error('Seed UI was not discovered');
    expect(getComputedStyle(input).paddingLeft).toBe('8px');
    expect(getComputedStyle(input).paddingRight).toBe('8px');
    expect(getComputedStyle(input).textAlign).toBe('left');
    if (view !== 'native') {
      expect(getComputedStyle(layout).display).toBe('flex');
      expect(getComputedStyle(container).flexShrink).toBe('0');
      expect(getComputedStyle(spinner).width).toBe('calc(20ch + 64px)');
    } else expect(getComputedStyle(spinner).width).toBe('100%');
    expect(getComputedStyle(increment).display).toBe('none');
    expect(getComputedStyle(nativeMode).display).toBe('none');
    toggle.click();
    expect(control.value).toBe('fixed');
    arrow.click();
    document.querySelector<HTMLButtonElement>('[data-sc-seed-option="decrement"]')?.click();
    expect(control.value).toBe('decrement');
    expect(modeCallback).toHaveBeenCalledWith('decrement');
    expect(widget.value).toBe(709312264123456);
    expect(input.value).toBe('709312264123456');
    vue = false;
    events.dispatchEvent(new Event('Comfy.VueNodes.Enabled.change'));
    await Promise.resolve();
    expect(root.querySelector('[data-sc-seed-split]')).toBeNull();
    expect(input.hasAttribute('data-sc-number-input')).toBe(false);
    expect(getComputedStyle(nativeMode).display).not.toBe('none');
    expect(control.value).toBe('decrement');
    vue = true;
    events.dispatchEvent(new Event('Comfy.VueNodes.Enabled.change'));
    await Promise.resolve();
    frames.splice(0).forEach((frame) => frame(0));
    expect(root.querySelector('[data-sc-seed-toggle]')?.getAttribute('data-sc-seed-mode')).toBe(
      'decrement',
    );
    host.setEnabled(false);
    expect(root.querySelector('[data-sc-seed-split]')).toBeNull();
    expect(input.isConnected).toBe(true);
    expect(control.value).toBe('decrement');
    expect(stop).toHaveBeenCalledTimes(2);
    expect(errors).not.toHaveBeenCalled();
    host.dispose();
  },
);

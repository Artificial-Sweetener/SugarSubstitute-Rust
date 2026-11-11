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
/** Preserve shared seed controls across native graph renderer changes. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { AppearanceHost } from '../../../frontend/comfyui/ui/surface/appearance/AppearanceHost.js';
import { VISUAL_STYLE_SETTING } from '../../../frontend/comfyui/ui/surface/appearance/VisualStylePreference.js';
import { prepareNativePresentation } from '../../../frontend/comfyui/ui/surface/NativePresentationReady.js';
import { registerNativeWidgetMount } from '../../../frontend/comfyui/ui/surface/NativeWidgetMountBoundary.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';

/** Supply Comfy's Vue numeric boundary while retaining real owned DOM and appearance behavior. */
function seedWidget() {
  const root = document.createElement('div');
  root.className = 'sugarcubes-native-widget';
  const layout = document.createElement('div');
  const label = document.createElement('label');
  label.dataset.testid = 'widget-layout-field-label';
  label.textContent = 'Seed';
  const container = document.createElement('div');
  const spinner = document.createElement('div');
  const input = document.createElement('input');
  input.setAttribute('role', 'spinbutton');
  input.value = '42';
  const increment = document.createElement('button');
  increment.dataset.testid = 'increment';
  increment.textContent = '+';
  const decrement = document.createElement('button');
  decrement.dataset.testid = 'decrement';
  decrement.textContent = '-';
  spinner.append(input, increment, decrement);
  container.append(spinner);
  layout.append(label, container);
  root.append(layout);
  const numeric = {
    type: { __name: 'ScrubableNumberInput' },
    component: {
      props: { modelValue: 42, min: 0, max: 999999, step: 1 },
      emit: jest.fn(),
      proxy: { $watch: () => () => undefined },
      subTree: { el: spinner },
    },
  };
  const vnode: UnknownRecord = {
    type: { __name: 'WidgetInputNumber' },
    component: {
      props: { widget: { name: 'seed', options: { precision: 0 } } },
      subTree: {
        el: layout,
        children: [
          {
            type: { __name: 'WidgetLayoutField' },
            component: { subTree: { el: layout, children: [numeric] } },
          },
        ],
      },
    },
  };
  registerNativeWidgetMount(root, {
    vnode,
    seed: {
      modes: ['fixed', 'randomize', 'increment', 'decrement'],
      readMode: () => 'fixed',
      writeMode: jest.fn(),
    },
  });
  document.body.append(root);
  return { root, spinner, input, increment, decrement };
}

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
  jest.restoreAllMocks();
});

test.each(['comfy', 'fluent', 'spectrum', 'photopro'] as const)(
  '%s keeps the same seed input and split controls when switching Nodes 1.0 and 2.0',
  async (style) => {
    const widget = seedWidget();
    const frames: FrameRequestCallback[] = [];
    jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
    jest.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => undefined);
    const events = new EventTarget();
    let modern = false;
    const errors = jest.fn();
    const host = new AppearanceHost({
      document,
      reportError: errors,
      app: {
        ui: {
          settings: {
            addSetting: () => undefined,
            getSettingValue: (id: string) =>
              id === VISUAL_STYLE_SETTING
                ? style
                : id === 'Comfy.VueNodes.Enabled'
                  ? modern
                  : undefined,
            addEventListener: events.addEventListener.bind(events),
            removeEventListener: events.removeEventListener.bind(events),
          },
        },
      },
    });
    host.setup();
    const refresh = async (): Promise<void> => {
      prepareNativePresentation(widget.root);
      for (let cycle = 0; cycle < 3; cycle++) {
        await Promise.resolve();
        frames.splice(0).forEach((frame) => frame(0));
      }
    };
    try {
      await refresh();
      expect(errors).not.toHaveBeenCalled();
      expect(widget.spinner.hasAttribute('data-sc-seed-box')).toBe(true);
      expect(getComputedStyle(widget.increment).display).toBe('none');
      expect(getComputedStyle(widget.decrement).display).toBe('none');
      const split = widget.root.querySelector('[data-sc-seed-split]');
      expect(split).not.toBeNull();
      for (const next of [true, false, true]) {
        modern = next;
        events.dispatchEvent(new Event('Comfy.VueNodes.Enabled.change'));
        await refresh();
        expect(widget.root.querySelector('[data-sc-seed-split]')).toBe(split);
        expect(widget.root.querySelector('input')).toBe(widget.input);
        expect(widget.input.value).toBe('42');
        expect(getComputedStyle(widget.increment).display).toBe('none');
        expect(getComputedStyle(widget.decrement).display).toBe('none');
      }
      expect(errors).not.toHaveBeenCalled();
    } finally {
      host.dispose();
    }
  },
);

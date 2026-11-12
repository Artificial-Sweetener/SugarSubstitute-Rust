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
/** Prove active gallery titles reveal overflow without changing selection or leaking animation. */
import { createApp, h, nextTick, reactive } from 'vue';
import { jest, test, expect } from '@jest/globals';
import { ModelPickerWall } from '../../../frontend/comfyui/ui/model_library/vue/ModelPickerWall.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import { ModelFixtureResizeObserver } from '../support/resize_observer.js';

/** Mount the real gallery with deterministic browser measurements and time. */
function fixture(reduced = false, textWidth = 232) {
  jest.useFakeTimers();
  Object.defineProperty(window, 'ResizeObserver', {
    configurable: true,
    value: ModelFixtureResizeObserver,
  });
  const motion = { matches: reduced, addEventListener: jest.fn(), removeEventListener: jest.fn() };
  Object.defineProperty(window, 'matchMedia', { configurable: true, value: () => motion });
  jest.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(100);
  jest.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: HTMLElement,
  ) {
    const width = this.classList.contains('sc-model-marquee-text') ? textWidth : 100;
    return {
      x: 0,
      y: 0,
      width,
      height: 17,
      left: 0,
      top: 0,
      right: width,
      bottom: 17,
      toJSON: () => ({}),
    };
  });
  const props = reactive({
    items: modelPickerChoices([], 'checkpoints', [
      '<long>_literal_title.safetensors',
      'another_long_title.safetensors',
    ]),
    selected: '<long>_literal_title.safetensors',
    active: 0,
    optionPrefix: 'motion',
  });
  const host = document.createElement('div');
  document.body.append(host);
  const app = createApp({ render: () => h(ModelPickerWall, props) });
  app.mount(host);
  return { host, props, app, motion };
}

afterEach(() => {
  jest.useRealTimers();
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

test('reveals only the highlighted title through the reference hold, scroll and end phases', async () => {
  const wall = fixture();
  try {
    await nextTick();
    const title = wall.host.querySelector('.sc-model-title');
    expect(title?.getAttribute('data-marquee-phase')).toBe('start');
    expect(title?.textContent).toBe('<long>_literal_title');
    expect(title?.querySelector('long')).toBeNull();
    jest.advanceTimersByTime(1500);
    await nextTick();
    expect(title?.getAttribute('data-marquee-phase')).toBe('scroll');
    expect(title?.querySelector<HTMLElement>('.sc-model-marquee-text')?.style.transform).toMatch(
      /translateX\(-/,
    );
    expect(title?.getAttribute('data-marquee-left-fade')).toBe('true');
    expect(title?.getAttribute('data-marquee-right-fade')).toBe('true');
    jest.advanceTimersByTime(2500);
    await nextTick();
    expect(title?.getAttribute('data-marquee-phase')).toBe('end');
    wall.props.active = 1;
    await nextTick();
    expect(title?.getAttribute('data-marquee-phase')).toBe('static');
    expect(
      wall.host.querySelectorAll('.sc-model-title')[1]?.getAttribute('data-marquee-phase'),
    ).toBe('start');
    expect(wall.props.selected).toBe('<long>_literal_title.safetensors');
  } finally {
    wall.app.unmount();
  }
  expect(jest.getTimerCount()).toBe(0);
});

test('keeps readable static elision for reduced motion', async () => {
  const wall = fixture(true);
  try {
    await nextTick();
    expect(wall.host.querySelector('.sc-model-title')?.getAttribute('data-marquee-phase')).toBe(
      'static',
    );
    expect(jest.getTimerCount()).toBe(0);
  } finally {
    wall.app.unmount();
  }
  expect(wall.motion.removeEventListener).toHaveBeenCalled();
});

test('does not animate a title that fits its tile', async () => {
  const wall = fixture(false, 80);
  try {
    await nextTick();
    expect(wall.host.querySelector('.sc-model-title')?.getAttribute('data-marquee-phase')).toBe(
      'static',
    );
    expect(jest.getTimerCount()).toBe(0);
  } finally {
    wall.app.unmount();
  }
});

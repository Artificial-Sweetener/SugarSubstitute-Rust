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
/** Verify authoritative Cube prompt textarea browser sizing. */

import { describe, expect, jest, test } from '@jest/globals';

import {
  CubePromptTextareaAutosizer,
  settleCubePromptTextareaGeometry,
  PROMPT_VALUE_CHANGE_EVENT,
} from '../../../frontend/comfyui/ui/surface/CubePromptTextareaAutosizer.js';

describe('CubePromptTextareaAutosizer', () => {
  test('preserves settled geometry when native rendering rebinds and assigns an unchanged value', () => {
    const textarea = document.createElement('textarea');
    const measure = jest.fn(() => 40);
    Object.defineProperty(textarea, 'scrollHeight', { get: measure });
    const autosizer = new CubePromptTextareaAutosizer(document);
    autosizer.bind(textarea);
    measure.mockClear();
    autosizer.bind(textarea);
    const unchanged = textarea.value;
    textarea.value = unchanged;
    expect(measure).not.toHaveBeenCalled();
    expect(textarea.style.height).toBe('40px');
    textarea.value = 'changed content';
    expect(measure).toHaveBeenCalledTimes(1);
    autosizer.dispose();
  });
  test('includes the border box so fully fitted content has no phantom scroll range', () => {
    const textarea = document.createElement('textarea');
    textarea.style.boxSizing = 'border-box';
    Object.defineProperties(textarea, {
      scrollHeight: { value: 50 },
      clientHeight: { value: 31 },
      offsetHeight: { value: 33 },
    });
    const autosizer = new CubePromptTextareaAutosizer(document);
    autosizer.bind(textarea);
    expect(textarea.style.height).toBe('52px');
    autosizer.dispose();
  });

  test('notifies scoped value consumers once per actual programmatic change and releases observation', () => {
    const textarea = document.createElement('textarea');
    const changed = jest.fn();
    textarea.addEventListener(PROMPT_VALUE_CHANGE_EVENT, changed);
    const autosizer = new CubePromptTextareaAutosizer(document);
    autosizer.bind(textarea);
    textarea.value = 'updated';
    textarea.value = 'updated';
    expect(changed).toHaveBeenCalledTimes(1);
    autosizer.dispose();
    textarea.value = 'released';
    expect(changed).toHaveBeenCalledTimes(1);
    expect(Object.hasOwn(textarea, 'value')).toBe(false);
  });
  test('shrinks to content and themed minimum instead of intrinsic textarea rows', () => {
    const textarea = document.createElement('textarea');
    textarea.rows = 2;
    Object.defineProperty(textarea, 'scrollHeight', {
      get: () => (textarea.style.height === '0px' ? 33 : 65),
    });
    const autosizer = new CubePromptTextareaAutosizer(document);
    autosizer.bind(textarea);
    expect(textarea.style.height).toBe('33px');
    expect(textarea.rows).toBe(2);
    autosizer.dispose();
  });
  test('owns content height and restores exact inline styles on release', () => {
    const textarea = document.createElement('textarea');
    textarea.style.setProperty('height', '30px', 'important');
    textarea.style.overflowY = 'auto';
    Object.defineProperty(textarea, 'scrollHeight', { configurable: true, value: 168 });
    const onHeightChange = jest.fn();
    const autosizer = new CubePromptTextareaAutosizer(document);

    autosizer.bind(textarea, onHeightChange);

    expect(textarea.style.height).toBe('168px');
    expect(textarea.style.overflowY).toBe('hidden');
    expect(textarea.style.getPropertyPriority('height')).toBe('important');
    expect(Object.hasOwn(textarea, 'value')).toBe(true);
    expect(onHeightChange).toHaveBeenCalledWith(168);

    autosizer.unbind(textarea);

    expect(textarea.style.height).toBe('30px');
    expect(textarea.style.getPropertyPriority('height')).toBe('important');
    expect(textarea.style.overflowY).toBe('auto');
    expect(Object.hasOwn(textarea, 'value')).toBe(false);
    autosizer.dispose();
  });

  test('remeasures property-only workflow hydration through the native value setter', () => {
    const textarea = document.createElement('textarea');
    let contentHeight = 1;
    Object.defineProperty(textarea, 'scrollHeight', {
      configurable: true,
      get: () => contentHeight,
    });
    const autosizer = new CubePromptTextareaAutosizer(document);
    autosizer.bind(textarea);
    expect(textarea.style.height).toBe('1px');

    contentHeight = 148;
    textarea.value = 'hydrated multiline workflow prompt';

    expect(textarea.style.height).toBe('148px');
    autosizer.dispose();
  });

  test('remeasures subsequent editor input immediately', () => {
    const textarea = document.createElement('textarea');
    let contentHeight = 80;
    Object.defineProperty(textarea, 'scrollHeight', {
      configurable: true,
      get: () => contentHeight,
    });
    const autosizer = new CubePromptTextareaAutosizer(document);
    autosizer.bind(textarea);

    contentHeight = 220;
    textarea.dispatchEvent(new Event('input'));

    expect(textarea.style.height).toBe('220px');
    autosizer.dispose();
  });
});

test('batches presentation sizing across separately owned prompts before publishing heights', () => {
  const root = document.createElement('section');
  const prompts = [document.createElement('textarea'), document.createElement('textarea')];
  root.append(...prompts);
  const owners = prompts.map(() => new CubePromptTextareaAutosizer(document));
  let measuring = false;
  const reads: number[] = [];
  prompts.forEach((prompt, index) => {
    Object.defineProperty(prompt, 'scrollHeight', {
      get: () => {
        if (measuring) {
          expect(prompts.every((member) => member.style.height === '0px')).toBe(true);
          reads.push(index);
        }
        return 40 + index * 20;
      },
    });
    owners[index]?.bind(prompt);
  });
  measuring = true;
  settleCubePromptTextareaGeometry([root]);
  expect(reads).toEqual([0, 1]);
  expect(prompts.map((prompt) => prompt.style.height)).toEqual(['40px', '60px']);
  owners.forEach((owner) => owner.dispose());
});

test('ignores resize delivery for explicitly settled prompt geometry and refits actual width changes', () => {
  const descriptor = Object.getOwnPropertyDescriptor(window, 'ResizeObserver');
  let deliver: ResizeObserverCallback | null = null;
  let observation: ResizeObserver | null = null;
  class Observer implements ResizeObserver {
    constructor(callback: ResizeObserverCallback) {
      deliver = callback;
      observation = this;
    }
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }
  Object.defineProperty(window, 'ResizeObserver', { configurable: true, value: Observer });
  const textarea = document.createElement('textarea');
  let width = 200;
  let reads = 0;
  Object.defineProperty(textarea, 'offsetWidth', { get: () => width });
  Object.defineProperty(textarea, 'scrollHeight', {
    get: () => {
      reads += 1;
      return 33;
    },
  });
  const root = document.createElement('section');
  root.append(textarea);
  const autosizer = new CubePromptTextareaAutosizer(document);
  try {
    autosizer.bind(textarea);
    settleCubePromptTextareaGeometry([root]);
    reads = 0;
    const send = (inlineSize: number): void => {
      const callback: ResizeObserverCallback | null = deliver;
      if (!callback || !observation) throw new Error('Prompt resize observation missing');
      callback(
        [
          {
            target: textarea,
            contentRect: textarea.getBoundingClientRect(),
            borderBoxSize: [{ inlineSize, blockSize: 33 }],
            contentBoxSize: [],
            devicePixelContentBoxSize: [],
          },
        ],
        observation,
      );
    };
    send(200);
    expect(reads).toBe(0);
    width = 300;
    send(300);
    expect(reads).toBe(1);
  } finally {
    autosizer.dispose();
    if (descriptor) Object.defineProperty(window, 'ResizeObserver', descriptor);
    else Reflect.deleteProperty(window, 'ResizeObserver');
  }
});

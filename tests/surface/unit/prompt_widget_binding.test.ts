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
/** Prove prompt edits retain native value, callback, history and external-update behavior. */
import { jest } from '@jest/globals';
import { PromptWidgetBinding } from '../../../frontend/comfyui/ui/prompt_editor/PromptWidgetBinding.js';
import type { ComfyNode, ComfyWidget } from '../../../frontend/comfyui/ui/types/graph.js';

test('preserves inherited native widget accessors and their serialized DOM value', () => {
  const textarea = document.createElement('textarea');
  textarea.value = 'original';
  const widget: ComfyWidget = { name: 'text' };
  Object.setPrototypeOf(widget, {
    get value(): string {
      return textarea.value;
    },
    set value(source: string) {
      textarea.value = source;
    },
  });
  const binding = new PromptWidgetBinding({}, widget, () => undefined);
  binding.commit('committed rich source');
  expect(textarea.value).toBe('committed rich source');
  textarea.value = 'native workflow restore';
  expect(binding.read()).toBe('native workflow restore');
  binding.dispose();
  expect(Object.hasOwn(widget, 'value')).toBe(false);
  expect(widget.value).toBe('native workflow restore');
});

test('commits exact source once inside native history and restores property ownership', () => {
  const steps: string[] = [];
  const callback = jest.fn(() => steps.push('callback'));
  const changed = jest.fn();
  const widget: ComfyWidget = { name: 'text', value: 'before', callback };
  const node: ComfyNode = {
    widgets: [widget],
    graph: {
      beforeChange: () => steps.push('before'),
      afterChange: () => steps.push('after'),
    },
  };
  const binding = new PromptWidgetBinding(node, widget, changed);
  const source = '(literal <b>text</b>:1.20)\r\n<lora:folder/model:0.8>';
  binding.commit(source);
  binding.commit(source);
  expect(widget.value).toBe(source);
  expect(steps).toEqual(['before', 'callback', 'after']);
  expect(changed).not.toHaveBeenCalled();
  widget.value = 'host undo';
  expect(changed).toHaveBeenCalledWith('host undo');
  binding.dispose();
  expect(widget.value).toBe('host undo');
  expect(Object.getOwnPropertyDescriptor(widget, 'value')?.get).toBeUndefined();
});

test('preserves a native accessor and balances graph history when its callback fails', () => {
  let value = 'original';
  const failure = new Error('native callback');
  const after = jest.fn();
  const widget: ComfyWidget = {
    name: 'text',
    callback: () => {
      throw failure;
    },
  };
  const descriptor: PropertyDescriptor = {
    configurable: true,
    get: () => value,
    set: (next: string) => {
      value = next;
    },
  };
  Object.defineProperty(widget, 'value', descriptor);
  const binding = new PromptWidgetBinding(
    { graph: { beforeChange: jest.fn(), afterChange: after } },
    widget,
    jest.fn(),
  );
  expect(() => binding.commit('new')).toThrow(failure);
  expect(after).toHaveBeenCalledTimes(1);
  binding.dispose();
  expect(Object.getOwnPropertyDescriptor(widget, 'value')?.get).toBe(descriptor.get);
  expect(widget.value).toBe('new');
});

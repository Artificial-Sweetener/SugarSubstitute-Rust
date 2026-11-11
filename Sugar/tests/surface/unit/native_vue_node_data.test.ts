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
/** Bound repeated native accessor installation without replacing Comfy's rendering or widget membership policy. */
import { extractNativeVueNodeData } from '../../../frontend/comfyui/ui/surface/NativeVueNodeData.js';
import type { ComfyNode, ComfyWidget } from '../../../frontend/comfyui/ui/types/graph.js';

/** Model only the host capability's observable accessor installation, not its Vue rendering implementation. */
function host() {
  let reads = 0;
  const extract = (node: ComfyNode): unknown => {
    const previous = Object.getOwnPropertyDescriptor(node, 'widgets')?.get,
      widgets = node.widgets ?? [];
    Object.defineProperty(node, 'widgets', {
      configurable: true,
      enumerable: true,
      get() {
        reads++;
        return previous ? previous.call(node) : widgets;
      },
      set(value: ComfyWidget[]) {
        widgets.splice(0, widgets.length, ...value);
      },
    });
    return { title: node.title, widgets: node.widgets };
  };
  return { extract, reads: () => reads };
}

test('repeated extraction keeps native widget reads bounded while metadata, values and membership change', () => {
  const node: ComfyNode = { widgets: [{ name: 'text', value: 'start' }] },
    native = host(),
    count = 256;
  for (let at = 0; at < count; at++) {
    node.title = String(at);
    const widget = node.widgets?.[0];
    if (!widget) throw new Error('Missing current widget');
    widget.value = String(at);
    const result = extractNativeVueNodeData(node, native.extract);
    expect(result).toEqual({
      title: String(at),
      widgets: [
        { name: 'text', value: String(at) },
        ...(at > 50 ? [{ name: 'extra', value: 'present' }] : []),
      ],
    });
    if (at === 50) node.widgets?.push({ name: 'extra', value: 'present' });
    if (at >= 50) expect(node.widgets?.some((w) => w.name === 'extra')).toBe(true);
  }
  expect(native.reads()).toBeLessThanOrEqual(count * 8);
});

test('dynamic base getters and later external descriptor replacement remain authoritative', () => {
  const a: ComfyWidget[] = [{ name: 'one' }],
    b: ComfyWidget[] = [{ name: 'two' }],
    node: ComfyNode = { properties: { visible: true } },
    native = host();
  Object.defineProperty(node, 'widgets', {
    configurable: true,
    enumerable: true,
    get(this: ComfyNode) {
      return this.properties?.visible ? a : b;
    },
  });
  for (let at = 0; at < 128; at++) {
    node.properties = { visible: at % 2 === 0 };
    expect(extractNativeVueNodeData(node, native.extract)).toEqual({
      title: undefined,
      widgets: at % 2 === 0 ? a : b,
    });
  }
  Object.defineProperty(node, 'widgets', {
    configurable: true,
    enumerable: true,
    get() {
      return b;
    },
  });
  for (let at = 0; at < 128; at++)
    expect(extractNativeVueNodeData(node, native.extract)).toEqual({
      title: undefined,
      widgets: b,
    });
  expect(native.reads()).toBeLessThanOrEqual(256 * 5);
});

test('different runtime instances share the same node descriptor owner and preserve native setter changes', () => {
  const node: ComfyNode = { widgets: [] },
    first = host(),
    second = host();
  for (let at = 0; at < 128; at++) {
    extractNativeVueNodeData(node, at % 2 ? first.extract : second.extract);
    node.widgets = [{ name: String(at) }];
    expect(node.widgets?.[0]?.name).toBe(String(at));
  }
  expect(first.reads() + second.reads()).toBeLessThanOrEqual(128 * 5);
});

test('partial host failure retains its original cause and does not accumulate later accessor layers', () => {
  const node: ComfyNode = { widgets: [{ name: 'text' }] },
    native = host(),
    cause = new Error('host extraction failed');
  expect(() =>
    extractNativeVueNodeData(node, (current) => {
      native.extract(current);
      throw cause;
    }),
  ).toThrow(cause);
  for (let at = 0; at < 128; at++) extractNativeVueNodeData(node, native.extract);
  expect(native.reads()).toBeLessThanOrEqual(129 * 4);
});

test('inherited dynamic widget membership survives repeated native own-accessor installation', () => {
  const node: ComfyNode = {},
    native = host();
  let roster: ComfyWidget[] = [];
  Object.setPrototypeOf(node, {
    get widgets() {
      return roster;
    },
  });
  for (let at = 0; at < 128; at++) {
    roster = [{ name: String(at) }];
    expect(extractNativeVueNodeData(node, native.extract)).toEqual({
      title: undefined,
      widgets: roster,
    });
  }
  expect(native.reads()).toBeLessThanOrEqual(128 * 4);
});

test('a nonconfigurable host replacement fails visibly before another extraction can mutate ownership', () => {
  const node: ComfyNode = { widgets: [] };
  let calls = 0;
  const extract = (current: ComfyNode): unknown => {
    calls++;
    Object.defineProperty(current, 'widgets', {
      configurable: false,
      get() {
        return [];
      },
    });
    return { widgets: [] };
  };
  extractNativeVueNodeData(node, extract);
  expect(() => extractNativeVueNodeData(node, extract)).toThrow(TypeError);
  expect(calls).toBe(1);
});

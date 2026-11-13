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
import assert from 'node:assert/strict';
import type { PreparedReorderPaintNode } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-preparation.ts';
/** Finite controlled clone transport keeps declaration values/priorities isolated by native copy identity. */
export interface PlatformNode {
  readonly namespaceURI: string;
  readonly localName: string;
  readonly style: {
    cssText: string;
    setProperty(name: string, value: string, priority?: string): void;
  };
  readonly priorities: Map<string, readonly [string, string]>;
  readonly attributes: Map<string, string>;
  className: string;
  contentEditable: string;
  readonly children: unknown[];
  setAttribute(name: string, value: string): void;
  append(child: unknown): void;
  cloneNode(deep: boolean): PlatformNode;
  seed(value: string): void;
  remove(): void;
}
/** Independent native declaration transport records exact serialization and important priorities, including clone isolation. */
export function platform(failure?: {
  readonly stage: 'allocate' | 'style' | 'clone';
  readonly cause: unknown;
}) {
  let allocations = 0,
    publications = 0,
    clones = 0,
    removals = 0;
  const nodes: PlatformNode[] = [];
  /** Only the native element/style boundary is controlled; actual paint/request owners run unchanged. */
  function element(namespace: string, tag: string): PlatformNode {
    ++allocations;
    if (failure?.stage === 'allocate') throw failure.cause;
    let serialized = '';
    const priorities = new Map<string, readonly [string, string]>(),
      attributes = new Map<string, string>();
    const style = {
      get cssText() {
        return serialized;
      },
      set cssText(value: string) {
        ++publications;
        if (failure?.stage === 'style') throw failure.cause;
        serialized = value;
      },
      setProperty(name: string, value: string, priority = '') {
        priorities.set(name, [value, priority]);
      },
    };
    const node: PlatformNode = {
      namespaceURI: namespace,
      localName: tag,
      style,
      priorities,
      attributes,
      className: '',
      contentEditable: 'inherit',
      children: [] as unknown[],
      setAttribute(name: string, value: string) {
        attributes.set(name, value);
      },
      append(child: unknown) {
        this.children.push(child);
      },
      cloneNode(deep: boolean) {
        assert.equal(deep, false);
        ++clones;
        if (failure?.stage === 'clone') throw failure.cause;
        const copy = element(namespace, tag);
        copy.seed(serialized);
        for (const [name, value] of priorities) copy.priorities.set(name, value);
        for (const [name, value] of attributes) copy.attributes.set(name, value);
        copy.className = this.className;
        copy.contentEditable = this.contentEditable;
        return copy;
      },
      remove(): void {
        ++removals;
      },
      seed(value: string) {
        serialized = value;
      },
    };
    nodes.push(node);
    return node;
  }
  const document = {
    createElement(name: string) {
      return element('http://www.w3.org/1999/xhtml', name);
    },
    createElementNS(namespace: string, name: string) {
      return element(namespace, name);
    },
    createTextNode(text: string) {
      return { textContent: text };
    },
  };
  return {
    document,
    nodes,
    get removals() {
      return removals;
    },
    get allocations() {
      return allocations;
    },
    get publications() {
      return publications;
    },
    get clones() {
      return clones;
    },
  };
}
/** Canonical immutable descriptor carries literal resolved declarations rather than a cache class identity. */
export function descriptor(
  cssText = 'font:14px serif;color:rgb(1, 2, 3);--sp-region-color:green;',
  tag = 'span',
  vector = false,
): Extract<PreparedReorderPaintNode, { readonly kind: 'element' }> {
  return Object.freeze({
    kind: 'element',
    tag,
    vector,
    cssText,
    opacity: '1',
    display: 'inline',
    position: 'static',
    inset: 'auto',
    transform: 'none',
    generatedOutside: null,
    buffer: false,
    media: null,
    className: '',
    attributes: Object.freeze([]),
    readonly: false,
    children: Object.freeze([]),
  });
}
/** Every retained inert declaration keeps its original important priority. */
export function safety(node: ReturnType<typeof platform>['nodes'][number]) {
  for (const name of [
    'animation',
    'transition',
    'pointer-events',
    '-webkit-user-modify',
    'user-select',
  ])
    assert.deepEqual(node.priorities.get(name), [
      name === '-webkit-user-modify' ? 'read-only' : 'none',
      'important',
    ]);
}

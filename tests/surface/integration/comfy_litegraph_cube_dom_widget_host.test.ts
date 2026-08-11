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
/** Verify exact Nodes 1.0 DOM widgets remain usable on Cube faces. */

import { jest } from '@jest/globals';
import { ComfyLiteGraphCubeDomWidgetHost } from '../../../frontend/comfyui/ui/surface/ComfyLiteGraphCubeDomWidgetHost.js';
import type { CubeCanvasLayout } from '../../../frontend/comfyui/ui/surface/CubeCanvasLayout.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

describe('ComfyLiteGraphCubeDomWidgetHost', () => {
  test('mounts the exact native element at transformed face geometry without replacing it', () => {
    const original = document.createElement('div');
    const before = document.createElement('span');
    const textarea = document.createElement('textarea');
    const after = document.createElement('span');
    original.append(before, textarea, after);
    document.body.append(original);
    const input = jest.fn();
    textarea.addEventListener('input', input);
    const { canvas, host } = createHost();
    const item = cubeItem(textarea);

    host.sync([item]);

    const wrapper = document.querySelector<HTMLElement>('[data-sugarcubes-cube-face-dom-widget]');
    expect(wrapper).not.toBeNull();
    expect(wrapper?.firstElementChild).toBe(textarea);
    expect(document.querySelectorAll('textarea')).toHaveLength(1);
    expect(wrapper?.style.left).toBe('260px');
    expect(wrapper?.style.top).toBe('550px');
    expect(wrapper?.style.width).toBe('280px');
    expect(wrapper?.style.height).toBe('80px');
    expect(wrapper?.style.transform).toBe('scale(2)');
    expect(wrapper?.style.pointerEvents).toBe('auto');
    expect(canvas.getBoundingClientRect).toHaveBeenCalled();

    textarea.value = 'native multiline value';
    textarea.dispatchEvent(new Event('input', { bubbles: true }));
    expect(input).toHaveBeenCalledTimes(1);
    expect(item.layout.cards[0]?.node.widgets?.[0]?.value).toBeUndefined();

    host.sync([item]);
    expect(document.querySelector<HTMLElement>('[data-sugarcubes-cube-face-dom-widget]')).toBe(
      wrapper,
    );
    expect(wrapper?.firstElementChild).toBe(textarea);

    host.dispose();
    expect(original.children).toEqual(expect.objectContaining({ length: 3 }));
    expect(original.children[0]).toBe(before);
    expect(original.children[1]).toBe(textarea);
    expect(original.children[2]).toBe(after);
    expect(document.querySelector('[data-sugarcubes-cube-face-dom-widgets]')).toBeNull();
    original.remove();
  });

  test('restores native ownership when the Cube is hidden or its host is disabled', () => {
    const original = document.createElement('div');
    const textarea = document.createElement('textarea');
    original.append(textarea);
    document.body.append(original);
    const { host } = createHost();
    const item = cubeItem(textarea);

    host.sync([item]);
    expect(textarea.parentElement).not.toBe(original);

    host.sync([]);
    expect(textarea.parentElement).toBe(original);

    host.sync([item]);
    host.sync([]);
    expect(textarea.parentElement).toBe(original);

    host.dispose();
    original.remove();
  });

  test('reclaims the exact multiline element after Comfy remounts it', () => {
    const original = document.createElement('div');
    const textarea = document.createElement('textarea');
    original.append(textarea);
    document.body.append(original);
    const input = jest.fn();
    textarea.addEventListener('input', input);
    const { host } = createHost();
    const item = cubeItem(textarea);

    host.sync([item]);
    const faceWrapper = document.querySelector<HTMLElement>(
      '[data-sugarcubes-cube-face-dom-widget]',
    );
    expect(faceWrapper?.firstElementChild).toBe(textarea);

    original.append(textarea);
    expect(faceWrapper?.childElementCount).toBe(0);

    host.sync([item]);
    expect(faceWrapper?.firstElementChild).toBe(textarea);
    textarea.focus();
    textarea.value = 'editable after native remount';
    textarea.dispatchEvent(new Event('input', { bubbles: true }));

    expect(document.activeElement).toBe(textarea);
    expect(input).toHaveBeenCalledTimes(1);

    host.dispose();
    original.remove();
  });

  test('ignores hidden, disabled, and non-DOM widgets without disturbing native elements', () => {
    const hiddenElement = document.createElement('textarea');
    const disabledElement = document.createElement('textarea');
    const node = faceNode([
      {
        name: 'hidden',
        element: hiddenElement,
        y: 20,
        width: 300,
        computedHeight: 100,
        margin: 10,
        isVisible: () => false,
      },
      {
        name: 'disabled',
        element: disabledElement,
        y: 20,
        width: 300,
        computedHeight: 100,
        margin: 10,
        computedDisabled: true,
      },
      { name: 'canvas-only', y: 20, computedHeight: 20 },
    ]);
    const { host } = createHost();

    host.sync([cubeItemForNode(node)]);

    expect(document.querySelectorAll('[data-sugarcubes-cube-face-dom-widget]')).toHaveLength(1);
    const mounted = document.querySelector<HTMLElement>('[data-sugarcubes-cube-face-dom-widget]');
    expect(mounted?.firstElementChild).toBe(disabledElement);
    expect(mounted?.style.pointerEvents).toBe('none');
    expect(mounted?.style.opacity).toBe('0.5');
    expect(hiddenElement.parentElement).toBeNull();

    host.dispose();
  });

  test('grows a semantic prompt textarea and requests a new Cube layout on input', () => {
    const textarea = document.createElement('textarea');
    Object.defineProperty(textarea, 'scrollHeight', { configurable: true, value: 80 });
    const widget = {
      name: 'text',
      type: 'customtext',
      element: textarea,
      y: 20,
      computedHeight: 100,
      margin: 10,
    };
    const node = faceNode([widget]);
    node.title = 'Positive prompt';
    const onGeometryChange = jest.fn();
    const { host } = createHost(onGeometryChange);

    host.sync([cubeItemForNode(node)]);

    expect(textarea.style.height).toBe('80px');
    expect(textarea.style.overflowY).toBe('hidden');
    expect(widget.computedHeight).toBe(100);
    const wrapper = document.querySelector<HTMLElement>('[data-sugarcubes-cube-face-dom-widget]');
    expect(wrapper?.style.top).toBe('514px');
    expect(wrapper?.style.height).toBe('128px');
    onGeometryChange.mockClear();
    Object.defineProperty(textarea, 'scrollHeight', { configurable: true, value: 170 });
    textarea.dispatchEvent(new Event('input'));
    expect(textarea.style.height).toBe('170px');
    expect(onGeometryChange).toHaveBeenCalled();

    host.dispose();
  });

  test('remeasures a Nodes 1 prompt after property-only workflow hydration', () => {
    const textarea = document.createElement('textarea');
    let contentHeight = 1;
    Object.defineProperty(textarea, 'scrollHeight', {
      configurable: true,
      get: () => contentHeight,
    });
    const widget = {
      name: 'text',
      type: 'customtext',
      element: textarea,
      y: 20,
      computedHeight: 100,
      margin: 10,
    };
    const node = faceNode([widget]);
    node.title = 'Positive prompt';
    const onGeometryChange = jest.fn();
    const { host } = createHost(onGeometryChange);

    host.sync([cubeItemForNode(node)]);
    expect(textarea.style.height).toBe('1px');
    onGeometryChange.mockClear();

    contentHeight = 170;
    textarea.value = 'hydrated multiline workflow prompt';

    expect(textarea.style.height).toBe('170px');
    expect(onGeometryChange).toHaveBeenCalledTimes(1);
    host.dispose();
  });
});

/** Build one transformed canvas and its focused DOM-widget host. */
function createHost(onGeometryChange = jest.fn()): {
  canvas: HTMLCanvasElement;
  host: ComfyLiteGraphCubeDomWidgetHost;
} {
  const canvas = document.createElement('canvas');
  canvas.getBoundingClientRect = jest.fn<() => DOMRect>(
    () =>
      ({
        left: 20,
        top: 40,
        right: 1020,
        bottom: 840,
        width: 1000,
        height: 800,
        x: 20,
        y: 40,
        toJSON: () => ({}),
      }) as DOMRect,
  );
  document.body.append(canvas);
  return {
    canvas,
    host: new ComfyLiteGraphCubeDomWidgetHost({
      document,
      canvas: {
        canvas,
        ds: { scale: 2, offset: [10, -5] },
      },
      onGeometryChange,
    }),
  };
}

/** Build one revealed Cube item containing a native multiline widget. */
function cubeItem(element: HTMLTextAreaElement) {
  return cubeItemForNode(
    faceNode([
      {
        name: 'text',
        element,
        y: 20,
        width: 300,
        computedHeight: 100,
        margin: 10,
      },
    ]),
  );
}

/** Build one face item with deterministic card geometry. */
function cubeItemForNode(node: ComfyNode): {
  layout: CubeCanvasLayout;
} {
  return {
    layout: {
      frame: { x: 90, y: 130, width: 720, height: 510 },
      header: { x: 90, y: 130, width: 720, height: 42 },
      content: { x: 102, y: 172, width: 696, height: 456 },
      inputGutter: { x: 90, y: 172, width: 84, height: 456 },
      outputGutter: { x: 770, y: 172, width: 40, height: 456 },
      masonry: { x: 102, y: 172, width: 696, height: 456 },
      preview: null,
      previewDivider: null,
      previewWidthRange: { minimum: 160, maximum: 444 },
      editAction: { x: 650, y: 139, width: 68, height: 24 },
      unsavedIndicator: null,
      chromeActions: {
        'swap-left': { x: 618, y: 139, width: 28, height: 24 },
        'swap-right': { x: 652, y: 139, width: 28, height: 24 },
      },
      resizeHandles: [],
      cards: [
        {
          node,
          id: 'inner',
          label: 'Inner',
          enabled: true,
          showActivationControl: false,
          bodyHeight: 150,
          rect: { x: 100, y: 200, width: 300, height: 180 },
          activationAction: null,
        },
      ],
      inputs: [],
      outputs: [],
      minimumSize: [440, 180],
    },
  };
}

/** Build the dynamic native-node shape consumed at the Comfy adapter boundary. */
function faceNode(widgets: object[]): ComfyNode {
  return {
    id: 'inner',
    pos: [0, 0],
    size: [300, 150],
    widgets: widgets.map((widget, index) => ({
      name: `widget-${String(index)}`,
      ...widget,
    })),
  };
}

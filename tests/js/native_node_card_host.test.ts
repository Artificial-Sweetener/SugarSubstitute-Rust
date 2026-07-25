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
/** Verify SugarCubes passes real internal graph nodes to Comfy's native renderer. */

import { describe, expect, jest, test } from '@jest/globals';

import { NativeNodeCardHost } from '../../frontend/comfyui/ui/surface/NativeNodeCardHost.js';
import type {
  NativeNodeCardMount,
  NativeNodeCardRenderer,
} from '../../frontend/comfyui/ui/surface/NativeNodeCardRenderer.js';
import type { ComfyNode } from '../../frontend/comfyui/ui/types/graph.js';

describe('NativeNodeCardHost', () => {
  test('mounts each exact internal node without cloning or projecting it', () => {
    const mountedNodes: ComfyNode[] = [];
    const renderer: NativeNodeCardRenderer = {
      mount: jest.fn((_target: HTMLElement, node: ComfyNode) => {
        mountedNodes.push(node);
        return createMount();
      }),
      dispose: jest.fn(),
    };
    const first: ComfyNode = {
      id: 'inside-a',
      type: 'KSampler',
      graph: { id: 'cube-definition' },
    };
    const second: ComfyNode = { id: 'inside-b', type: 'VAEDecode' };
    const root = document.createElement('div');
    const host = new NativeNodeCardHost(renderer);

    host.mount(root, [card(first), card(second)], jest.fn());

    expect(mountedNodes).toEqual([first, second]);
    expect(root.querySelectorAll('[data-cube-node-id]')).toHaveLength(2);
    expect(
      root.querySelector('[data-cube-node-id="inside-a"]')?.getAttribute('data-cube-node-locator'),
    ).toBe('cube-definition:inside-a');
  });

  test('owns mount cleanup without disposing the shared Comfy renderer', () => {
    const mounts = [createMount(), createMount()];
    let mountIndex = 0;
    const renderer: NativeNodeCardRenderer = {
      mount: () => mounts[mountIndex++] ?? createMount(),
      dispose: jest.fn(),
    };
    const host = new NativeNodeCardHost(renderer);
    host.mount(
      document.createElement('div'),
      [card({ id: '1', type: 'A' }), card({ id: '2', type: 'B' })],
      jest.fn(),
    );

    host.dispose();

    expect(mounts[0]?.unmount).toHaveBeenCalledTimes(1);
    expect(mounts[1]?.unmount).toHaveBeenCalledTimes(1);
    expect(renderer.dispose).not.toHaveBeenCalled();
  });

  test('renders the Cube activation control as a labeled Nodes 2.0 switch', () => {
    const renderer: NativeNodeCardRenderer = {
      mount: jest.fn(() => createMount()),
      dispose: jest.fn(),
    };
    const node: ComfyNode = { id: 'patch', type: 'MahiroCFG', mode: 4 };
    const onActivationChange = jest.fn();
    const root = document.createElement('div');
    const host = new NativeNodeCardHost(renderer);

    host.mount(
      root,
      [
        {
          ...card(node),
          enabled: false,
          showActivationControl: true,
        },
      ],
      onActivationChange,
    );

    const nativeTarget = root.querySelector('.sugarcubes-cube-face__native-card-mount');
    const activation = root.querySelector<HTMLElement>('[data-cube-card-activation="patch"]');
    const input = activation?.querySelector<HTMLInputElement>('[role="switch"]');
    expect(nativeTarget?.contains(activation)).toBe(false);
    expect(activation?.textContent).toContain('Disabled');
    expect(input?.checked).toBe(false);
    expect(input?.getAttribute('aria-checked')).toBe('false');
    const switchRoot = activation?.querySelector<HTMLElement>('.p-toggleswitch');
    expect(switchRoot?.style.position).toBe('relative');
    expect(switchRoot?.classList.contains('p-toggleswitch-checked')).toBe(false);
    expect(activation?.querySelector('.p-toggleswitch-slider')).not.toBeNull();
    expect(activation?.querySelector('.p-toggleswitch-handle')).not.toBeNull();
    if (!input) throw new Error('Missing Nodes 2.0 activation switch.');
    input.checked = true;
    input.dispatchEvent(new Event('change'));
    expect(activation?.textContent).toContain('Enabled');
    expect(switchRoot?.classList.contains('p-toggleswitch-checked')).toBe(true);
    expect(onActivationChange).toHaveBeenCalledWith(node, true);
  });
});

/** Build one visible card decision around an exact graph node. */
function card(node: ComfyNode) {
  return {
    id: String(node.id),
    node,
    label: node.type ?? '',
    visible: true,
    enabled: true,
    showActivationControl: false,
  };
}

function createMount(): NativeNodeCardMount {
  return {
    refresh: jest.fn(),
    unmount: jest.fn(),
  };
}

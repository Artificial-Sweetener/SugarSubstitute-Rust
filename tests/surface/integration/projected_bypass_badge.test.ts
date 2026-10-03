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
/** Prove masonry suppresses only native bypass labels while preserving activation state. */
import { afterEach, expect, test } from '@jest/globals';
import { ComfyVueNodeCardRenderer } from '../../../frontend/comfyui/ui/surface/ComfyVueNodeCardRenderer.js';
import { ensureWorkflowMasonryStyles } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryStyles.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';

afterEach(() => {
  document.head.replaceChildren();
  document.body.replaceChildren();
});

/** Model mounted native component roots with their validated presentation props. */
function component(
  name: string,
  el: HTMLElement,
  props: UnknownRecord,
  children: UnknownRecord[] = [],
): UnknownRecord {
  return { type: { __name: name }, component: { props, subTree: { el, children } } };
}

test.each([false, true])(
  'hides the bypass label only in masonry=%s and reconciles a reused muted badge',
  (masonry) => {
    ensureWorkflowMasonryStyles(document);
    const surface = document.createElement('section');
    if (masonry) surface.className = 'sugarcubes-workflow-masonry';
    const target = document.createElement('div');
    surface.append(target);
    document.body.append(surface);
    const root = document.createElement('div');
    root.className = 'lg-node';
    const header = document.createElement('div');
    header.dataset.testid = 'node-header-test';
    const bypass = document.createElement('div');
    bypass.textContent = 'Bypassed';
    const price = document.createElement('div');
    price.textContent = 'Credits';
    const custom = document.createElement('div');
    custom.textContent = 'Bypassed';
    const switchInput = document.createElement('input');
    switchInput.type = 'checkbox';
    switchInput.setAttribute('role', 'switch');
    header.append(bypass, price, switchInput);
    root.append(header, custom);
    const props = { cssIcon: 'icon-[lucide--redo-dot]' };
    const tree = component('LGraphNode', root, {}, [
      component('NodeHeader', header, {}, [
        component('NodeBadge', bypass, props),
        component('NodeBadge', price, {}),
      ]),
      component('NodeBadge', custom, props),
    ]);
    const node = { id: 'test', type: 'LoraLoader', mode: 4 };
    const renderer = new ComfyVueNodeCardRenderer({
      component: {},
      appContext: {},
      runtime: {
        h: () => ({}),
        extractVueNodeData: () => ({ id: 'test', mode: node.mode }),
        render: (vnode) => {
          if (vnode !== null) {
            target.append(root);
            Object.assign(target, { _vnode: tree });
          }
        },
      },
    });
    const mount = renderer.mount(target, node, { preserveNativeBody: true });
    expect(getComputedStyle(bypass).display).toBe(masonry ? 'none' : 'block');
    expect(getComputedStyle(price).display).toBe('block');
    expect(getComputedStyle(custom).display).toBe('block');
    expect(node.mode).toBe(4);
    expect(switchInput.isConnected).toBe(true);
    props.cssIcon = 'icon-[lucide--ban]';
    bypass.textContent = 'Muted';
    mount.refresh();
    expect(getComputedStyle(bypass).display).toBe('block');
    expect(bypass.hasAttribute('data-masonry-bypass-badge')).toBe(false);
    expect(node.mode).toBe(4);
    renderer.dispose();
  },
);

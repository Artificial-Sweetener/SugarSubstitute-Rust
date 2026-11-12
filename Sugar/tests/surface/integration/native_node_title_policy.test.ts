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
/** Prove native title editing respects the projected card's surface policy. */
import { expect, jest, test } from '@jest/globals';
import { ComfyVueNodeCardRenderer } from '../../../frontend/comfyui/ui/surface/ComfyVueNodeCardRenderer.js';
import { NativeNodeCardHost } from '../../../frontend/comfyui/ui/surface/NativeNodeCardHost.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

/** Mount native header handlers behind the real projected renderer boundary. */
function titleFixture(allowRename?: boolean, surface?: 'cube face' | 'workflow masonry') {
  const target = document.createElement('div');
  const node: ComfyNode = { id: 'title-policy', title: 'Original title' };
  const rename = jest.fn(() => {
    node.title = 'Renamed title';
  });
  const editWidget = jest.fn();
  const collapse = jest.fn();
  const renderer = new ComfyVueNodeCardRenderer({
    component: { __name: 'LGraphNode' },
    appContext: {},
    runtime: {
      h: () => ({}),
      extractVueNodeData: () => ({ id: node.id }),
      render: (vnode, mountTarget) => {
        if (vnode === null) return;
        const root = document.createElement('div');
        root.className = 'lg-node';
        const header = document.createElement('div');
        header.className = 'lg-node-header';
        header.addEventListener('dblclick', rename);
        const title = document.createElement('span');
        title.dataset.testid = 'node-title';
        title.textContent = node.title ?? '';
        const button = document.createElement('button');
        button.textContent = 'Collapse';
        button.addEventListener('click', collapse);
        header.append(title, button);
        const input = document.createElement('input');
        input.addEventListener('dblclick', editWidget);
        root.append(header, input);
        mountTarget.replaceChildren(root);
      },
    },
  });
  const options = {
    preserveNativeBody: true,
    ...(allowRename === undefined ? {} : { allowRename }),
  };
  const host = new NativeNodeCardHost(renderer);
  const card = {
    id: String(node.id),
    node,
    label: node.title ?? '',
    visible: true,
    enabled: true,
    showActivationControl: false,
  };
  if (surface === 'cube face') host.mount(target, [card], jest.fn());
  else if (surface === 'workflow masonry') host.mountCard(target, card, jest.fn());
  const mount = surface
    ? { refresh: () => host.refresh(), unmount: () => host.dispose() }
    : renderer.mount(target, node, options);
  return { target, node, rename, editWidget, collapse, mount };
}

test.each(['cube face', 'workflow masonry'] as const)(
  '%s title rows cannot rename while widgets and single-click header controls stay native',
  (surface) => {
    const fixture = titleFixture(undefined, surface);
    try {
      for (const selector of ['.lg-node-header', '[data-testid="node-title"]']) {
        fixture.target
          .querySelector(selector)
          ?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
      }
      expect(fixture.rename).not.toHaveBeenCalled();
      expect(fixture.node.title).toBe('Original title');
      fixture.target
        .querySelector('input')
        ?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
      fixture.target.querySelector('button')?.click();
      expect(fixture.editWidget).toHaveBeenCalledTimes(1);
      expect(fixture.collapse).toHaveBeenCalledTimes(1);
      fixture.mount.refresh();
      fixture.target
        .querySelector('[data-testid="node-title"]')
        ?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
      expect(fixture.rename).not.toHaveBeenCalled();
      const header = fixture.target.querySelector('.lg-node-header');
      fixture.mount.unmount();
      header?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
      expect(fixture.rename).toHaveBeenCalledTimes(1);
    } finally {
      fixture.mount.unmount();
    }
  },
);

test.each([undefined, true])(
  'graph cards retain native title editing with allowRename=%s',
  (allowRename) => {
    const fixture = titleFixture(allowRename);
    try {
      fixture.target
        .querySelector('[data-testid="node-title"]')
        ?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
      expect(fixture.rename).toHaveBeenCalledTimes(1);
      expect(fixture.node.title).toBe('Renamed title');
    } finally {
      fixture.mount.unmount();
    }
  },
);

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
/** Controlled native DOM transport for the cold physical measurement and layout owners. */
import type { EditorView } from '@codemirror/view';
import { ReorderMountedLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-layout.ts';
import type { MountedReorderTarget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-target.ts';
/** Only native DOM Range/rectangle transport is fake; the real cold owner performs every admission and resource-free read. */
export function physicalFixture() {
  const box = {
    kind: 'chip' as const,
    index: 4,
    left: 10,
    right: 30,
    top: 10,
    bottom: 30,
  };
  let connected = true,
    blocked = false,
    width = 100;
  const text = {
    nodeType: 3,
    data: 'xxxx',
    length: 4,
    childNodes: [],
    parentNode: null as unknown,
  };
  const inner = {
    nodeType: 1,
    childNodes: [],
    parentNode: null as unknown,
    closest: () => line,
    getClientRects: () => [{ left: 5, right: 35, top: 0, bottom: 40, width: 30, height: 40 }],
  };
  const line = {
    get isConnected() {
      return connected;
    },
    getBoundingClientRect: () => ({ left: 0, top: 0, width }),
  };
  const root = {
    nodeType: 1,
    childNodes: [text, inner],
    parentNode: null as unknown,
    get isConnected() {
      return connected;
    },
    closest: () => line,
    getClientRects: () => [{ ...box, width: 20, height: 20 }],
    getBoundingClientRect: () => ({ ...box, width: 20, height: 20 }),
    ownerDocument: null as unknown,
  };
  text.parentNode = root;
  inner.parentNode = root;
  const document = {
    createRange() {
      if (blocked) throw new Error('controlled-native-range-read');
      let node: unknown,
        offset = 0;
      return {
        setStart(next: unknown, at: number) {
          node = next;
          offset = at;
        },
        collapse() {},
        getClientRects() {
          const x = node === text ? 30 - offset * 5 : offset === 0 ? 30 : 10;
          return [{ left: x, top: 10, bottom: 30 }];
        },
      };
    },
    defaultView: {
      getComputedStyle: () => ({
        font: '14px Segoe UI',
        width: `${width}px`,
        direction: 'rtl',
        lineHeight: '20px',
      }),
    },
  };
  root.ownerDocument = document;
  const target: MountedReorderTarget = {
    node: root as unknown as HTMLElement,
    kind: 'chip',
    index: 4,
    leases: [],
    origins: [7],
  };
  const view = {
    themeClasses: 'native-theme',
    contentDOM: {},
    dom: { ownerDocument: document },
  } as unknown as EditorView;
  return {
    target,
    box,
    line,
    text,
    inner,
    owner: new ReorderMountedLayout(view),
    block() {
      blocked = true;
    },
    unblock() {
      blocked = false;
    },
    resize() {
      width++;
    },
    detach() {
      connected = false;
    },
    oversize() {
      text.length = 16385;
    },
  };
}

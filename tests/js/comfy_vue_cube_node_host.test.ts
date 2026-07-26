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
/** Verify one Nodes 2.0 host owns each native Cube node root. */

import { jest } from '@jest/globals';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { ComfyVueCubeNodeHost } from '../../frontend/comfyui/ui/surface/ComfyVueCubeNodeHost.js';

describe('ComfyVueCubeNodeHost', () => {
  test('tracks the native Cube colors as inherited card theme tokens', async () => {
    const { root, inner } = nativeCubeRoot('cube-node');
    inner.style.backgroundColor = '#2b2859';
    inner.style.setProperty('--component-node-background', 'rgba(32, 33, 39, 0.9)');
    const host = createHost();
    const node = cubeNode('cube-node');

    host.mount(node);

    expect(root.style.getPropertyValue('--sugarcubes-cube-card-header')).toBe('rgb(43, 40, 89)');
    expect(root.style.getPropertyValue('--sugarcubes-cube-card-body')).toBe(
      'rgba(32, 33, 39, 0.9)',
    );
    expect(root.style.getPropertyValue('--sugarcubes-cube-backdrop')).toBe('rgb(27 28 33 / 0.9)');

    inner.style.backgroundColor = '#43335c';
    inner.style.setProperty('--component-node-background', '#24202d');
    await Promise.resolve();
    await Promise.resolve();

    expect(root.style.getPropertyValue('--sugarcubes-cube-card-header')).toBe('rgb(67, 51, 92)');
    expect(root.style.getPropertyValue('--sugarcubes-cube-card-body')).toBe('#24202d');
    expect(root.style.getPropertyValue('--sugarcubes-cube-backdrop')).toBe('rgb(30 27 38)');

    host.unmount(node);

    expect(root.style.getPropertyValue('--sugarcubes-cube-card-header')).toBe('');
    expect(root.style.getPropertyValue('--sugarcubes-cube-card-body')).toBe('');
    expect(root.style.getPropertyValue('--sugarcubes-cube-backdrop')).toBe('');
  });

  test('replaces stale object-identity mounts without duplicating the native face host', () => {
    const { body } = nativeCubeRoot('cube-node');
    const host = createHost();
    const firstNode = cubeNode('cube-node');
    const secondNode = cubeNode('cube-node');

    const firstFace = host.mount(firstNode);
    const secondFace = host.mount(secondNode);

    expect(secondFace).not.toBe(firstFace);
    expect(firstFace?.isConnected).toBe(false);
    expect(body.querySelectorAll(':scope > [data-sugarcube-face-host]')).toHaveLength(1);
    host.dispose();
  });

  test('removes an orphaned face host before taking ownership of a native root', () => {
    const { body } = nativeCubeRoot('cube-node');
    const orphan = document.createElement('div');
    orphan.dataset.sugarcubeFaceHost = '';
    body.append(orphan);
    const host = createHost();

    const mounted = host.mount(cubeNode('cube-node'));
    const mountedHosts = body.querySelectorAll(':scope > [data-sugarcube-face-host]');

    expect(orphan.isConnected).toBe(false);
    expect(mountedHosts).toHaveLength(1);
    expect(mountedHosts.item(0)).toBe(mounted);
    host.dispose();
  });

  test('transfers native-root ownership across replaced runtime hosts', async () => {
    const { body } = nativeCubeRoot('cube-node');
    const firstHost = createHost();
    const secondHost = createHost();
    const firstFace = firstHost.mount(cubeNode('cube-node'));

    const secondFace = secondHost.mount(cubeNode('cube-node'));
    await Promise.resolve();

    const mountedHosts = body.querySelectorAll(':scope > [data-sugarcube-face-host]');
    expect(firstFace?.isConnected).toBe(false);
    expect(mountedHosts).toHaveLength(1);
    expect(mountedHosts.item(0)).toBe(secondFace);
    firstHost.dispose();
    secondHost.dispose();
  });

  test('mounts Cube chrome inside the native header drag surface', () => {
    const { root, header, nativeHeaderContent, body } = nativeCubeRoot('cube-node');
    const host = createHost();
    const node = cubeNode('cube-node');
    const cubeHeader = document.createElement('header');
    const projectedChildHeader = document.createElement('div');
    projectedChildHeader.dataset.testid = 'node-header-projected-child';
    body.prepend(projectedChildHeader);
    const pointerDown = jest.fn();
    root.addEventListener('pointerdown', pointerDown);

    host.mount(node);

    expect(host.mountHeader(node, cubeHeader)).toBe(true);
    expect(cubeHeader.parentElement).toBe(header);
    expect(header.hidden).toBe(false);
    expect(header.hasAttribute('data-sugarcube-native-hidden')).toBe(false);
    expect(projectedChildHeader.contains(cubeHeader)).toBe(false);
    expect(nativeHeaderContent.hidden).toBe(true);
    expect(body.dataset.sugarcubeCubeBody).toBe('');

    cubeHeader.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true }));

    expect(pointerDown).toHaveBeenCalledTimes(1);
    host.unmount(node);
    expect(cubeHeader.isConnected).toBe(false);
    expect(nativeHeaderContent.hidden).toBe(false);
    expect(body.dataset.sugarcubeCubeBody).toBeUndefined();
  });

  test('adopts the native Nodes 2.0 footer as the Cube editor entry point', async () => {
    const { root, footerButton, footerLabel } = nativeCubeRoot('cube-node');
    const openEditor = jest.fn();
    const nativeEnterSubgraph = jest.fn();
    footerButton.addEventListener('click', nativeEnterSubgraph);
    const host = createHost(openEditor);
    const node = cubeNode('cube-node');

    host.mount(node);

    expect(footerButton.hidden).toBe(false);
    expect(footerLabel.textContent).toBe('Edit Cube');
    expect(footerButton.getAttribute('aria-label')).toBe('Edit Cube');

    const footerMutations = jest.fn();
    const observer = new MutationObserver(footerMutations);
    observer.observe(footerLabel, { childList: true, subtree: true });
    root.append(document.createElement('div'));
    await Promise.resolve();
    await Promise.resolve();
    expect(footerMutations).not.toHaveBeenCalled();
    observer.disconnect();

    footerButton.click();

    expect(openEditor).toHaveBeenCalledWith(node);
    expect(nativeEnterSubgraph).not.toHaveBeenCalled();

    host.unmount(node);
    expect(footerLabel.textContent).toBe('Enter Subgraph');
    expect(footerButton.hasAttribute('aria-label')).toBe(false);
  });

  test('keeps the Cube editor action live while Vue replaces the footer subtree', () => {
    const { footerButton } = nativeCubeRoot('cube-node');
    const openEditor = jest.fn();
    const host = createHost(openEditor);
    const node = cubeNode('cube-node');
    host.mount(node);
    const replacement = createFooterButton();

    footerButton.replaceWith(replacement.button);
    replacement.button.click();

    expect(openEditor).toHaveBeenCalledWith(node);
    host.dispose();
  });

  test('applies measured face height to the real native Cube node', () => {
    nativeCubeRoot('cube-node');
    const host = createHost();
    const node = cubeNode('cube-node');
    node.size = [640, 240];
    node.setSize = jest.fn();
    node.onResize = jest.fn();
    host.mount(node);

    expect(host.reconcileMinimumSize(node, 475.2)).toBe(true);
    expect([...node.size]).toEqual([640, 476]);
    expect(node.setSize).toHaveBeenCalledWith([640, 476]);
    expect(host.reconcileMinimumSize(node, 475.2)).toBe(false);

    host.dispose();
  });

  test('remeasures output anchors after Cube preview titles mount', () => {
    const { root, body } = nativeCubeRoot('cube-node');
    const row = body.firstElementChild as HTMLElement;
    const output = document.createElement('div');
    output.className = 'lg-slot lg-slot--output';
    row.append(output);
    setRect(root, { top: 0, left: 0, width: 500, height: 340 });
    setRect(body, { top: 30, left: 0, width: 500, height: 300 });
    setRect(row, { top: 30, left: 0, width: 500, height: 300 });
    Object.defineProperty(body, 'offsetWidth', { configurable: true, value: 500 });
    const requestSlotLayoutSync = jest.fn();
    const host = new ComfyVueCubeNodeHost({
      document,
      titleHeight: 30,
      history: {},
      getScale: () => 1,
      openEditor: () => undefined,
      requestSlotLayoutSync,
    });
    const node = cubeNode('cube-node');
    const face = host.mount(node);
    const title = document.createElement('div');
    title.dataset.cubePreviewOutputTitle = '';
    setRect(title, { top: 100, left: 300, width: 100, height: 20 });
    face?.append(title);

    host.reconcileBoundary(node);

    expect(output.style.getPropertyValue('--sugarcube-boundary-position')).toBe('80px');
    expect(requestSlotLayoutSync).toHaveBeenCalledTimes(2);
    host.dispose();
  });
});

/** Create one host with inert graph collaborators. */
function createHost(openEditor: (node: CubeNode) => void = () => undefined): ComfyVueCubeNodeHost {
  return new ComfyVueCubeNodeHost({
    document,
    titleHeight: 30,
    history: {},
    getScale: () => 1,
    openEditor,
    requestSlotLayoutSync: () => undefined,
  });
}

/** Mount the minimum native Nodes 2.0 shell consumed by the host seam. */
function nativeCubeRoot(id: string): {
  root: HTMLDivElement;
  inner: HTMLDivElement;
  header: HTMLDivElement;
  nativeHeaderContent: HTMLDivElement;
  body: HTMLDivElement;
  footerButton: HTMLButtonElement;
  footerLabel: HTMLSpanElement;
} {
  document.body.replaceChildren();
  const root = document.createElement('div');
  root.className = 'lg-node';
  root.dataset.nodeId = id;
  const inner = document.createElement('div');
  inner.dataset.testid = 'node-inner-wrapper';
  const header = document.createElement('div');
  header.dataset.testid = `node-header-${id}`;
  const nativeHeaderContent = document.createElement('div');
  nativeHeaderContent.textContent = 'Native title';
  header.append(nativeHeaderContent);
  const body = document.createElement('div');
  body.dataset.testid = `node-body-${id}`;
  const slotRow = document.createElement('div');
  const slot = document.createElement('div');
  slot.className = 'lg-slot lg-slot--input';
  slotRow.append(slot);
  body.append(slotRow);
  const footer = document.createElement('div');
  const { button: footerButton, label: footerLabel } = createFooterButton();
  footer.append(footerButton);
  inner.append(header, body);
  root.append(inner, footer);
  document.body.append(root);
  return { root, inner, header, nativeHeaderContent, body, footerButton, footerLabel };
}

/** Create one native footer subtree that Comfy may replace independently. */
function createFooterButton(): {
  button: HTMLButtonElement;
  label: HTMLSpanElement;
} {
  const button = document.createElement('button');
  button.dataset.testid = 'subgraph-enter-button';
  const label = document.createElement('span');
  label.className = 'truncate';
  label.textContent = 'Enter Subgraph';
  button.append(label);
  return { button, label };
}

/** Create two distinct graph objects representing one restored Cube identity. */
function cubeNode(id: string): CubeNode {
  return {
    id,
    title: 'Cube',
    pos: [0, 0],
    size: [640, 420],
    inputs: [{ name: 'input', type: 'IMAGE', link: null }],
    outputs: [{ name: 'output', type: 'IMAGE', links: null }],
    properties: {},
    subgraph: {
      id: `${id}-definition`,
      name: 'Cube',
      _nodes: [],
      inputs: [],
      outputs: [],
      inputNode: { slots: [] },
      outputNode: { slots: [] },
      add(): void {},
      remove(): void {},
      addInput(): void {},
      addOutput(): void {},
      configure(): void {},
    },
    isSubgraphNode: () => true,
    connect(): void {},
    serialize: () => ({}),
  } as unknown as CubeNode;
}

/** Give one JSDOM element finite viewport geometry. */
function setRect(
  element: HTMLElement,
  rect: { top: number; left: number; width: number; height: number },
): void {
  element.getBoundingClientRect = () =>
    ({
      x: rect.left,
      y: rect.top,
      top: rect.top,
      left: rect.left,
      width: rect.width,
      height: rect.height,
      right: rect.left + rect.width,
      bottom: rect.top + rect.height,
      toJSON: () => ({}),
    }) as DOMRect;
}

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
/** Verify Cube presentation replaces content inside a real native node root. */

import { jest } from '@jest/globals';
import type { NativeCubeSubgraph } from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import { CubePortPresentationController } from '../../frontend/comfyui/ui/cube/connection/CubePortPresentationController.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { CubeSurfacePresenter } from '../../frontend/comfyui/ui/surface/CubeSurfacePresenter.js';
import type {
  NativeNodeCardMount,
  NativeNodeCardRenderer,
} from '../../frontend/comfyui/ui/surface/NativeNodeCardRenderer.js';

describe('CubeSurfacePresenter', () => {
  beforeEach(() => {
    jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(() => null);
  });

  afterEach(() => {
    jest.restoreAllMocks();
  });

  test('mounts the custom face inside the real node while preserving shell edges and controls', async () => {
    const pane = createTransformPane();
    const shell = createNativeNodeShell('9');
    pane.append(shell.root);
    const rootGraph = {};
    const internalNode = nativeNode('inner', 'KSampler');
    const node = cubeNode(9, 'cube-1', [internalNode]);
    const nodes = new CubeNodeCatalog();
    nodes.add(node);
    const mount = jest.fn<
      (target: HTMLElement, targetNode: typeof internalNode) => NativeNodeCardMount
    >((target) => {
      const card = document.createElement('section');
      card.dataset.nativeCard = '';
      target.append(card);
      return { refresh() {}, unmount() {} };
    });
    const openEditor = jest.fn();
    const onBoundaryGeometryChange = jest.fn();
    const presenter = new CubeSurfacePresenter({
      document,
      openEditor,
      rootGraph,
      getCurrentGraph: () => rootGraph,
      nodes,
      logger: console,
      renderer: { mount, dispose() {} },
      requestSlotLayoutSync: () => undefined,
      onBoundaryGeometryChange,
    });

    await flushMount();

    const faceHost = shell.root.querySelector<HTMLElement>('[data-sugarcube-face-host]');
    expect(faceHost).not.toBeNull();
    expect(faceHost?.closest('.lg-node')).toBe(shell.root);
    expect(pane.querySelectorAll(':scope > .sugarcubes-cube-container')).toHaveLength(0);
    expect(shell.root.dataset.sugarcubeNode).toBe('true');
    expect(shell.root.querySelector('[data-native-border]')).not.toBeNull();
    expect(shell.root.querySelector('[data-native-resize]')).not.toBeNull();
    expect(shell.header.hidden).toBe(false);
    expect(shell.header.querySelector<HTMLElement>('.sugarcubes-cube-face__header')).not.toBeNull();
    expect(shell.slot.hidden).toBe(false);
    expect(shell.slot.dataset.sugarcubeBoundaryDirection).toBe('input');
    expect(shell.slot.closest('[data-sugarcube-boundary-row]')).not.toBeNull();
    expect(shell.genericContent.hidden).toBe(true);
    expect(shell.root.querySelectorAll('[data-sugarcube-edge-resize]')).toHaveLength(4);
    expect(faceHost?.querySelector('[data-cube-resize-edge]')).toBeNull();
    expect(faceHost?.querySelector('[data-cube-port-direction]')).toBeNull();
    expect(mount).toHaveBeenCalledWith(expect.any(HTMLElement), internalNode);
    expect(onBoundaryGeometryChange).toHaveBeenCalledTimes(1);

    const replacementHeader = document.createElement('div');
    replacementHeader.dataset.testid = `node-header-${String(node.id)}`;
    shell.header.replaceWith(replacementHeader);
    shell.root.querySelector<HTMLElement>('[data-sugarcube-edge-resize]')?.remove();
    await flushMount();
    expect(replacementHeader.hidden).toBe(false);
    expect(
      replacementHeader.querySelector<HTMLElement>('.sugarcubes-cube-face__header'),
    ).not.toBeNull();
    expect(shell.root.querySelectorAll('[data-sugarcube-edge-resize]')).toHaveLength(4);

    shell.footerButton.click();
    expect(openEditor).toHaveBeenCalledWith(node);
    presenter.dispose();
    expect(shell.root.querySelector('[data-sugarcube-edge-resize]')).toBeNull();
    expect(replacementHeader.hidden).toBe(false);
  });

  test('preserves the face origin through movement, menu cycles, and Cube editor return', async () => {
    jest.useFakeTimers();
    const pane = createTransformPane();
    const optionalNode = { ...nativeNode('optional', 'MahiroCFG'), mode: 4 };
    const node = cubeNode(9, 'cube-1', [nativeNode('inner', 'KSampler'), optionalNode]);
    const shell = createNativeNodeShell(String(node.id));
    pane.append(shell.root);
    const nodes = new CubeNodeCatalog();
    nodes.add(node);
    const rootGraph = {};
    let currentGraph: object = rootGraph;
    const unmount = jest.fn();
    const presenter = new CubeSurfacePresenter({
      document,
      openEditor: jest.fn(),
      rootGraph,
      getCurrentGraph: () => currentGraph,
      nodes,
      logger: console,
      renderer: {
        mount: (target) => {
          Object.defineProperty(target.parentElement, 'offsetHeight', {
            configurable: true,
            value: 100,
          });
          return { refresh() {}, unmount };
        },
        dispose() {},
      },
      requestSlotLayoutSync: () => undefined,
    });
    await flushMount();
    const firstHost = shell.root.querySelector<HTMLElement>('[data-sugarcube-face-host]');
    const firstContent = firstHost?.querySelector<HTMLElement>('[data-cube-content]');
    const firstCard = firstHost?.querySelector<HTMLElement>('[data-cube-node-id="inner"]');
    const firstMenuButton = shell.header.querySelector<HTMLButtonElement>(
      '[data-cube-action="card-menu"]',
    );
    if (!firstHost || !firstContent || !firstCard || !firstMenuButton) {
      throw new Error('Missing initial Cube face invariants.');
    }
    const firstGeometry = readMountedFaceGeometry(firstContent, firstCard);

    node.pos = [320, 240];
    jest.advanceTimersByTime(100);
    expect(readMountedFaceGeometry(firstContent, firstCard)).toEqual(firstGeometry);

    node.size = [880, 640];
    jest.advanceTimersByTime(100);
    const resizedGeometry = readMountedFaceGeometry(firstContent, firstCard);
    firstMenuButton.click();
    firstMenuButton.click();

    expect(firstHost.parentElement).toBe(shell.body);
    expect(shell.header.contains(firstHost)).toBe(false);
    expect(readMountedFaceGeometry(firstContent, firstCard)).toEqual(resizedGeometry);

    currentGraph = node.subgraph;
    presenter.refresh();
    expect(shell.root.querySelector('[data-sugarcube-face-host]')).toBeNull();
    expect(shell.genericContent.hidden).toBe(false);
    expect(unmount).toHaveBeenCalledTimes(1);

    currentGraph = rootGraph;
    presenter.refresh();
    await flushMount();
    const remountedHost = shell.root.querySelector<HTMLElement>('[data-sugarcube-face-host]');
    const remountedContent = remountedHost?.querySelector<HTMLElement>('[data-cube-content]');
    const remountedCard = remountedHost?.querySelector<HTMLElement>('[data-cube-node-id="inner"]');
    const remountedMenuButton = shell.header.querySelector<HTMLButtonElement>(
      '[data-cube-action="card-menu"]',
    );
    if (!remountedHost || !remountedContent || !remountedCard || !remountedMenuButton) {
      throw new Error('Missing remounted Cube face invariants.');
    }
    const remountedGeometry = readMountedFaceGeometry(remountedContent, remountedCard);
    remountedMenuButton.click();
    remountedMenuButton.click();

    expect(remountedHost.parentElement).toBe(shell.body);
    expect(shell.header.contains(remountedHost)).toBe(false);
    expect(readMountedFaceGeometry(remountedContent, remountedCard)).toEqual(remountedGeometry);

    presenter.dispose();
    jest.useRealTimers();
  });

  test('remounts exact native child cards after internal topology changes', async () => {
    const pane = createTransformPane();
    const first = nativeNode('first', 'First');
    const second = nativeNode('second', 'Second');
    const node = cubeNode(9, 'cube-1', [first]);
    pane.append(createNativeNodeShell(String(node.id)).root);
    const nodes = new CubeNodeCatalog();
    nodes.add(node);
    const rootGraph = {};
    const mount = jest.fn(() => ({ refresh() {}, unmount() {} }));
    const presenter = new CubeSurfacePresenter({
      document,
      openEditor: jest.fn(),
      rootGraph,
      getCurrentGraph: () => rootGraph,
      nodes,
      logger: console,
      renderer: { mount, dispose() {} },
      requestSlotLayoutSync: () => undefined,
    });
    await flushMount();

    node.subgraph._nodes = [second];
    node.subgraph.onAfterChange?.(node.subgraph);
    await flushMount();

    expect(mount).toHaveBeenLastCalledWith(expect.any(HTMLElement), second);
    presenter.dispose();
  });

  test('routes Cube face titlebar swap actions through the mounted native node face', async () => {
    const pane = createTransformPane();
    const node = cubeNode(9, 'cube-1', [nativeNode('inner', 'KSampler')]);
    pane.append(createNativeNodeShell(String(node.id)).root);
    const nodes = new CubeNodeCatalog();
    nodes.add(node);
    const onSwapLeft = jest.fn();
    const rootGraph = {};
    const activePresenter = new CubeSurfacePresenter({
      document,
      openEditor: jest.fn(),
      rootGraph,
      getCurrentGraph: () => rootGraph,
      nodes,
      logger: console,
      renderer: {
        mount: () => ({ refresh() {}, unmount() {} }),
        dispose() {},
      },
      requestSlotLayoutSync: () => undefined,
      chromeActions: {
        onSwapLeft,
        canSwap: (_metadata, direction) => direction === 'left',
      },
    });
    await flushMount();

    document.querySelector<HTMLButtonElement>('[data-cube-action="swap-left"]')?.click();

    expect(onSwapLeft).toHaveBeenCalledWith({
      instance_id: 'cube-1',
      cube_id: 'example.cube',
    });
    activePresenter.dispose();
  });

  test('performs no Cube DOM work while graph and renderer state remain idle', async () => {
    jest.useFakeTimers();
    const pane = createTransformPane();
    const node = cubeNode(9, 'cube-1', [nativeNode('inner', 'KSampler')]);
    const shell = createNativeNodeShell(String(node.id));
    pane.append(shell.root);
    const nodes = new CubeNodeCatalog();
    nodes.add(node);
    const rootGraph = {};
    const mount = jest.fn(() => ({ refresh() {}, unmount() {} }));
    const presenter = new CubeSurfacePresenter({
      document,
      openEditor: jest.fn(),
      rootGraph,
      getCurrentGraph: () => rootGraph,
      nodes,
      logger: console,
      renderer: { mount, dispose() {} },
      requestSlotLayoutSync: () => undefined,
    });
    await flushMount();
    const observer = new MutationObserver(jest.fn());
    observer.observe(shell.root, {
      attributes: true,
      childList: true,
      subtree: true,
    });

    jest.advanceTimersByTime(1_000);
    await flushMount();

    expect(observer.takeRecords()).toEqual([]);
    expect(mount).toHaveBeenCalledTimes(1);
    observer.disconnect();
    presenter.dispose();
    jest.useRealTimers();
  });

  test('releases Nodes 2 port geometry before Nodes 1 registers its canvas geometry', async () => {
    const pane = createTransformPane();
    const node = cubeNode(9, 'cube-1', [nativeNode('inner', 'KSampler')]);
    pane.append(createNativeNodeShell(String(node.id)).root);
    const nodes = new CubeNodeCatalog();
    nodes.add(node);
    const rootGraph = {};
    let rendererMode: 'vue' | 'litegraph' = 'vue';
    const portPresentation = new CubePortPresentationController({
      requestFrame: () => null,
    });
    const release = jest.spyOn(portPresentation, 'release');
    const register = jest.spyOn(portPresentation, 'register');
    const canvasElement = document.createElement('canvas');
    const presenter = new CubeSurfacePresenter({
      document,
      openEditor: jest.fn(),
      rootGraph,
      getCurrentGraph: () => rootGraph,
      nodes,
      logger: console,
      renderer: {
        mount: () => ({ refresh() {}, unmount() {} }),
        dispose() {},
      },
      getRendererMode: () => rendererMode,
      legacyCanvas: {
        canvas: canvasElement,
        graph: rootGraph,
        graph_mouse: [0, 0],
        drawNode: jest.fn(),
        processWidgetClick: jest.fn(),
        setDirty: jest.fn(),
      },
      portPresentation,
      requestSlotLayoutSync: () => undefined,
    });
    await flushMount();
    release.mockClear();
    register.mockClear();

    rendererMode = 'litegraph';
    presenter.refresh();

    expect(release).toHaveBeenCalledWith(node);
    expect(register).toHaveBeenCalledWith(node, 'input', expect.any(Array));
    expect(release.mock.invocationCallOrder[0]).toBeLessThan(register.mock.invocationCallOrder[0]!);
    presenter.dispose();
  });

  test('creates a fresh Nodes 2 renderer after returning from Nodes 1', async () => {
    const pane = createTransformPane();
    const node = cubeNode(9, 'cube-1', [nativeNode('inner', 'KSampler')]);
    pane.append(createNativeNodeShell(String(node.id)).root);
    const nodes = new CubeNodeCatalog();
    nodes.add(node);
    const rootGraph = {};
    let rendererMode: 'vue' | 'litegraph' = 'vue';
    const firstMount = jest.fn(() => ({ refresh() {}, unmount() {} }));
    const secondMount = jest.fn(() => ({ refresh() {}, unmount() {} }));
    const firstRenderer: NativeNodeCardRenderer = {
      mount: firstMount,
      dispose: jest.fn(),
    };
    const secondRenderer: NativeNodeCardRenderer = {
      mount: secondMount,
      dispose: jest.fn(),
    };
    const rendererQueue = [firstRenderer, secondRenderer];
    const createRenderer = jest.fn(async () => {
      const renderer = rendererQueue.shift();
      if (!renderer) throw new Error('Unexpected renderer request.');
      return renderer;
    });
    const canvasElement = document.createElement('canvas');
    const presenter = new CubeSurfacePresenter({
      document,
      openEditor: jest.fn(),
      rootGraph,
      getCurrentGraph: () => rootGraph,
      nodes,
      logger: console,
      createRenderer,
      getRendererMode: () => rendererMode,
      legacyCanvas: {
        canvas: canvasElement,
        graph: rootGraph,
        graph_mouse: [0, 0],
        drawNode: jest.fn(),
        processWidgetClick: jest.fn(),
        setDirty: jest.fn(),
      },
      requestSlotLayoutSync: () => undefined,
    });
    await flushMount();
    expect(firstMount).toHaveBeenCalled();

    rendererMode = 'litegraph';
    presenter.refresh();
    await flushMount();
    rendererMode = 'vue';
    presenter.refresh();
    await flushMount();

    expect(firstRenderer.dispose).toHaveBeenCalledTimes(1);
    expect(createRenderer).toHaveBeenCalledTimes(2);
    expect(secondMount).toHaveBeenCalled();
    presenter.dispose();
  });
});

/** Add the exact native Nodes 2.0 transform surface used by the host adapter. */
function createTransformPane(): HTMLDivElement {
  document.body.replaceChildren();
  const pane = document.createElement('div');
  pane.dataset.testid = 'transform-pane';
  document.body.append(pane);
  return pane;
}

/** Build a native Nodes 2.0 shell with border, slot, body, and resize ownership. */
function createNativeNodeShell(id: string): {
  root: HTMLDivElement;
  header: HTMLDivElement;
  slot: HTMLDivElement;
  genericContent: HTMLDivElement;
  body: HTMLDivElement;
  footerButton: HTMLButtonElement;
} {
  const root = document.createElement('div');
  root.className = 'lg-node';
  root.dataset.nodeId = id;
  const border = document.createElement('div');
  border.dataset.nativeBorder = '';
  const inner = document.createElement('div');
  inner.dataset.testid = 'node-inner-wrapper';
  const header = document.createElement('div');
  header.dataset.testid = `node-header-${id}`;
  const body = document.createElement('div');
  body.dataset.testid = `node-body-${id}`;
  const slotRow = document.createElement('div');
  const slot = document.createElement('div');
  slot.className = 'lg-slot lg-slot--input';
  const slotDot = document.createElement('button');
  slotDot.dataset.nodeId = id;
  slotDot.dataset.slotIndex = '0';
  slot.append(slotDot);
  const genericContent = document.createElement('div');
  genericContent.dataset.nativeGenericContent = '';
  slotRow.append(slot);
  body.append(slotRow, genericContent);
  inner.append(header, body);
  const footer = document.createElement('div');
  const footerButton = document.createElement('button');
  footerButton.dataset.testid = 'subgraph-enter-button';
  const footerLabel = document.createElement('span');
  footerLabel.className = 'truncate';
  footerLabel.textContent = 'Enter Subgraph';
  footerButton.append(footerLabel);
  footer.append(footerButton);
  const resize = document.createElement('div');
  resize.dataset.nativeResize = '';
  resize.setAttribute('role', 'button');
  root.append(border, inner, footer, resize);
  return { root, header, slot, genericContent, body, footerButton };
}

/** Build one real-object-shaped internal node for renderer identity assertions. */
function nativeNode(id: string, type: string) {
  return {
    id,
    type,
    pos: [0, 0],
    size: [240, 180],
    properties: {},
    inputs: [],
    outputs: [],
    widgets: [{ name: 'value' }],
    connect() {},
  };
}

/** Build one real SubgraphNode-shaped Cube instance. */
function cubeNode(
  id: number,
  instanceId: string,
  innerNodes: ReturnType<typeof nativeNode>[],
): CubeNode {
  const subgraph = {
    id: `definition-${instanceId}`,
    name: 'Example Cube',
    _nodes: innerNodes,
    inputs: [],
    outputs: [],
    inputNode: {},
    outputNode: {},
    onAfterChange: null,
    add() {},
    remove() {},
    addInput() {
      throw new Error('not used');
    },
    addOutput() {
      throw new Error('not used');
    },
    configure() {},
  } as unknown as NativeCubeSubgraph;
  return {
    id,
    type: subgraph.id,
    title: 'Example Cube',
    pos: [40, 60],
    size: [760, 520],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: instanceId, cube_id: 'example.cube' },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph,
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}

/** Flush async renderer resolution and view composition. */
async function flushMount(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

/** Snapshot mounted face geometry that transient header UI cannot influence. */
function readMountedFaceGeometry(content: HTMLElement, card: HTMLElement): object {
  return {
    contentMinHeight: content.style.minHeight,
    cardLeft: card.style.left,
    cardTop: card.style.top,
    cardWidth: card.style.width,
  };
}

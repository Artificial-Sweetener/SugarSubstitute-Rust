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
/** Verify a SimpleSyrup detail preview reaches the mounted Cube output rail. */

import { jest } from '@jest/globals';
import type { NativeCubeSubgraph } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { ComfyCubePreviewCatalog } from '../../../frontend/comfyui/ui/surface/ComfyCubePreviewCatalog.js';
import { CubeSurfacePresenter } from '../../../frontend/comfyui/ui/surface/CubeSurfacePresenter.js';

describe('SimpleSyrup Cube preview surface', () => {
  afterEach(() => {
    jest.restoreAllMocks();
    document.body.replaceChildren();
  });

  test('renders and replaces the full-context detail preview at the Cube boundary', async () => {
    jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(() => null);
    const shell = createNativeNodeShell('9');
    document.body.append(shell);
    const detailer = nativeNode('detailer', 'SimpleSyrup.DetailSEGSByScaleFactor');
    detailer.outputs = [{ links: ['detail-to-color'] }];
    const color = nativeNode('color', 'SimpleSyrup.VectorScopeCC');
    color.inputs = [{ link: 'detail-to-color' }];
    const cube = cubeNode(9, detailer, color);
    const nodes = new CubeNodeCatalog();
    nodes.add(cube);
    let detailPreviews = ['blob:simple-syrup-full-context-detail-preview-1'];
    const previewEvents = new EventTarget();
    const rootGraph = {};
    const previewCatalog = new ComfyCubePreviewCatalog({
      getNodeOutputs: () => ({}),
      getNodePreviewImages: () => ({ '9:detailer': detailPreviews }),
      buildOutputImageUrl: () => null,
    });
    const presenter = new CubeSurfacePresenter({
      document,
      openEditor: jest.fn(),
      rootGraph,
      getCurrentGraph: () => rootGraph,
      nodes,
      logger: console,
      renderer: { mount: () => ({ refresh() {}, unmount() {} }), dispose() {} },
      previewCatalog,
      previewEvents,
    });

    await flushMount();

    const firstFigure = shell.querySelector<HTMLElement>('[data-cube-preview-item]');
    const firstImage = firstFigure?.querySelector<HTMLImageElement>('img');
    expect(firstFigure?.dataset.cubePreviewItem).toBe('9:detailer:preview:0');
    expect(firstImage?.getAttribute('src')).toBe('blob:simple-syrup-full-context-detail-preview-1');

    detailPreviews = ['blob:simple-syrup-full-context-detail-preview-2'];
    previewEvents.dispatchEvent(
      new CustomEvent('executed', { detail: { node: '9:detailer', output: {} } }),
    );

    const figures = shell.querySelectorAll<HTMLElement>('[data-cube-preview-item]');
    expect(figures).toHaveLength(1);
    expect(figures[0]?.querySelector('img')?.getAttribute('src')).toBe(
      'blob:simple-syrup-full-context-detail-preview-2',
    );
    presenter.dispose();
  });
});

/** Build a native Nodes 2.0 shell recognized by the production host adapter. */
function createNativeNodeShell(id: string): HTMLDivElement {
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
  const genericContent = document.createElement('div');
  genericContent.dataset.nativeGenericContent = '';
  body.append(genericContent);
  inner.append(header, body);
  const footer = document.createElement('div');
  const enter = document.createElement('button');
  enter.dataset.testid = 'subgraph-enter-button';
  const label = document.createElement('span');
  label.className = 'truncate';
  label.textContent = 'Enter Subgraph';
  enter.append(label);
  footer.append(enter);
  const resize = document.createElement('div');
  resize.dataset.nativeResize = '';
  resize.setAttribute('role', 'button');
  root.append(border, inner, footer, resize);
  return root;
}

/** Build an internal Comfy node with mutable native link slots. */
function nativeNode(id: string, type: string) {
  return {
    id,
    type,
    pos: [0, 0],
    size: [240, 180],
    properties: {},
    inputs: [] as { link: string }[],
    outputs: [] as { links: string[] }[],
    widgets: [],
    connect() {},
  };
}

/** Build a native Cube whose output is fed through a post-detail color node. */
function cubeNode(
  id: number,
  detailer: ReturnType<typeof nativeNode>,
  color: ReturnType<typeof nativeNode>,
): CubeNode {
  const boundaryLink = { resolve: () => ({ outputNode: color }) };
  const subgraph = {
    id: 'automask-detailer-definition',
    name: 'Automask Detailer',
    _nodes: [detailer, color],
    inputs: [],
    outputs: [{ name: 'output.image', type: 'IMAGE' }],
    inputNode: {},
    outputNode: { slots: [{ getLinks: () => [boundaryLink] }] },
    onAfterChange: null,
    add() {},
    remove() {},
    addInput() {},
    addOutput() {},
    configure() {},
  } as unknown as NativeCubeSubgraph;
  return {
    id,
    type: subgraph.id,
    title: 'Automask Detailer',
    pos: [40, 60],
    size: [760, 520],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: {
        instance_id: 'automask-detailer-instance',
        cube_id: 'automask.detailer',
      },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [{ name: 'output.image', type: 'IMAGE' }],
    subgraph,
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}

/** Flush the async renderer and Cube-face mount work. */
async function flushMount(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

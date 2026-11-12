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
/** Prove legacy Cube preview painting reads the active palette on every frame. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { ComfyLiteGraphCubeRenderer } from '../../../frontend/comfyui/ui/surface/ComfyLiteGraphCubeRenderer.js';
import { CubeCanvasChromeRenderer } from '../../../frontend/comfyui/ui/surface/CubeCanvasChromeRenderer.js';
import { computeCubeCanvasLayout } from '../../../frontend/comfyui/ui/surface/CubeCanvasLayout.js';
import { createDefaultCubeSurfaceState } from '../../../frontend/comfyui/ui/surface/CubeSurfaceState.js';
import { CubeIconResolver } from '../../../frontend/comfyui/ui/core/CubeIconResolver.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { CubePreviewSnapshot } from '../../../frontend/comfyui/ui/surface/CubePreviewModel.js';
import { comfyPrimeIconGlyph } from '../../../frontend/comfyui/ui/surface/ComfyPrimeIcons.js';
import { MemoryBudgetRegistry } from '../../../frontend/comfyui/ui/core/memory/MemoryBudgetRegistry.js';
import { CubeIconImageStore } from '../../../frontend/comfyui/ui/core/CubeIconImageStore.js';

const originalPalette = Object.getOwnPropertyDescriptor(globalThis, 'LiteGraph');
afterEach(() => {
  jest.restoreAllMocks();
  if (originalPalette) Object.defineProperty(globalThis, 'LiteGraph', originalPalette);
  else Reflect.deleteProperty(globalThis, 'LiteGraph');
});

test.each<CubePreviewSnapshot | null>([
  null,
  { outputs: [{ id: 'image', label: 'image', items: [] }] },
  {
    outputs: [
      { id: 'image', label: 'image', items: [{ key: 'pending', url: '/pending', label: 'image' }] },
    ],
  },
])('keeps port and preview labels readable through light and dark changes (%j)', (preview) => {
  // Header icons are a separate paint boundary; exercise the real body and preview draw.
  jest.spyOn(CubeCanvasChromeRenderer.prototype, 'draw').mockImplementation(() => undefined);
  const node = cubeNode();
  const input = { name: 'image', type: 'IMAGE' };
  const output = { name: 'image', type: 'IMAGE' };
  node.inputs = [input];
  node.subgraph.inputs = [input];
  node.outputs = [output];
  node.subgraph.outputs = [output];
  const layout = computeCubeCanvasLayout(node, createDefaultCubeSurfaceState(), 30);
  const draws: { text: string; color: string | CanvasGradient | CanvasPattern }[] = [];
  const strokes: (string | CanvasGradient | CanvasPattern)[] = [];
  const context = {
    canvas: document.createElement('canvas'),
    fillStyle: '',
    strokeStyle: '',
    globalAlpha: 1,
    font: '',
    textBaseline: '',
    lineWidth: 1,
    save() {},
    restore() {},
    beginPath() {},
    roundRect() {},
    fill() {},
    clip() {},
    moveTo() {},
    lineTo() {},
    stroke() {
      strokes.push(this.strokeStyle);
    },
    fillText(text: string) {
      draws.push({ text, color: this.fillStyle });
    },
    measureText(text: string) {
      return { width: text.length * 6 };
    },
  };
  const renderer = new ComfyLiteGraphCubeRenderer(
    { drawNode() {} },
    { get: () => null },
    new CubeIconResolver({ images: new CubeIconImageStore(new MemoryBudgetRegistry()) }),
  );
  for (const color of ['#242424', '#eeeeee', '#123456']) {
    Object.defineProperty(globalThis, 'LiteGraph', {
      configurable: true,
      value: {
        NODE_TITLE_COLOR: color,
        NODE_SELECTED_TITLE_COLOR: color,
        NODE_DEFAULT_COLOR: '#ffffff',
        NODE_DEFAULT_BGCOLOR: '#ffffff',
      },
    });
    draws.length = 0;
    strokes.length = 0;
    // Adapt the finite test paint surface to the browser-owned Canvas interface.
    renderer.draw(context as unknown as CanvasRenderingContext2D, [
      { node, layout, preview, chromeActions: null, editorButton: null },
    ]);
    expect(draws.length).toBeGreaterThan(0);
    expect(draws.map((draw) => draw.text)).toContain('IMAGE');
    expect(draws.every((draw) => draw.color === color)).toBe(true);
    expect(strokes.length).toBeGreaterThan(0);
    expect(strokes.every((stroke) => stroke === color)).toBe(true);
  }
});

test('paints legacy Cube header labels and glyphs with its accepted foreground', () => {
  const node = cubeNode();
  const layout = computeCubeCanvasLayout(node, createDefaultCubeSurfaceState(), 30);
  const draws: { text: string; color: string | CanvasGradient | CanvasPattern }[] = [];
  const context = {
    fillStyle: '',
    font: '',
    textBaseline: '',
    textAlign: '',
    save() {},
    restore() {},
    beginPath() {},
    roundRect() {},
    fill() {},
    fillRect() {},
    translate() {},
    scale() {},
    fillText(text: string) {
      draws.push({ text, color: this.fillStyle });
    },
    measureText(text: string) {
      return {
        width: text.length * 6,
        actualBoundingBoxAscent: 8,
        actualBoundingBoxDescent: 2,
        fontBoundingBoxAscent: 10,
        fontBoundingBoxDescent: 2,
      };
    },
  };
  const renderer = new CubeCanvasChromeRenderer(
    new CubeIconResolver({ images: new CubeIconImageStore(new MemoryBudgetRegistry()) }),
  );
  // Adapt measured text and paint methods at the test's browser Canvas boundary.
  renderer.draw(context as unknown as CanvasRenderingContext2D, {
    node,
    layout,
    chromeActions: null,
    editorButton: null,
    headerColor: '#ffffff',
    titleTextColor: '#242424',
    modelPillColors: { background: '#479ef5', foreground: '#111111' },
  });
  expect(draws.map((draw) => draw.text)).toContain('Workflow only');
  expect(
    draws.filter((draw) => draw.text !== comfyPrimeIconGlyph('ban') && draw.color !== '#242424'),
  ).toEqual([]);
});

test('shares decoded icon contrast across renderer draws without repeating canvas sampling', () => {
  const node = cubeNode();
  node.properties.sugarcubes_cube = {
    instance_id: 'palette',
    cube_id: 'test.cube',
    default_alias: 'Text to Image',
    icon: { kind: 'asset', url: '/icon.png', media_type: 'image/png' },
  };
  const image = document.createElement('img');
  Object.defineProperties(image, { naturalWidth: { value: 64 }, naturalHeight: { value: 32 } });
  const sample = {
    drawImage: jest.fn(),
    getImageData: (_x: number, _y: number, width: number, height: number): ImageData => ({
      data: new Uint8ClampedArray(width * height * 4).fill(255),
      width,
      height,
      colorSpace: 'srgb',
    }),
    putImageData: jest.fn(),
  };
  // Replace the browser raster boundary while exercising the real header renderer.
  jest
    .spyOn(HTMLCanvasElement.prototype, 'getContext')
    .mockReturnValue(sample as unknown as CanvasRenderingContext2D);
  const sources: CanvasImageSource[] = [];
  const context = {
    fillStyle: '',
    font: '',
    textBaseline: '',
    textAlign: '',
    save() {},
    restore() {},
    beginPath() {},
    roundRect() {},
    fill() {},
    fillRect() {},
    translate() {},
    scale() {},
    fillText() {},
    drawImage(source: CanvasImageSource) {
      sources.push(source);
    },
    measureText(text: string) {
      return {
        width: text.length * 6,
        actualBoundingBoxAscent: 8,
        actualBoundingBoxDescent: 2,
        fontBoundingBoxAscent: 10,
        fontBoundingBoxDescent: 2,
      };
    },
  };
  const icons = new CubeIconResolver({
    images: new CubeIconImageStore(new MemoryBudgetRegistry()),
  });
  jest.spyOn(icons, 'getImage').mockReturnValue({ status: 'ready', image });
  const renderer = new CubeCanvasChromeRenderer(icons);
  for (const foreground of ['#242424', '#eeeeee', '#242424']) {
    // Adapt only the paint boundary; all identity and contrast selection remains real.
    renderer.draw(context as unknown as CanvasRenderingContext2D, {
      node,
      layout: computeCubeCanvasLayout(node, createDefaultCubeSurfaceState(), 30),
      chromeActions: null,
      editorButton: null,
      headerColor: '#ffffff',
      titleTextColor: foreground,
      modelPillColors: { background: '#479ef5', foreground: '#111111' },
    });
  }
  expect(sources[0]).toBeInstanceOf(HTMLCanvasElement);
  expect(sources[1]).toBe(image);
  expect(sources[2]).toBe(sources[0]);
  expect(sample.drawImage).toHaveBeenCalledTimes(2);
});

/** Build an empty native Cube with a visible output rail and no graph mutations. */
function cubeNode(): CubeNode {
  return {
    id: 'palette-cube',
    type: 'cube-definition',
    title: 'Text to Image',
    pos: [0, 0],
    size: [920, 600],
    properties: { sugarcubes_kind: 'cube', sugarcubes_cube: { instance_id: 'palette' } },
    inputs: [],
    outputs: [],
    subgraph: {
      id: 'cube-definition',
      name: 'Text to Image',
      _nodes: [],
      inputs: [],
      outputs: [],
      inputNode: {},
      outputNode: {},
      add() {},
      remove() {},
      addInput() {
        throw new Error('Unused');
      },
      addOutput() {
        throw new Error('Unused');
      },
      configure() {},
    },
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
    setSize(size) {
      this.size[0] = size[0] ?? 0;
      this.size[1] = size[1] ?? 0;
    },
  };
}

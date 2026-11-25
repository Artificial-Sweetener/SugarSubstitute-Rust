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
/**
 * Frame presentation: the card surface's one owner, the toolbar row above side panels, the minimap
 * beside its toolbar, the grid kept out from under node cards, tooltips at the pointer and a card
 * change that leaves the other cards alone.
 */
import { afterEach, expect, jest, test } from '@jest/globals';
import { desktopFrameCss } from '../../../frontend/comfyui/ui/surface/frame/DesktopFrameStyles.js';
import { substituteToolbarCss } from '../../../frontend/comfyui/ui/surface/frame/SubstituteToolbarStyles.js';
import {
  GRAPH_MINIMAP_GAP,
  GRAPH_MINIMAP_WIDTH,
  GRAPH_TOOLBAR_MIRROR,
  graphToolbarCss,
} from '../../../frontend/comfyui/ui/surface/frame/GraphToolbarStyles.js';
import {
  DesktopGraphNodeOcclusion,
  NODE_CARD_RADIUS,
  NODE_OPACITY_SETTING,
  eraseUnderNodes,
  occlusionStrength,
} from '../../../frontend/comfyui/ui/surface/frame/DesktopGraphNodeOcclusion.js';
import {
  addGraphBackgroundPainter,
  paintGraphBackground,
} from '../../../frontend/comfyui/ui/overlays/GraphBackgroundPainters.js';
import {
  PointerTooltipPlacement,
  TOOLTIP_POINTER_OFFSET,
  pointerTooltipOrigin,
} from '../../../frontend/comfyui/ui/surface/appearance/PointerTooltipPlacement.js';
import { buildCubeCardVisibilitySignature } from '../../../frontend/comfyui/ui/surface/CubeCardVisibilitySignature.js';
import { setCubeFaceNodeEnabled } from '../../../frontend/comfyui/ui/surface/CubeFaceCardStateController.js';
import {
  parseCubeSurfaceState,
  serializeCubeSurfaceState,
} from '../../../frontend/comfyui/ui/surface/CubeSurfaceState.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import {
  fieldOwnUnderlineCss,
  fieldUnderlineCss,
} from '../../../frontend/comfyui/ui/surface/appearance/FieldSurfaceStyles.js';
import { nativeInteractionCss } from '../../../frontend/comfyui/ui/surface/appearance/NativeInteractionStyles.js';
import { modelPickerCss } from '../../../frontend/comfyui/ui/model_library/ModelPickerStyles.js';
import { ensureFinderStyles } from '../../../frontend/comfyui/ui/search/presentation/FinderStyles.js';

afterEach(() => {
  document.body.replaceChildren();
  document.getElementById('sc-masonry-finder-styles')?.remove();
});

const flat = (css: string): string => css.replace(/\s+/g, ' ');

test('only a card paints the card surface; a widget standing alone paints nothing behind it', () => {
  const css = flat(desktopFrameCss);
  expect(css).toContain(
    '[data-sc-style-card] { background:var(--sc-desktop-card-wash); border-radius:var(--sc-node-card-radius,4px); }',
  );
  // The node styling scope, which toolbar overrides and linked sources share, has no background.
  const scope = /\[data-sc-style-node\] \{([^}]*)\}/.exec(css)?.[1] ?? '';
  expect(scope).toContain('--sc-card-fill:transparent');
  expect(scope).not.toContain('background');
});

test('the toolbar row spans the window and every side panel opens beneath it', () => {
  const css = flat(substituteToolbarCss);
  expect(css).toContain(
    '.p-splitterpanel > [data-sc-substitute-toolbar-menu] { position:absolute; left:0; right:0; height:44px; margin-top:-44px; }',
  );
  expect(css).toContain(
    '.p-splitterpanel:has(> [data-sc-substitute-toolbar-menu]) { padding-top:44px; }',
  );
  expect(css).toContain('> :not(:has(> [data-sc-substitute-toolbar-menu])) { margin-top:44px; }');
});

test('the minimap and the toolbar beneath it share one width, right edge, corner and surface', () => {
  const css = flat(graphToolbarCss);
  const { inset, padding, control, radius } = GRAPH_TOOLBAR_MIRROR;
  const toolbarHeight = control + 2 * padding + 2;
  expect(css).toContain(`width:${String(GRAPH_MINIMAP_WIDTH)}px; justify-content:space-between;`);
  const minimap = css.slice(css.indexOf('.minimap-main-container {'));
  expect(minimap).toContain(
    `right:calc(var(--sc-output-workspace-reserved-width,0px) + ${String(inset)}px)`,
  );
  expect(minimap).toContain(`+ ${String(inset + toolbarHeight + GRAPH_MINIMAP_GAP)}px)`);
  expect(minimap).toContain(`border-radius:${String(radius)}px !important`);
  const surface =
    'background:var(--sugarcubes-card-surface,var(--comfy-menu-bg,#202020)); box-shadow:0 2px 8px #0003;';
  expect(css.split(surface)).toHaveLength(3);
});

interface Recorded {
  operation: string;
  fill: string;
  rects: number[][];
}

function recordingContext(): { context: CanvasRenderingContext2D; drawn: Recorded[] } {
  const drawn: Recorded[] = [];
  let rects: number[][] = [];
  const context = {
    globalCompositeOperation: 'source-over',
    fillStyle: '',
    save: jest.fn(),
    restore: jest.fn(),
    beginPath: () => {
      rects = [];
    },
    roundRect: (...args: number[]) => rects.push(args),
    fill() {
      drawn.push({ operation: this.globalCompositeOperation, fill: this.fillStyle, rects });
    },
  };
  return { context: context as unknown as CanvasRenderingContext2D, drawn };
}

test('the grid is erased under each node card in view, by the node opacity setting', () => {
  expect(occlusionStrength(undefined)).toBe(1);
  expect(occlusionStrength(0.4)).toBe(0.4);
  expect(occlusionStrength(7)).toBe(1);
  expect(occlusionStrength(-1)).toBe(0);
  const nodes = [
    { boundingRect: new Float64Array([10, 20, 300, 200]) },
    { boundingRect: new Float64Array([5000, 5000, 300, 200]) },
    { boundingRect: new Float64Array([0, 0, 0, 0]) },
    {},
  ];
  const { context, drawn } = recordingContext();
  eraseUnderNodes(context, nodes, [0, 0, 1000, 800], 0.5);
  expect(drawn).toEqual([
    {
      operation: 'destination-out',
      fill: 'rgba(0,0,0,0.5)',
      rects: [[10, 20, 300, 200, NODE_CARD_RADIUS]],
    },
  ]);
  const none = recordingContext();
  eraseUnderNodes(none.context, nodes, [0, 0, 1000, 800], 0);
  expect(none.drawn).toEqual([]);
});

test('the occlusion paints through the overlay hook’s registry and follows the setting live', () => {
  let opacity = 1;
  const listeners = new Map<string, EventListener>();
  const setDirty = jest.fn();
  const canvas = {
    setDirty,
    visible_area: new Float32Array([0, 0, 1000, 800]),
    graph: { _nodes: [{ boundingRect: new Float64Array([10, 20, 300, 200]) }] },
  };
  const app = {
    extensionManager: {
      setting: { get: (id: string) => (id === NODE_OPACITY_SETTING ? opacity : null) },
    },
    ui: {
      settings: {
        addEventListener: (name: string, listener: EventListener) => listeners.set(name, listener),
        removeEventListener: (name: string) => listeners.delete(name),
      },
    },
  };
  const occlusion = new DesktopGraphNodeOcclusion();
  occlusion.attach(app, canvas);
  const first = recordingContext();
  paintGraphBackground(canvas, first.context, () => undefined);
  expect(first.drawn[0]?.fill).toBe('rgba(0,0,0,1)');
  opacity = 0.25;
  setDirty.mockClear();
  listeners.get(`${NODE_OPACITY_SETTING}.change`)?.(new Event('change'));
  expect(setDirty).toHaveBeenCalledWith(true, true);
  const second = recordingContext();
  paintGraphBackground(canvas, second.context, () => undefined);
  expect(second.drawn[0]?.fill).toBe('rgba(0,0,0,0.25)');
  occlusion.detach();
  expect(listeners.size).toBe(0);
  const after = recordingContext();
  paintGraphBackground(canvas, after.context, () => undefined);
  expect(after.drawn).toEqual([]);
});

test('one failing background painter is reported and the others still paint', () => {
  const canvas = {};
  const painted = jest.fn();
  const report = jest.fn();
  const removeFailing = addGraphBackgroundPainter(canvas, () => {
    throw new Error('broken');
  });
  const remove = addGraphBackgroundPainter(canvas, painted);
  paintGraphBackground(canvas, recordingContext().context, report);
  expect(report).toHaveBeenCalledTimes(1);
  expect(painted).toHaveBeenCalledTimes(1);
  removeFailing();
  remove();
  paintGraphBackground(canvas, recordingContext().context, report);
  expect(painted).toHaveBeenCalledTimes(1);
});

test('a tooltip opens beside the pointer and stays inside the window', () => {
  const { x, y, margin } = TOOLTIP_POINTER_OFFSET;
  const viewport = { width: 1280, height: 800 };
  const box = { width: 200, height: 160 };
  expect(pointerTooltipOrigin(600, 300, box, viewport)).toEqual({ left: 600 + x, top: 300 + y });
  // Against the right and bottom edges it opens on the pointer's other side.
  expect(pointerTooltipOrigin(1200, 300, box, viewport).left).toBe(1200 - x - box.width);
  expect(pointerTooltipOrigin(600, 750, box, viewport).top).toBe(750 - y / 2 - box.height);
  const corner = pointerTooltipOrigin(2, 2, { width: 1400, height: 900 }, viewport);
  expect(corner).toEqual({ left: margin, top: margin });
});

test('every tooltip added to the page is placed by the one pointer rule; the keyboard keeps its anchor', async () => {
  const placement = new PointerTooltipPlacement(document);
  const tooltip = (): HTMLElement => {
    const element = document.createElement('div');
    element.className = 'p-tooltip p-component p-tooltip-left';
    element.style.left = '57px';
    element.style.top = '580px';
    document.body.append(element);
    return element;
  };
  document.dispatchEvent(new MouseEvent('pointermove', { clientX: 400, clientY: 300 }));
  const first = tooltip();
  await Promise.resolve();
  expect(first.hasAttribute('data-sc-pointer-tooltip')).toBe(true);
  expect([first.style.left, first.style.top]).toEqual([
    `${String(400 + TOOLTIP_POINTER_OFFSET.x)}px`,
    `${String(300 + TOOLTIP_POINTER_OFFSET.y)}px`,
  ]);
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab' }));
  const focused = tooltip();
  await Promise.resolve();
  expect(focused.hasAttribute('data-sc-pointer-tooltip')).toBe(false);
  expect(focused.style.left).toBe('57px');
  placement.dispose();
  document.dispatchEvent(new MouseEvent('pointermove', { clientX: 10, clientY: 10 }));
  const late = tooltip();
  await Promise.resolve();
  expect(late.style.left).toBe('57px');
});

test('recording a card’s first enable choice does not change which cards a Cube shows', () => {
  const nodes = [
    { id: 1, mode: 0 },
    { id: 2, mode: 4 },
  ] as unknown as ComfyNode[];
  const state = parseCubeSurfaceState(undefined);
  const before = buildCubeCardVisibilitySignature(nodes, serializeCubeSurfaceState(state));
  const first = nodes[0];
  if (!first) throw new Error('Missing node');
  setCubeFaceNodeEnabled(state, first, false);
  expect(Object.keys(state.cards)).toEqual(['1']);
  const disabled = buildCubeCardVisibilitySignature(nodes, serializeCubeSurfaceState(state));
  expect(disabled).toBe(before);
  setCubeFaceNodeEnabled(state, first, true);
  expect(buildCubeCardVisibilitySignature(nodes, serializeCubeSurfaceState(state))).toBe(before);
});

test('the focus underline is one definition: a bar across the field, cut by the field’s corners', () => {
  const layer = flat(fieldUnderlineCss('.host', '.host:focus-within'));
  // A layer the size of the field with the field's radius; the bar is its bottom strip.
  expect(layer).toContain('.host::after {');
  expect(layer).toContain('position:absolute; inset:0; border-radius:inherit;');
  expect(layer).toContain('bottom / 100% 2px no-repeat');
  expect(layer).toContain('.host:focus-within::after { transform:scaleX(1); }');
  const own = flat(fieldOwnUnderlineCss('input.box', 'input.box:focus'));
  // In a text box the bar is the box's own background, which its corners cut the same way.
  expect(own).toContain(
    'background-origin:border-box; background-position:bottom center; background-size:0 2px;',
  );
  expect(own).toContain(
    'input.box:focus { background-size:var(--sc-field-underline-size,100% 2px); border-bottom-color:transparent; }',
  );
  // Every field the pack draws takes it from there: none paints a brand border or strip of its own.
  ensureFinderStyles(document);
  const finderCss = document.getElementById('sc-masonry-finder-styles')?.textContent ?? '';
  expect(finderCss).toContain('.sc-masonry-finder__group::after');
  for (const css of [nativeInteractionCss(), modelPickerCss, finderCss]) {
    expect(css).not.toMatch(/border-bottom(-color)?:[^;]*brand/);
    expect(css).not.toMatch(/height:2px;[^}]*brand/);
  }
});

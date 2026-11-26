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
/** Prove graph-frame qualification and exact reversible ownership without replacing Comfy's renderers. */

import {
  ComfyCanvasFrameAllocation,
  GRAPH_LAYER_CLIP_PROPERTY,
} from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyCanvasFrameAllocation.js';
import {
  CANVAS_PANE_LEFT_PROPERTY,
  ensureCanvasWorkspaceStyles,
} from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasWorkspaceStyles.js';
import { jest } from '@jest/globals';

/** Build only the installed public frame/chrome shape, leaving all renderer children in place. */
function frame() {
  const parent = document.createElement('div');
  parent.className = 'comfyui-body';
  const root = document.createElement('div');
  parent.append(root);
  root.id = 'graph-canvas-container';
  // ComfyUI 1.47.11: the chrome layer holds the graph splitter, which holds the graph panel.
  const chrome = document.createElement('div');
  const splitter = document.createElement('div');
  splitter.className = 'splitter-overlay-bottom';
  const panel = document.createElement('div');
  panel.className = 'graph-canvas-panel';
  splitter.append(panel);
  chrome.append(splitter);
  const canvas = document.createElement('canvas');
  canvas.id = 'graph-canvas';
  const nodes = document.createElement('div');
  nodes.className = 'ph-no-capture';
  root.append(chrome, canvas, nodes);
  return { parent, root, chrome, splitter, panel, canvas, nodes };
}

/**
 * Lay the graph panel out inside the frame as the browser would: Comfy's own spacing from each
 * frame edge, and on the right and bottom also the padding the allocation puts on the splitter.
 */
function lay(
  f: ReturnType<typeof frame>,
  bounds: () => DOMRect,
  spacing: { left: number; top: number; right: number; bottom: number },
): void {
  const padding = (property: string): number =>
    Number.parseFloat(f.splitter.style.getPropertyValue(property)) || 0;
  jest.spyOn(f.parent, 'getBoundingClientRect').mockImplementation(bounds);
  jest.spyOn(f.panel, 'getBoundingClientRect').mockImplementation(() => {
    const outer = bounds();
    const left = outer.left + spacing.left;
    const top = outer.top + spacing.top;
    return rectangle(
      left,
      top,
      outer.right - spacing.right - padding('padding-right') - left,
      outer.bottom - spacing.bottom - padding('padding-bottom') - top,
    );
  });
}

test('allocates a common graph origin and ends the graph splitter where the graph ends', () => {
  const f = frame();
  const adapter = new ComfyCanvasFrameAllocation(f.root, f.panel);
  adapter.apply(480);
  expect(f.root.style.marginLeft).toBe('0px');
  expect(f.root.style.width).toBe('calc(100% - 480px)');
  expect(f.splitter.style.paddingRight).toBe('480px');
  expect(adapter.reservation()).toEqual({ right: 480, bottom: 0 });
  expect(f.chrome.style.position).toBe('fixed');
  expect(f.chrome.style.left).toBe('0px');
  expect(f.root.style.overflow).toBe('clip');
  expect(f.root.style.isolation).toBe('isolate');
  expect(f.root.style.clipPath).toBe('');
  expect(f.parent.style.overflow).toBe('clip');
  expect(f.root.children).toHaveLength(3);
  expect(f.canvas.parentElement).toBe(f.root);
  expect(f.nodes.parentElement).toBe(f.root);
  expect(f.canvas.style.width).toBe('');
  adapter.apply(0);
  expect(f.root.style.cssText).toBe('');
  expect(f.chrome.style.cssText).toBe('');
  expect(f.parent.style.cssText).toBe('');
  expect(f.splitter.style.cssText).toBe('');
  expect(adapter.reservation()).toEqual({ right: 0, bottom: 0 });
  adapter.dispose();
});

test('anchors ComfyUI graph chrome on the graph’s own edges, beside the output pane, with no per-control offsets', () => {
  const f = frame();
  lay(f, () => rectangle(0, 0, 1280, 800), { left: 0, top: 78, right: 0, bottom: 0 });
  const adapter = new ComfyCanvasFrameAllocation(f.root, f.panel);
  adapter.apply(320, 200);
  // The panel Comfy anchors its toolbar, minimap and zoom menu in ends where the graph ends.
  const panel = f.panel.getBoundingClientRect();
  expect([panel.left, panel.right, panel.bottom]).toEqual([0, 960, 600]);
  expect(f.root.style.width).toBe('calc(100% - 320px)');
  expect(f.root.style.height).toBe('calc(100% - 278px)');
  expect(adapter.reservation()).toEqual({ right: 320, bottom: 200 });
  // Repeating the allocation in its own layout is stable.
  const mutations = new MutationObserver(() => undefined);
  mutations.observe(f.parent, { attributes: true, subtree: true });
  adapter.apply(320, 200);
  expect(mutations.takeRecords()).toHaveLength(0);
  mutations.disconnect();
  ensureCanvasWorkspaceStyles(document);
  const css = [...document.head.querySelectorAll('style')]
    .map((style) => style.textContent)
    .join('');
  expect(css).not.toContain('--sc-output-workspace-reserved');
  adapter.dispose();
  expect(f.splitter.style.cssText).toBe('');
});

test('restores pre-existing inline styles and priorities on disposal and rejects unsupported roots', () => {
  const f = frame();
  f.root.style.setProperty('width', '90%', 'important');
  f.root.style.setProperty('overflow', 'clip');
  f.root.style.setProperty(GRAPH_LAYER_CLIP_PROPERTY, 'none');
  f.parent.style.setProperty('overflow', 'hidden', 'important');
  f.chrome.style.setProperty('left', '2px');
  f.chrome.style.setProperty('position', 'absolute', 'important');
  f.splitter.style.setProperty('padding-right', '10px', 'important');
  const original = f.root.style.cssText;
  const chromeOriginal = f.chrome.style.cssText;
  const adapter = new ComfyCanvasFrameAllocation(f.root, f.panel);
  expect(() => adapter.apply(-1)).toThrow(RangeError);
  adapter.apply(400);
  adapter.dispose();
  expect(f.root.style.cssText).toBe(original);
  expect(f.chrome.style.cssText).toBe(chromeOriginal);
  expect(f.parent.style.getPropertyValue('overflow')).toBe('hidden');
  expect(f.parent.style.getPropertyPriority('overflow')).toBe('important');
  expect(f.splitter.style.getPropertyValue('padding-right')).toBe('10px');
  expect(f.splitter.style.getPropertyPriority('padding-right')).toBe('important');
  adapter.apply(500);
  expect(f.root.style.cssText).toBe(original);
  expect(() => new ComfyCanvasFrameAllocation(document.createElement('div'), f.panel)).toThrow(
    'unsupported',
  );
  f.canvas.remove();
  expect(() => new ComfyCanvasFrameAllocation(f.root, f.panel)).toThrow('unsupported');
});

test('allocates the real graph below chrome even with the output canvas closed and restores on suspension', () => {
  const f = frame();
  const spacing = { left: 4, top: 86, right: 4, bottom: 4 };
  lay(f, () => rectangle(0, 0, 1200, 900), spacing);
  const adapter = new ComfyCanvasFrameAllocation(f.root, f.panel);
  adapter.apply(480);
  expect(f.root.style.marginTop).toBe('86px');
  expect(f.root.style.height).toBe('calc(100% - 90px)');
  expect(f.chrome.style.top).toBe('0px');
  expect(f.chrome.style.height).toBe('900px');
  expect(f.root.style.marginLeft).toBe('4px');
  expect(f.root.style.width).toBe('calc(100% - 488px)');
  expect(f.chrome.style.left).toBe('0px');
  expect(f.chrome.style.width).toBe('1200px');
  expect(f.root.style.getPropertyValue(GRAPH_LAYER_CLIP_PROPERTY)).toBe(
    'inset(0px 0px calc(100% - 810px) 0px)',
  );
  adapter.apply(0);
  expect(f.root.style.width).toBe('calc(100% - 8px)');
  expect(f.root.style.marginTop).toBe('86px');
  spacing.top = 120;
  adapter.apply(0);
  expect(f.root.style.marginTop).toBe('120px');
  expect(f.root.style.height).toBe('calc(100% - 124px)');
  const mutations = new MutationObserver(() => undefined);
  mutations.observe(f.root, { attributes: true, subtree: true });
  adapter.apply(0);
  expect(mutations.takeRecords()).toHaveLength(0);
  mutations.disconnect();
  adapter.suspend();
  expect(f.root.style.cssText).toBe('');
  expect(f.chrome.style.cssText).toBe('');
  adapter.apply(0);
  expect(f.root.style.marginTop).toBe('120px');
  adapter.dispose();
  expect(f.root.style.cssText).toBe('');
  expect(f.chrome.style.cssText).toBe('');
});

test('preserves one layout quantum as a real fractional inset without reconstructing chrome', () => {
  const f = frame();
  jest.spyOn(f.parent, 'getBoundingClientRect').mockReturnValue(rectangle(0, 0, 1484, 937));
  jest
    .spyOn(f.panel, 'getBoundingClientRect')
    .mockReturnValue(rectangle(0, 78, 1484 - 1 / 64, 859));
  const adapter = new ComfyCanvasFrameAllocation(f.root, f.panel);
  adapter.apply(593.59375);
  expect(f.root.style.width).toBe('calc(100% - 593.609375px)');
  expect(f.chrome.style.width).toBe('1484px');
  adapter.dispose();
});

test('preserves a meaningful fractional edge inset without rewriting a stable allocation', () => {
  const f = frame();
  lay(f, () => rectangle(0, 0, 1484, 937), { left: 0, top: 78, right: 0.25, bottom: 0 });
  const adapter = new ComfyCanvasFrameAllocation(f.root, f.panel);
  adapter.apply(593.59375);

  expect(f.root.style.width).toBe('calc(100% - 593.84375px)');
  expect(f.chrome.style.width).toBe('1484px');
  const mutations = new MutationObserver(() => undefined);
  mutations.observe(f.root, { attributes: true, subtree: true });
  adapter.apply(593.59375);
  expect(mutations.takeRecords()).toHaveLength(0);
  mutations.disconnect();
  adapter.dispose();
});

test('updates authoritative chrome dimensions when only the outer frame resizes', () => {
  const f = frame();
  let width = 1484;
  lay(f, () => rectangle(0, 0, width, 937), { left: 63.25, top: 78.5, right: 4.125, bottom: 2.25 });
  const adapter = new ComfyCanvasFrameAllocation(f.root, f.panel);
  adapter.apply(593.59375);
  expect(f.chrome.style.cssText).toContain('width: 1484px');
  expect(f.chrome.style.cssText).toContain('height: 937px');
  expect(f.root.style.marginLeft).toBe('63.25px');
  expect(f.root.style.marginTop).toBe('78.5px');
  expect(f.root.style.getPropertyValue(GRAPH_LAYER_CLIP_PROPERTY)).toBe(
    'inset(0px 0px calc(100% - 856.25px) 0px)',
  );

  width = 1484.375;
  adapter.apply(593.59375);
  expect(f.chrome.style.width).toBe('1484.375px');
  adapter.dispose();
});

test('keeps fixed chrome on the frame client rectangle when the frame moves', () => {
  const f = frame();
  let left = 12;
  lay(f, () => rectangle(left, 30, 1200, 900), { left: 4, top: 86, right: 4, bottom: 4 });
  const adapter = new ComfyCanvasFrameAllocation(f.root, f.panel);
  adapter.apply(480);
  expect(f.chrome.getAttribute('style')).toBe(
    'position: fixed; left: 12px; top: 30px; width: 1200px; height: 900px;',
  );
  expect(f.root.style.marginLeft).toBe('4px');
  left = 40;
  adapter.apply(480);
  expect(f.chrome.style.left).toBe('40px');
  expect(f.root.style.marginLeft).toBe('4px');
  adapter.dispose();
  expect(f.chrome.style.cssText).toBe('');
  expect(f.root.style.cssText).toBe('');
});

test('clips only the DOM-widget layer through the graph root allocation', () => {
  const f = frame();
  const widgets = document.createElement('div');
  widgets.className = 'isolate';
  const widget = document.createElement('div');
  widget.className = 'dom-widget';
  widgets.append(widget);
  f.root.append(widgets);
  ensureCanvasWorkspaceStyles(document);
  // jsdom does not cascade custom properties into clip-path; inspect the matching CSSOM rule.
  const rules = [...document.styleSheets]
    .flatMap((sheet) => [...sheet.cssRules])
    .filter((entry): entry is CSSStyleRule => entry instanceof CSSStyleRule)
    .filter((entry) => entry.style.getPropertyValue('clip-path'));
  expect(
    rules.map((entry) => [entry.selectorText, entry.style.getPropertyValue('clip-path')]),
  ).toEqual([
    ['#graph-canvas-container > :has(> .dom-widget)', `var(${GRAPH_LAYER_CLIP_PROPERTY},none)`],
    // ComfyUI's node toolbar is centred on --tb-x and is cut off where the canvas pane begins.
    [
      '#graph-canvas-container [data-testid="selection-toolbox"]',
      `inset(0 max(0px,calc(var(--tb-x,0px) + 50% - var(${CANVAS_PANE_LEFT_PROPERTY},100000px))) 0 0)`,
    ],
  ]);
  jest.spyOn(f.parent, 'getBoundingClientRect').mockReturnValue(rectangle(0, 0, 1200, 900));
  jest.spyOn(f.panel, 'getBoundingClientRect').mockReturnValue(rectangle(0, 86, 1200, 814));
  const adapter = new ComfyCanvasFrameAllocation(f.root, f.panel);
  adapter.apply(480);
  expect(f.root.style.getPropertyValue(GRAPH_LAYER_CLIP_PROPERTY)).toBe(
    'inset(0px 0px calc(100% - 814px) 0px)',
  );
  expect(widgets.getAttribute('style')).toBeNull();
  expect(widget.getAttribute('style')).toBeNull();
  adapter.suspend();
  expect(f.root.style.getPropertyValue(GRAPH_LAYER_CLIP_PROPERTY)).toBe('');
  adapter.dispose();
});

/** Supply stable browser geometry without relying on JSDOM layout. */
function rectangle(x: number, y: number, width: number, height: number): DOMRect {
  return {
    x,
    y,
    width,
    height,
    left: x,
    top: y,
    right: x + width,
    bottom: y + height,
    toJSON: () => ({ x, y, width, height }),
  };
}

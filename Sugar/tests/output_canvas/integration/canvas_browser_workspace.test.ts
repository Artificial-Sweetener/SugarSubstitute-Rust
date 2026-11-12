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
/** Verify allocated sibling identity, explicit drawers, finite resize work, and exact native frame restoration. */

import { jest } from '@jest/globals';
import { BrowserCanvasWorkspace } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasWorkspace.js';
import { CanvasWorkspaceLayout } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasWorkspaceLayout.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { installCanvasWorkspaceAnimation } from '../support/CanvasWorkspaceMotionFixture.js';

/** Supply only platform geometry while exercising real DOM allocation and layout policy. */
function fixture(width = 1400) {
  const root = document.createElement('div');
  root.id = 'graph-canvas-container';
  root.style.setProperty('overflow', 'clip');
  const chrome = document.createElement('div');
  const panel = document.createElement('div');
  panel.className = 'graph-canvas-panel';
  chrome.append(panel);
  const graph = document.createElement('canvas');
  graph.id = 'graph-canvas';
  root.append(chrome, graph);
  const frame = document.createElement('div');
  frame.className = 'comfyui-body';
  frame.append(root);
  document.body.append(frame);
  let measuredWidth = width;
  jest.spyOn(frame, 'getBoundingClientRect').mockImplementation(() => ({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    width: measuredWidth,
    height: 800,
    right: measuredWidth,
    bottom: 800,
    toJSON: () => ({}),
  }));
  jest.spyOn(panel, 'getBoundingClientRect').mockImplementation(() => ({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    width: measuredWidth,
    height: 800,
    right: measuredWidth,
    bottom: 800,
    toJSON: () => ({}),
  }));
  const frames = new CanvasFrameFixture();
  // A 1400 px workspace leaves this default editor a 560 px canvas.
  const model = new CanvasWorkspaceLayout(840);
  const actions = {
    editorWidth: jest.fn<(editorWidth: number) => void>(),
    close: jest.fn(),
    changed: jest.fn(),
    report: jest.fn(),
  };
  const workspace = new BrowserCanvasWorkspace(root, panel, model, frames, actions);
  return {
    root,
    chrome,
    panel,
    graph,
    frames,
    model,
    actions,
    workspace,
    resize: (next: number) => {
      measuredWidth = next;
      window.dispatchEvent(new Event('resize'));
    },
    dispose: () => {
      workspace.dispose();
      root.remove();
    },
  };
}

test('keeps masonry content identity and restores the native frame through close, suspension, and disposal', () => {
  const f = fixture();
  expect(f.workspace.visible).toBe(true);
  expect(f.panel.querySelector('[aria-label="Close output canvas"]')).toBeNull();
  expect(f.workspace.content.style.left).toBe('0px');
  expect(f.workspace.content.style.width).toBe('calc(100% - 560px)');
  expect(f.root.style.marginLeft).toBe('0px');
  const borrowed = document.createElement('input');
  f.workspace.content.append(borrowed);
  f.workspace.suspend('app');
  expect(f.workspace.visible).toBe(false);
  expect(f.workspace.content.contains(borrowed)).toBe(true);
  expect(f.root.style.marginLeft).toBe('');
  f.workspace.suspend(null);
  f.workspace.preferences({ visible: false, editorWidth: 800 });
  expect(f.workspace.content.style.left).toBe('0px');
  f.workspace.preferences({ visible: true, editorWidth: 800 });
  expect(f.workspace.content.contains(borrowed)).toBe(true);
  expect(f.workspace.content.style.left).toBe('0px');
  expect(f.workspace.content.style.width).toBe('calc(100% - 600px)');
  expect(f.graph.parentElement).toBe(f.root);
  const changed = f.actions.changed.mock.calls.length;
  f.dispose();
  f.workspace.dispose();
  expect(f.root.style.cssText).toBe('overflow: clip;');
  expect(f.chrome.style.cssText).toBe('');
  expect(f.frames.pending).toBe(0);
  window.dispatchEvent(new Event('resize'));
  expect(f.actions.changed).toHaveBeenCalledTimes(changed);
});

test('does not generate presentation mutations when accepted allocation is unchanged', () => {
  const f = fixture();
  const mutations = new MutationObserver(() => undefined);
  mutations.observe(f.workspace.content, { attributes: true });
  for (let index = 0; index < 100; index++)
    f.workspace.preferences({ visible: true, editorWidth: 0 });
  expect(mutations.takeRecords()).toHaveLength(0);
  mutations.disconnect();
  f.dispose();
});

test('keeps the fixed sibling on the panel client rectangle across allocation and app suspension', () => {
  const f = fixture();
  const parent = f.root.parentElement!;
  /** Model native layout rectangles independently of the allocation policy. */
  const rectangle = (left: number, top: number, width: number, height: number): DOMRect => ({
    x: left,
    y: top,
    left,
    top,
    width,
    height,
    right: left + width,
    bottom: top + height,
    toJSON: () => ({ left, top, width, height }),
  });
  jest.spyOn(parent, 'getBoundingClientRect').mockReturnValue(rectangle(0, 0, 1400, 900));
  jest.spyOn(f.panel, 'getBoundingClientRect').mockReturnValue(rectangle(4, 86, 1392, 810));
  const sibling = f.root.querySelector<HTMLElement>('.sc-output-workspace-frame')!;
  window.dispatchEvent(new Event('resize'));
  f.frames.advance(16);
  expect(f.root.style.marginTop).toBe('86px');
  expect(f.chrome.style.position).toBe('fixed');
  expect([sibling.style.left, sibling.style.top, sibling.style.width]).toEqual([
    '4px',
    '86px',
    '1392px',
  ]);
  f.workspace.suspend('app');
  expect(f.root.style.marginTop).toBe('');
  expect(f.chrome.style.position).toBe('');
  expect([sibling.style.left, sibling.style.top]).toEqual(['4px', '86px']);
  f.workspace.suspend(null);
  expect(f.root.style.marginTop).toBe('86px');
  expect([sibling.style.left, sibling.style.top]).toEqual(['4px', '86px']);
  f.dispose();
});

test('requires explicit narrow activation and exposes authoritative keyboard width bounds', () => {
  const f = fixture(800);
  expect(f.workspace.visible).toBe(false);
  f.workspace.drawer(true);
  expect(f.workspace.visible).toBe(true);
  expect(f.root.style.marginLeft).toBe('');
  expect(f.workspace.content.style.left).toBe('0px');
  const divider = f.root.querySelector('[role="separator"]')!;
  expect(divider.getAttribute('aria-valuenow')).toBe('640');
  expect(divider.getAttribute('aria-valuemax')).toBe('776');
  expect(f.root.querySelector<HTMLElement>('.sc-output-workspace-backdrop')!.style.right).toBe(
    '640px',
  );
  expect(document.activeElement?.getAttribute('aria-label')).toBe('Workflow output canvas');
  f.root.querySelector<HTMLButtonElement>('.sc-output-workspace-backdrop')!.click();
  expect(f.actions.close).toHaveBeenCalledTimes(1);
  f.resize(1400);
  f.frames.advance(16);
  expect(f.root.style.marginLeft).toBe('0px');
  f.resize(800);
  f.frames.advance(16);
  expect(f.workspace.visible).toBe(false);
  f.dispose();
});

test('keeps native minimap chrome aligned with the remaining graph allocation', () => {
  const f = fixture();
  const minimap = document.createElement('div');
  minimap.className = 'minimap-main-container';
  f.panel.append(minimap);
  // jsdom does not resolve custom properties in geometry; inspect the matching CSSOM rule.
  const rule = [...document.styleSheets]
    .flatMap((sheet) => [...sheet.cssRules])
    .filter((entry): entry is CSSStyleRule => entry instanceof CSSStyleRule)
    .find((entry) => minimap.matches(entry.selectorText) && entry.style.getPropertyValue('right'));
  expect(rule?.style.getPropertyValue('right')).toBe(
    'var(--sc-output-workspace-reserved-width,0px)',
  );
  expect(f.panel.style.getPropertyValue('--sc-output-workspace-reserved-width')).toBe('560px');
  f.workspace.preferences({ visible: false, editorWidth: 0 });
  expect(f.panel.style.getPropertyValue('--sc-output-workspace-reserved-width')).toBe('');
  f.dispose();
});

test('coalesces workspace measurements, gives the canvas the new width, and cancels pending observation', () => {
  const f = fixture();
  const changed = f.actions.changed.mock.calls.length;
  for (let width = 1400; width < 1500; width++) f.resize(width);
  expect(f.frames.pending).toBe(1);
  expect(f.actions.changed).toHaveBeenCalledTimes(changed);
  f.frames.advance(16);
  expect(f.workspace.content.style.left).toBe('0px');
  expect(f.workspace.content.style.width).toBe('calc(100% - 659px)');
  expect(f.chrome.style.width).toBe('1499px');
  expect(f.actions.changed).toHaveBeenCalledTimes(changed + 1);
  f.resize(1600);
  f.dispose();
  expect(f.frames.pending).toBe(0);
});

test('keeps exit pixels and their width until settled while accepted visibility closes immediately', async () => {
  const f = fixture();
  const pane = f.root.querySelector<HTMLElement>('.sc-output-workspace-pane')!;
  const animation = installCanvasWorkspaceAnimation(pane);
  f.workspace.preferences({ visible: false, editorWidth: 0 }, true);
  expect(f.workspace.visible).toBe(false);
  expect(f.workspace.presented).toBe(true);
  expect(pane.style.width).toBe('560px');
  expect(f.workspace.content.style.width).toBe('calc(100% - 0px)');
  animation.effects[0]!.finish();
  await Promise.resolve();
  await Promise.resolve();
  expect(f.workspace.presented).toBe(false);
  expect(pane.hidden).toBe(true);
  f.workspace.preferences({ visible: true, editorWidth: 0 }, true);
  expect(f.workspace.presented).toBe(true);
  expect(animation.effects[1]?.frames[0]?.transform).toBe('translateX(100%)');
  f.workspace.suspend('configure');
  expect(f.workspace.presented).toBe(false);
  await Promise.resolve();
  expect(f.actions.report).not.toHaveBeenCalled();
  f.dispose();
});

test('renders a two-pixel divider with a wider invisible resize target', () => {
  const f = fixture();
  const divider = f.root.querySelector<HTMLElement>('.sc-output-workspace-divider')!;
  expect(window.getComputedStyle(divider).width).toBe('2px');
  expect(window.getComputedStyle(f.workspace.canvas).marginLeft).toBe('2px');
  const rules = [...document.styleSheets].flatMap((sheet) => [...sheet.cssRules]);
  expect(
    rules.some(
      (rule) =>
        rule.cssText.includes('.sc-output-workspace-divider::before') &&
        rule.cssText.includes('inset: 0 -3px'),
    ),
  ).toBe(true);
  f.dispose();
});

test('rolls back owned DOM, listeners, and graph geometry when initial allocation publication fails', () => {
  const f = fixture();
  f.workspace.dispose();
  expect(
    () =>
      new BrowserCanvasWorkspace(f.root, f.panel, f.model, f.frames, {
        editorWidth: () => undefined,
        close: () => undefined,
        report: () => undefined,
        changed: () => {
          throw new Error('Publication failed.');
        },
      }),
  ).toThrow('Publication failed.');
  expect(f.panel.children).toHaveLength(0);
  expect(f.root.style.cssText).toBe('overflow: clip;');
  expect(f.chrome.style.cssText).toBe('');
  window.dispatchEvent(new Event('resize'));
  expect(f.frames.pending).toBe(0);
  f.dispose();
});

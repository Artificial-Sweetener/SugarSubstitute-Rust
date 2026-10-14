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
/** Prove native execution overlays never reserve space in the shared output/masonry frame. */
import { jest } from '@jest/globals';
import { BrowserCanvasWorkspace } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasWorkspace.js';
import { CanvasWorkspaceLayout } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasWorkspaceLayout.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';

/** Model browser rectangles without depending on jsdom's optional DOMRect constructor. */
function bounds(left: number, top: number, width: number, height: number): DOMRect {
  return {
    x: left,
    y: top,
    left,
    top,
    width,
    height,
    right: left + width,
    bottom: top + height,
    toJSON: () => ({}),
  };
}

/** Supply native layout measurements while retaining the actual workspace and DOM owners. */
function fixture() {
  const body = document.createElement('div');
  body.className = 'comfyui-body';
  const root = document.createElement('div');
  root.id = 'graph-canvas-container';
  const chrome = document.createElement('div');
  const center = document.createElement('div');
  center.className = 'p-splitterpanel';
  const menu = document.createElement('div');
  menu.style.rowGap = '4px';
  const row = document.createElement('div');
  const bar = document.createElement('div');
  bar.className = 'actionbar-container';
  row.append(bar);
  menu.append(row);
  const splitter = document.createElement('div');
  const panel = document.createElement('div');
  panel.className = 'graph-canvas-panel';
  splitter.append(panel);
  center.append(menu, splitter);
  chrome.append(center);
  const graph = document.createElement('canvas');
  graph.id = 'graph-canvas';
  root.append(chrome, graph);
  body.append(root);
  document.body.append(body);
  let top = 92;
  let left = 60;
  let bottom = 800;
  let toolbarBottom = 88;
  jest.spyOn(root, 'getBoundingClientRect').mockImplementation(() => bounds(0, 0, 1460, 800));
  jest
    .spyOn(menu, 'getBoundingClientRect')
    .mockImplementation(() => bounds(left, 36, 1460 - left, top - 36));
  jest.spyOn(panel, 'getBoundingClientRect').mockImplementation(() => {
    // jsdom has no flex layout; model its allocation from the qualified menu's published height.
    const activeMenu = center.firstElementChild;
    const allocated =
      activeMenu instanceof HTMLElement &&
      activeMenu.classList.contains('sc-output-workspace-topmenu')
        ? 36 + Number.parseFloat(activeMenu.style.getPropertyValue('--sc-output-toolbar-height'))
        : top;
    return bounds(left, allocated, 1460 - left, bottom - allocated);
  });
  jest
    .spyOn(row, 'getBoundingClientRect')
    .mockImplementation(() => bounds(left, 40, 1460 - left, toolbarBottom - 40));
  const frames = new CanvasFrameFixture();
  const changed = jest.fn();
  const workspace = new BrowserCanvasWorkspace(root, panel, new CanvasWorkspaceLayout(), frames, {
    changed,
    width: () => undefined,
    close: () => undefined,
    report: () => undefined,
  });
  return {
    root,
    panel,
    menu,
    bar,
    row,
    center,
    graph,
    workspace,
    frames,
    changed,
    layout: (next: { top?: number; left?: number; bottom?: number; toolbarBottom?: number }) => {
      top = next.top ?? top;
      left = next.left ?? left;
      bottom = next.bottom ?? bottom;
      toolbarBottom = next.toolbarBottom ?? toolbarBottom;
      window.dispatchEvent(new Event('resize'));
      frames.advance(16);
    },
    feedback: (height: number) => {
      const reserved = menu.classList.contains('sc-output-workspace-topmenu')
        ? Number.parseFloat(menu.style.getPropertyValue('--sc-output-toolbar-height'))
        : 56 + height;
      top = 36 + reserved;
      bottom = top + (800 - top) / 2;
      window.dispatchEvent(new Event('resize'));
      frames.advance(16);
    },
    dispose: () => {
      workspace.dispose();
      body.remove();
      jest.restoreAllMocks();
    },
  };
}

test('keeps a stable renderer sibling while progress and expanded jobs grow the native overlay', () => {
  const f = fixture();
  const frame = f.workspace.content.parentElement!;
  expect(frame.parentElement).toBe(f.root);
  expect(frame.style.top).toBe('92px');
  expect(frame.style.height).toBe('708px');
  const content = f.workspace.content;
  const changed = f.changed.mock.calls.length;
  f.layout({ top: 184 });
  f.layout({ top: 420 });
  expect(frame.style.top).toBe('92px');
  expect(frame.style.height).toBe('708px');
  expect(f.workspace.content).toBe(content);
  expect(f.graph.parentElement).toBe(f.root);
  expect(f.changed).toHaveBeenCalledTimes(changed);
  f.dispose();
  expect(frame.isConnected).toBe(false);
  expect(f.frames.pending).toBe(0);
});

test('follows deliberate toolbar, sidebar and bottom-panel allocation without moving native controls', () => {
  const f = fixture();
  const frame = f.workspace.content.parentElement!;
  const toolbarParent = f.bar.parentElement;
  f.layout({ left: 360, bottom: 600, toolbarBottom: 120, top: 420 });
  expect(frame.style.left).toBe('360px');
  expect(frame.style.width).toBe('1100px');
  expect(frame.style.top).toBe('124px');
  expect(frame.style.height).toBe('476px');
  expect(f.bar.parentElement).toBe(toolbarParent);
  f.dispose();
});

test('ignores unrelated popup content and retains content when the canvas is closed', async () => {
  const f = fixture();
  const frame = f.workspace.content.parentElement!;
  const count = f.changed.mock.calls.length;
  const popup = document.createElement('div');
  popup.textContent = '<b>Literal popup</b>';
  f.menu.append(popup);
  await Promise.resolve();
  expect(f.frames.pending).toBe(0);
  expect(f.changed).toHaveBeenCalledTimes(count);
  f.workspace.preferences({ visible: false, width: 560 });
  expect(frame.parentElement).toBe(f.root);
  expect(f.workspace.content.parentElement).toBe(frame);
  f.dispose();
});

test('reserves only toolbar space so a proportional bottom splitter stays stable during execution feedback', () => {
  const f = fixture();
  const frame = f.workspace.content.parentElement!;
  const rule = [...document.styleSheets]
    .flatMap((sheet) => [...sheet.cssRules])
    .filter((entry): entry is CSSStyleRule => entry instanceof CSSStyleRule)
    .find((entry) => entry.selectorText === '.sc-output-workspace-topmenu');
  expect(rule?.style.height).toBe('var(--sc-output-toolbar-height)');
  expect(rule?.style.getPropertyValue('flex-shrink')).toBe('0');
  f.feedback(0);
  const height = frame.style.height;
  const changed = f.changed.mock.calls.length;
  f.feedback(130);
  f.feedback(346);
  expect(f.menu.style.getPropertyValue('--sc-output-toolbar-height')).toBe('56px');
  expect(frame.style.top).toBe('92px');
  expect(frame.style.height).toBe(height);
  expect(f.changed).toHaveBeenCalledTimes(changed);
  f.dispose();
  expect(f.menu.classList.contains('sc-output-workspace-topmenu')).toBe(false);
  expect(f.menu.style.getPropertyValue('--sc-output-toolbar-height')).toBe('');
});

test('releases old toolbar styles and follows a replacement without remounting workflow content', async () => {
  const f = fixture();
  const original = f.menu;
  const next = document.createElement('div');
  next.style.rowGap = '4px';
  next.style.setProperty('--sc-output-toolbar-height', '64px', 'important');
  const row = document.createElement('div');
  const bar = document.createElement('div');
  bar.className = 'actionbar-container';
  row.append(bar);
  next.append(row);
  jest.spyOn(next, 'getBoundingClientRect').mockImplementation(() => bounds(60, 36, 1400, 96));
  jest.spyOn(row, 'getBoundingClientRect').mockImplementation(() => bounds(60, 40, 1400, 88));
  f.center.replaceChild(next, original);
  await Promise.resolve();
  f.frames.advance(16);
  expect(original.classList.contains('sc-output-workspace-topmenu')).toBe(false);
  expect(original.style.getPropertyValue('--sc-output-toolbar-height')).toBe('');
  expect(next.style.getPropertyValue('--sc-output-toolbar-height')).toBe('96px');
  expect(f.workspace.content.parentElement?.style.top).toBe('132px');
  f.dispose();
  expect(next.style.getPropertyValue('--sc-output-toolbar-height')).toBe('64px');
  expect(next.style.getPropertyPriority('--sc-output-toolbar-height')).toBe('important');
  expect(next.classList.contains('sc-output-workspace-topmenu')).toBe(false);
});

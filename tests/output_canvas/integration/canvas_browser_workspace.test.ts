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
  const model = new CanvasWorkspaceLayout();
  const actions = {
    width: jest.fn<(width: number) => void>(),
    close: jest.fn(),
    changed: jest.fn(),
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
  expect(f.workspace.content.style.left).toBe('560px');
  expect(f.root.style.marginLeft).toBe('560px');
  const borrowed = document.createElement('input');
  f.workspace.content.append(borrowed);
  f.workspace.suspend(true);
  expect(f.workspace.visible).toBe(false);
  expect(f.workspace.content.contains(borrowed)).toBe(true);
  expect(f.root.style.marginLeft).toBe('');
  f.workspace.suspend(false);
  f.workspace.preferences({ visible: false, width: 600 });
  expect(f.workspace.content.style.left).toBe('0px');
  f.workspace.preferences({ visible: true, width: 600 });
  expect(f.workspace.content.contains(borrowed)).toBe(true);
  expect(f.workspace.content.style.left).toBe('600px');
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

test('requires explicit narrow activation and exposes authoritative keyboard width bounds', () => {
  const f = fixture(800);
  expect(f.workspace.visible).toBe(false);
  f.workspace.drawer(true);
  expect(f.workspace.visible).toBe(true);
  expect(f.root.style.marginLeft).toBe('');
  expect(f.workspace.content.style.left).toBe('0px');
  const divider = f.panel.querySelector('[role="separator"]')!;
  expect(divider.getAttribute('aria-valuenow')).toBe('640');
  expect(divider.getAttribute('aria-valuemax')).toBe('776');
  expect(f.panel.querySelector<HTMLElement>('.sc-output-workspace-backdrop')!.style.left).toBe(
    '640px',
  );
  expect(document.activeElement?.getAttribute('aria-label')).toBe('Close output canvas');
  f.panel.querySelector<HTMLButtonElement>('.sc-output-workspace-backdrop')!.click();
  expect(f.actions.close).toHaveBeenCalledTimes(1);
  f.resize(1400);
  f.frames.advance(16);
  expect(f.root.style.marginLeft).toBe('560px');
  f.resize(800);
  f.frames.advance(16);
  expect(f.workspace.visible).toBe(false);
  f.dispose();
});

test('coalesces workspace measurements and cancels pending observation on teardown', () => {
  const f = fixture();
  const changed = f.actions.changed.mock.calls.length;
  for (let width = 1400; width < 1500; width++) f.resize(width);
  expect(f.frames.pending).toBe(1);
  expect(f.actions.changed).toHaveBeenCalledTimes(changed);
  f.frames.advance(16);
  expect(f.workspace.content.style.left).toBe('599.6px');
  expect(f.actions.changed).toHaveBeenCalledTimes(changed + 1);
  f.resize(1600);
  f.dispose();
  expect(f.frames.pending).toBe(0);
});

test('rolls back owned DOM, listeners, and graph geometry when initial allocation publication fails', () => {
  const f = fixture();
  f.workspace.dispose();
  expect(
    () =>
      new BrowserCanvasWorkspace(f.root, f.panel, f.model, f.frames, {
        width: () => undefined,
        close: () => undefined,
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

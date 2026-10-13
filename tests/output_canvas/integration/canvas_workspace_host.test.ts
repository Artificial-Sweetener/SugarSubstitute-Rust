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
/** Prove identified mounts, public app/configure lifecycles, accepted preferences, and unrelated popup isolation. */

import { jest } from '@jest/globals';
import { ComfyCanvasWorkspaceHost } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyCanvasWorkspaceHost.js';
import { ComfyCanvasPreferences } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyCanvasPreferences.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';

interface Setting {
  readonly id: string;
  readonly onChange: (value: unknown) => void;
}

/** Compose the actual observer and layout while providing only public Comfy settings/store and DOM geometry. */
function fixture() {
  const definitions = new Map<string, Setting>();
  const values = new Map<string, unknown>();
  const listeners = new Set<() => void>();
  const workflow = {
    activeWorkflow: { activeMode: 'graph' },
    $subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  const app = {
    extensionManager: { workflow },
    ui: {
      settings: {
        addSetting: (setting: Setting) => definitions.set(setting.id, setting),
        getSettingValue: (id: string, fallback: unknown) => values.get(id) ?? fallback,
        setSettingValueAsync: async (id: string, value: unknown) => {
          definitions.get(id)!.onChange(value);
          values.set(id, value);
        },
      },
    },
  };
  const preferences = new ComfyCanvasPreferences(app);
  preferences.setup();
  const frames = new CanvasFrameFixture();
  const changed = jest.fn();
  const report = jest.fn();
  const host = new ComfyCanvasWorkspaceHost(app, document, preferences, frames, changed, report);
  host.setup();
  const root = document.createElement('div');
  root.id = 'graph-canvas-container';
  const chrome = document.createElement('div');
  const panel = document.createElement('div');
  panel.className = 'graph-canvas-panel';
  jest.spyOn(panel, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 1400,
    bottom: 800,
    width: 1400,
    height: 800,
    toJSON: () => ({}),
  });
  chrome.append(panel);
  const canvas = document.createElement('canvas');
  canvas.id = 'graph-canvas';
  root.append(chrome, canvas);
  const frame = document.createElement('div');
  frame.className = 'comfyui-body';
  frame.append(root);
  document.body.append(frame);
  return {
    host,
    preferences,
    frames,
    changed,
    report,
    panel,
    root,
    listeners,
    mode: (mode: string) => {
      workflow.activeWorkflow.activeMode = mode;
      for (const listener of listeners) listener();
    },
    dispose: () => {
      host.dispose();
      preferences.dispose();
      root.remove();
      jest.restoreAllMocks();
    },
  };
}

test('mounts identified surfaces and preserves allocated content through app/configure suspension', async () => {
  const f = fixture();
  await Promise.resolve();
  const content = f.host.content;
  expect(content).not.toBeNull();
  expect(f.host.canvas).not.toBeNull();
  f.mode('app');
  expect(f.host.canvas).toBeNull();
  expect(f.host.content).toBe(content);
  expect(f.root.style.marginLeft).toBe('');
  f.mode('graph');
  expect(f.host.canvas).not.toBeNull();
  f.host.beforeConfigureGraph();
  expect(f.host.canvas).toBeNull();
  f.host.afterConfigureGraph();
  expect(f.host.content).toBe(content);
  f.host.toggle();
  expect(f.host.canvas).toBeNull();
  await Promise.resolve();
  expect(f.preferences.snapshot.visible).toBe(false);
  f.host.toggle();
  expect(f.host.canvas).not.toBeNull();
  f.dispose();
  expect(f.listeners.size).toBe(0);
  expect(f.frames.pending).toBe(0);
  expect(f.report).not.toHaveBeenCalled();
});

test('preserves canvas allocation during qualified history restoration while respecting app mode', async () => {
  const f = fixture();
  await Promise.resolve();
  const canvas = f.host.canvas;
  f.host.beforeConfigureGraph(true);
  expect(f.host.canvas).toBe(canvas);
  f.mode('app');
  expect(f.host.canvas).toBeNull();
  f.mode('graph');
  expect(f.host.canvas).toBe(canvas);
  f.host.afterConfigureGraph();
  expect(f.host.canvas).toBe(canvas);
  f.dispose();
});

test('ignores popup churn and retires removed workspace allocation before any later native mount', async () => {
  const f = fixture();
  await Promise.resolve();
  const content = f.host.content;
  const changes = f.changed.mock.calls.length;
  for (let index = 0; index < 100; index++) {
    const popup = document.createElement('div');
    popup.className = 'native-popup';
    document.body.append(popup);
    popup.textContent = '<b>Literal popup</b>';
    popup.remove();
  }
  await Promise.resolve();
  expect(f.host.content).toBe(content);
  expect(f.changed).toHaveBeenCalledTimes(changes);
  f.root.remove();
  await Promise.resolve();
  expect(f.host.canvas).toBeNull();
  expect(f.host.content).toBeNull();
  expect(f.root.style.marginLeft).toBe('');
  f.dispose();
});

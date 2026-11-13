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
/** Prove prompt keystrokes leave LiteGraph's keyup repaint path while graph keys keep it. */
import { jest } from '@jest/globals';
import { ComfyCanvasEditorKeyBoundary } from '../../../frontend/comfyui/ui/prompt_editor/ComfyCanvasEditorKeyBoundary.js';
import { CubePromptEditorHost } from '../../../frontend/comfyui/ui/prompt_editor/CubePromptEditorHost.js';
import { AppearanceState } from '../../../frontend/comfyui/ui/surface/appearance/AppearanceState.js';

/**
 * The LiteGraph contract this boundary relies on (ComfyUI frontend 1.47.11, LGraphCanvas):
 * `bindEvents` stores `this._key_callback = this.processKey.bind(this)` and registers it with
 * `document.addEventListener('keyup', this._key_callback, true)`; `processKey` returns early only
 * for `<input>` targets, clears space-drag state on a space keyup, and then calls `graph.change()`.
 */
class LiteGraphCanvas {
  read_only = false;
  readonly graph = { change: jest.fn() };
  _key_callback?: (event: KeyboardEvent) => void;

  bindEvents(): void {
    this._key_callback = this.processKey.bind(this);
    document.addEventListener('keyup', this._key_callback, true);
  }

  processKey(event: KeyboardEvent): void {
    if ((event.target as Element | null)?.localName === 'input') return;
    if (event.type === 'keyup' && event.key === ' ') this.read_only = false;
    this.graph.change();
  }
}

const listeners: Array<() => void> = [];
let editor: HTMLElement;
let graphKeys: HTMLElement;

beforeEach(() => {
  document.body.innerHTML =
    '<div data-sc-prompt-editor><div class="cm-content" contenteditable="true"></div></div>' +
    '<div class="graph-keys" tabindex="0"></div>';
  editor = document.querySelector<HTMLElement>('.cm-content')!;
  graphKeys = document.querySelector<HTMLElement>('.graph-keys')!;
});

afterEach(() => {
  for (const release of listeners.splice(0)) release();
});

/** Bind one canvas like LiteGraph and release its listener after the test. */
function bindCanvas(): LiteGraphCanvas {
  const canvas = new LiteGraphCanvas();
  canvas.bindEvents();
  const callback = canvas._key_callback!;
  listeners.push(() => document.removeEventListener('keyup', callback, true));
  return canvas;
}

function keyup(target: Element, key: string): void {
  target.dispatchEvent(new KeyboardEvent('keyup', { key, bubbles: true }));
}

test('withholds prompt keyups from LiteGraph and forwards every graph keyup once', () => {
  const canvas = bindCanvas();
  const report = jest.fn();
  const boundary = new ComfyCanvasEditorKeyBoundary({ canvas }, document, report);
  boundary.install();
  boundary.install();
  for (const key of ['a', ' ', 'Backspace', 'Enter']) keyup(editor, key);
  expect(canvas.graph.change).not.toHaveBeenCalled();
  keyup(graphKeys, 'a');
  keyup(document.body, 'Delete');
  expect(canvas.graph.change).toHaveBeenCalledTimes(2);
  // Other keyup listeners, including the editor's own, still observe every key.
  const observed = jest.fn();
  editor.addEventListener('keyup', observed);
  keyup(editor, 'b');
  expect(observed).toHaveBeenCalledTimes(1);
  expect(report).not.toHaveBeenCalled();
  boundary.dispose();
});

test('still ends a space-drag that LiteGraph began when space is released in the editor', () => {
  const canvas = bindCanvas();
  const boundary = new ComfyCanvasEditorKeyBoundary({ canvas }, document, jest.fn());
  boundary.install();
  canvas.read_only = true;
  keyup(editor, ' ');
  expect(canvas.read_only).toBe(false);
  expect(canvas.graph.change).toHaveBeenCalledTimes(1);
  boundary.dispose();
});

test('restores LiteGraph registration on dispose', () => {
  const canvas = bindCanvas();
  const boundary = new ComfyCanvasEditorKeyBoundary({ canvas }, document, jest.fn());
  boundary.install();
  boundary.dispose();
  keyup(editor, 'a');
  keyup(graphKeys, 'a');
  expect(canvas.graph.change).toHaveBeenCalledTimes(2);
});

test('reports a changed LiteGraph keyup contract and leaves native handling in place', () => {
  const canvas = bindCanvas();
  const report = jest.fn<(error: Error) => void>();
  const renamed = { graph: canvas.graph, processKey: canvas.processKey };
  new ComfyCanvasEditorKeyBoundary({ canvas: renamed }, document, report).install();
  expect(report).toHaveBeenCalledTimes(1);
  expect(report.mock.calls[0]?.[0].message).toMatch(/_key_callback is not a function/u);
  keyup(editor, 'a');
  expect(canvas.graph.change).toHaveBeenCalledTimes(1);
  new ComfyCanvasEditorKeyBoundary({}, document, report).install();
  expect(report).toHaveBeenCalledTimes(2);
});

test('retires without double handling when LiteGraph rebinds its keyup callback', () => {
  const canvas = bindCanvas();
  const report = jest.fn<(error: Error) => void>();
  const boundary = new ComfyCanvasEditorKeyBoundary({ canvas }, document, report);
  boundary.install();
  canvas.bindEvents();
  const rebound = canvas._key_callback!;
  listeners.push(() => document.removeEventListener('keyup', rebound, true));
  keyup(graphKeys, 'a');
  expect(canvas.graph.change).toHaveBeenCalledTimes(1);
  expect(report.mock.calls[0]?.[0].message).toMatch(/rebound/u);
  keyup(graphKeys, 'a');
  expect(canvas.graph.change).toHaveBeenCalledTimes(2);
  boundary.dispose();
  keyup(graphKeys, 'a');
  expect(canvas.graph.change).toHaveBeenCalledTimes(3);
});

/** Create a prompt host with an inert catalog; only its canvas key lifetime is exercised. */
function promptHost(): CubePromptEditorHost {
  return new CubePromptEditorHost(
    document,
    {
      load: () => new Promise(() => undefined),
      watch: () => () => undefined,
      prioritize: async () => undefined,
    },
    jest.fn(),
    new AppearanceState(),
  );
}

test('the prompt host holds the boundary for its lifetime and logs a changed contract', () => {
  const canvas = bindCanvas();
  const logger = { error: jest.fn() };
  const host = promptHost();
  host.setup(null, { app: { canvas }, logger });
  keyup(editor, 'a');
  expect(canvas.graph.change).not.toHaveBeenCalled();
  host.dispose();
  keyup(editor, 'a');
  expect(canvas.graph.change).toHaveBeenCalledTimes(1);
  expect(logger.error).not.toHaveBeenCalled();
  const changed = promptHost();
  changed.setup(null, { app: { canvas: {} }, logger });
  expect(logger.error).toHaveBeenCalledWith(
    '[SugarCubes] Prompt editor canvas key boundary unavailable.',
    { error: expect.objectContaining({ message: expect.stringMatching(/_key_callback/u) }) },
  );
  changed.dispose();
});

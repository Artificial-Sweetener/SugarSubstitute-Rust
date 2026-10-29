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
/** Exercise renderer-independent discrete keys through the actual shared command request boundary. */
import { jest } from '@jest/globals';
import { CanvasViewportKeyboard } from '../../../frontend/comfyui/ui/output_canvas/presentation/CanvasViewportKeyboard.js';
import { CanvasPortFixture, canvasSnapshot } from '../support/CanvasPortFixture.js';
import { canvasContextActions } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasContextActions.js';

/** Supply only a DOM measurement boundary; request IDs and command dispatch stay real. */
function fixture(
  commands: ConstructorParameters<typeof CanvasPortFixture>[1] = ['fit', 'native', 'zoom', 'pan'],
) {
  const element = document.createElement('div');
  document.body.append(element);
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 500,
    bottom: 400,
    width: 500,
    height: 400,
    toJSON: () => ({}),
  });
  const port = new CanvasPortFixture(canvasSnapshot('shared-keys'), commands);
  const errors: unknown[] = [];
  const keyboard = new CanvasViewportKeyboard(element, port, (error) => errors.push(error));
  const key = (value: string, target: HTMLElement = element, modifiers: KeyboardEventInit = {}) => {
    const event = new KeyboardEvent('keydown', {
      key: value,
      bubbles: true,
      cancelable: true,
      ...modifiers,
    });
    target.dispatchEvent(event);
    return event;
  };
  return { element, port, errors, keyboard, key };
}

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('routes Fit, physical100%, zoom steps and normalized arrow pan through one port sequence', () => {
  const f = fixture();
  for (const key of ['0', '1', '+', '-', 'ArrowLeft', 'ArrowDown'])
    expect(f.key(key).defaultPrevented).toBe(true);
  expect(f.port.requests.map((request) => request.requestId)).toEqual([1, 2, 3, 4, 5, 6]);
  expect(f.port.requests.map((request) => request.command)).toEqual([
    { type: 'fit' },
    { type: 'native' },
    { type: 'zoom', factor: 1.25 },
    { type: 'zoom', factor: 0.8 },
    { type: 'pan', x: 0.08, y: 0, units: 'viewport' },
    { type: 'pan', x: 0, y: -0.1, units: 'viewport' },
  ]);
  f.keyboard.dispose();
});

test('leaves unsupported, modified and editable-focus keys with their original owner', () => {
  const f = fixture(['fit']);
  const field = document.createElement('input');
  f.element.append(field);
  expect(f.key('1').defaultPrevented).toBe(false);
  expect(f.key('ArrowLeft').defaultPrevented).toBe(false);
  expect(f.key('0', field).defaultPrevented).toBe(false);
  for (const modifiers of [
    { ctrlKey: true },
    { metaKey: true },
    { altKey: true },
    { shiftKey: true },
  ])
    expect(f.key('0', f.element, modifiers).defaultPrevented).toBe(false);
  expect(f.port.requests).toHaveLength(0);
  f.keyboard.dispose();
  expect(f.key('0').defaultPrevented).toBe(false);
});

test('projects the same capability-gated menu commands for both native and browser surfaces', () => {
  const f = fixture();
  const sent: unknown[] = [];
  const snapshot = {
    ...f.port.read(),
    resource: { state: 'ready' as const, message: '', canRetry: false, canAdmitLargeImage: false },
  };
  const entries = canvasContextActions(snapshot, f.port.capabilities, (command) =>
    sent.push(command),
  );
  expect(entries.map((entry) => entry.label)).toEqual(['Fit image', '100%', 'Zoom in', 'Zoom out']);
  for (const entry of entries) entry.select?.();
  expect(sent).toEqual([
    { type: 'fit' },
    { type: 'native' },
    { type: 'zoom', factor: 1.25 },
    { type: 'zoom', factor: 0.8 },
  ]);
  const limited = canvasContextActions(snapshot, { schema: 1, commands: ['fit'] }, () => {});
  expect(limited.map((entry) => entry.label)).toEqual(['Fit image']);
  const retained = canvasContextActions(
    { ...snapshot, resource: { ...snapshot.resource, state: 'loading' } },
    f.port.capabilities,
    () => {},
  );
  expect(retained.every((entry) => !entry.disabled)).toBe(true);
  for (const unavailable of [
    { ...snapshot, zoom: null },
    { ...snapshot, zoom: { ...snapshot.zoom!, dimensions: null } },
    { ...snapshot, view: { ...snapshot.view, mode: 'overview' as const } },
  ])
    expect(
      canvasContextActions(unavailable, f.port.capabilities, () => {}).every(
        (entry) => entry.disabled,
      ),
    ).toBe(true);
  const preview = canvasContextActions(
    { ...snapshot, preview: true },
    f.port.capabilities,
    () => {},
  );
  expect(preview.every((entry) => entry.disabled)).toBe(true);
  f.keyboard.dispose();
});

test('consumes navigation keys without mutation while preview pixels are displayed, independently of running inference', () => {
  const f = fixture();
  f.port.publish({ ...f.port.read(), preview: true, runStatus: 'running' });
  for (const value of ['0', '1', '+', '-', 'ArrowLeft', 'ArrowDown'])
    expect(f.key(value).defaultPrevented).toBe(true);
  expect(f.port.requests).toHaveLength(0);
  f.port.publish({ ...f.port.read(), preview: false });
  for (const value of ['0', '1', '+', '-', 'ArrowLeft', 'ArrowDown'])
    expect(f.key(value).defaultPrevented).toBe(true);
  expect(f.port.requests).toHaveLength(6);
  f.keyboard.dispose();
});

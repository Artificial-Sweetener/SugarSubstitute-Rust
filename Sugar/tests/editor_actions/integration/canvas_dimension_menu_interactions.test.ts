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
/** Prove manual scaling, discovery and cancellation through the existing shared context menu. */
import { jest } from '@jest/globals';
import { ContextMenu } from '../../../frontend/comfyui/ui/menus/ContextMenu.js';
import { canvasDimensionMenuEntries } from '../../../frontend/comfyui/ui/editor_actions/presentation/CanvasDimensionMenu.js';
import { CanvasDimensionCommands } from '../../../frontend/comfyui/ui/editor_actions/application/CanvasDimensionCommands.js';
import type { CanvasDimensionConfiguration } from '../../../integrations/contracts/CanvasGeometry.js';
import type {
  CanvasGeometryCommand,
  CanvasGeometryResult,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';

/** Retain real command and menu owners while substituting the external native canvas and probe. */
function fixture() {
  const values = { width: 768, height: 512 };
  const context = {
    kind: 'context',
    supported: true,
    resize: true,
    selectionSupported: true,
    document: { id: 'doc', geometryRevision: 'frame', width: 1920, height: 1080 },
    selection: { left: 10, top: 20, width: 400, height: 240 },
  } as const;
  const commands: CanvasGeometryCommand[] = [];
  const execute = async (command: CanvasGeometryCommand): Promise<CanvasGeometryResult> => {
    commands.push(command);
    return command.type === 'context'
      ? context
      : { kind: 'resized', document: { ...context.document, ...command.size } };
  };
  const discover = jest.fn<() => Promise<number>>().mockResolvedValue(2);
  const dispose = jest.fn<() => void>();
  const prepare = jest.fn(async () => ({
    outputs: [
      { id: 'first', label: 'First <output>' },
      { id: 'second', label: 'Second output' },
    ],
    discover,
    dispose,
  }));
  const app = new CanvasDimensionCommands(() => ({ canvasId: 'native', execute }), { prepare });
  const binding = {
    width: {
      read: () => values.width,
      write: (value: number) => {
        values.width = value;
        return true;
      },
      disabled: () => false,
      limits: () => ({ minimum: 8, maximum: 16384, multiple: 8 }),
    },
    height: {
      read: () => values.height,
      write: (value: number) => {
        values.height = value;
        return true;
      },
      disabled: () => false,
      limits: () => ({ minimum: 8, maximum: 16384, multiple: 8 }),
    },
  };
  const pending: Promise<void>[] = [];
  let ready: ((configuration: CanvasDimensionConfiguration) => void) | null = null;
  const next = (): Promise<CanvasDimensionConfiguration> =>
    new Promise((resolve) => {
      ready = resolve;
    });
  const menu = new ContextMenu(document);
  menu.open({
    label: 'Dimensions',
    anchor: { kind: 'pointer', x: 10, y: 10 },
    entries: canvasDimensionMenuEntries(
      true,
      (action, configure, cancellation) =>
        app.execute(action, binding, cancellation, async (request, signal) => {
          if (!configure) throw new Error('Missing menu configuration');
          const result = configure(request, signal);
          ready?.(request);
          ready = null;
          return result;
        }),
      (operation) => {
        pending.push(operation);
      },
    ),
  });
  /** Activate a shared command row by its literal label. */
  const click = (label: string): void => {
    const row = Array.from(
      document.querySelectorAll<HTMLElement>('[role=menuitem],[role=menuitemradio]'),
    ).find((element) => element.textContent === label);
    if (!row) throw new Error('Missing menu command: ' + label);
    row.click();
  };
  /** Read the sole native editable control in the active submenu. */
  const input = (): HTMLInputElement => {
    const element = document.querySelector('[aria-label="Scale factor"]');
    if (!(element instanceof HTMLInputElement)) throw new Error('Missing scale field');
    return element;
  };
  /** Edit through native input events rather than bypassing menu state. */
  const scale = (value: string): void => {
    input().value = value;
    input().dispatchEvent(new Event('input', { bubbles: true }));
  };
  return { menu, values, commands, prepare, discover, dispose, pending, next, click, input, scale };
}

afterEach(() => {
  document.body.replaceChildren();
});

test('edits a factor and applies the visible constrained dimensions without rebuilding the input', async () => {
  const f = fixture();
  const ready = f.next();
  f.click('Set size from canvas with scaling');
  await ready;
  const input = f.input();
  input.focus();
  f.scale('1.5');
  expect(document.body.textContent).toContain('Size: 1280 × 720 px');
  expect(f.input()).toBe(input);
  expect(document.activeElement).toBe(input);
  const key = new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true, cancelable: true });
  input.dispatchEvent(key);
  expect(key.defaultPrevented).toBe(false);
  f.click('Apply dimensions');
  await Promise.all(f.pending);
  expect(f.values).toEqual({ width: 1280, height: 720 });
  expect(f.menu.isOpen()).toBe(false);
  expect(f.prepare).not.toHaveBeenCalled();
  expect(f.commands.map((command) => command.type)).toEqual(['context', 'context']);
});

test('discovers an explicitly selected output and keeps the measured factor editable before Apply', async () => {
  const f = fixture();
  let ready = f.next();
  f.click('Set size from canvas with scaling');
  await ready;
  const input = f.input();
  ready = f.next();
  f.click('Discover scale factor');
  await ready;
  expect(f.discover).not.toHaveBeenCalled();
  f.click('Choose output path');
  expect(document.querySelector('output')).toBeNull();
  f.click('Second output');
  expect(document.querySelectorAll('[role=menu]')).toHaveLength(2);
  ready = f.next();
  f.click('Discover scale factor');
  await ready;
  expect(f.discover).toHaveBeenCalledWith('second', expect.anything());
  expect(f.input()).toBe(input);
  expect(input.value).toBe('2');
  expect(document.body.textContent).toContain('Size: 960 × 544 px');
  f.scale('3');
  expect(document.body.textContent).toContain('Size: 640 × 360 px');
  f.click('Apply dimensions');
  await Promise.all(f.pending);
  expect(f.values).toEqual({ width: 640, height: 360 });
  expect(f.dispose).toHaveBeenCalledTimes(1);
});

test('dismissal cancels pending sizing and invalid input disables Apply without changing fields', async () => {
  const f = fixture();
  const ready = f.next();
  f.click('Set size from canvas with scaling');
  await ready;
  f.scale('0');
  const apply = Array.from(document.querySelectorAll('[role=menuitem]')).find(
    (row) => row.textContent === 'Apply dimensions',
  );
  expect(apply?.getAttribute('aria-disabled')).toBe('true');
  f.click('Apply dimensions');
  expect(f.menu.isOpen()).toBe(true);
  f.menu.close();
  await Promise.all(f.pending);
  expect(f.values).toEqual({ width: 768, height: 512 });
  expect(f.commands).toHaveLength(1);
});

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
/** Verify SugarScript file drops compose with Comfy's normal file handling. */

import { jest } from '@jest/globals';
import { SugarScriptFileDropAdapter } from '../../../frontend/comfyui/ui/sugarscript/SugarScriptFileDropAdapter.js';

test('leaves every non-SugarScript drop untouched', () => {
  const importSource = jest.fn<() => Promise<unknown>>();
  const hostDrop = jest.fn();
  document.addEventListener('drop', hostDrop);
  const adapter = createAdapter(importSource);
  adapter.setup();

  const event = dropEvent([file('workflow.json', '{}')]);
  document.dispatchEvent(event);

  expect(event.defaultPrevented).toBe(false);
  expect(hostDrop).toHaveBeenCalledTimes(1);
  expect(importSource).not.toHaveBeenCalled();
  adapter.dispose();
  document.removeEventListener('drop', hostDrop);
});

test('captures one SugarScript file and imports its exact source', async () => {
  const importSource = jest.fn(async () => undefined);
  const hostDrop = jest.fn();
  document.addEventListener('drop', hostDrop);
  const adapter = createAdapter(importSource);
  adapter.setup();

  const event = dropEvent([file('Recipe.SUGAR', 'use "sdxl" as Base\n')]);
  document.dispatchEvent(event);
  await settleAsyncDrop();

  expect(event.defaultPrevented).toBe(true);
  expect(hostDrop).not.toHaveBeenCalled();
  expect(importSource).toHaveBeenCalledWith('use "sdxl" as Base\n');
  adapter.dispose();
  document.removeEventListener('drop', hostDrop);
});

test('rejects an oversized source before reading or importing it', async () => {
  const importSource = jest.fn(async () => undefined);
  const feedback = { push: jest.fn() };
  const oversized = file('large.sugar', 'ignored', 1_000_001);
  const adapter = new SugarScriptFileDropAdapter({
    document,
    importSource,
    feedback,
    readErrorMessage: (error) => String(error),
  });
  adapter.setup();

  document.dispatchEvent(dropEvent([oversized]));
  await settleAsyncDrop();

  expect(oversized.text).not.toHaveBeenCalled();
  expect(importSource).not.toHaveBeenCalled();
  expect(feedback.push).toHaveBeenCalledWith(
    'error',
    'SugarScript import failed',
    expect.stringContaining('exceeds 1 MB'),
  );
  adapter.dispose();
});

function createAdapter(
  importSource: (source: string) => Promise<unknown>,
): SugarScriptFileDropAdapter {
  return new SugarScriptFileDropAdapter({
    document,
    importSource,
    readErrorMessage: (error) => String(error),
  });
}

function file(name: string, source: string, size = source.length): File {
  return {
    name,
    size,
    text: jest.fn(async () => source),
  } as unknown as File;
}

function dropEvent(files: readonly File[]): DragEvent {
  const event = new Event('drop', { bubbles: true, cancelable: true }) as DragEvent;
  Object.defineProperty(event, 'dataTransfer', { value: { files } });
  return event;
}

async function settleAsyncDrop(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

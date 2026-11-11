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
/** Prove host app-mode validation, precise notifications, and subscription teardown. */
import { expect, jest, test } from '@jest/globals';
import { createComfyAppModeSource } from '../../../frontend/comfyui/ui/core/ComfyAppMode.js';

test.each([null, {}, { extensionManager: null }, { extensionManager: { workflow: 42 } }])(
  'supports older hosts without an app-mode store: %p',
  (host) => {
    const source = createComfyAppModeSource(host);
    expect(source.active).toBe(false);
    const changed = jest.fn();
    source.subscribe(changed)();
    expect(changed).not.toHaveBeenCalled();
  },
);

test('reads initial and active modes and ignores unrelated workflow mutations', () => {
  let notify = (): void => undefined;
  const unsubscribe = jest.fn();
  const workflow = {
    activeWorkflow: { initialMode: 'app', activeMode: undefined as string | undefined },
    $subscribe: jest.fn((listener: () => void) => {
      notify = listener;
      return unsubscribe;
    }),
  };
  const source = createComfyAppModeSource({ extensionManager: { workflow } });
  expect(source.active).toBe(true);
  const changed = jest.fn();
  const release = source.subscribe(changed);
  expect(workflow.$subscribe).toHaveBeenCalledWith(expect.any(Function), {
    detached: true,
    flush: 'sync',
  });
  for (const mode of [
    'app',
    'builder:arrange',
    'graph',
    'builder:inputs',
    'builder:outputs',
    'app',
  ]) {
    workflow.activeWorkflow.activeMode = mode;
    notify();
    expect(source.active).toBe(mode === 'app' || mode === 'builder:arrange');
  }
  expect(changed.mock.calls).toEqual([[false], [true]]);
  release();
  expect(unsubscribe).toHaveBeenCalledTimes(1);
  workflow.activeWorkflow.activeMode = 'graph';
  notify();
  expect(changed).toHaveBeenCalledTimes(2);
});

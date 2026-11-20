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
/** Prove graph suppression outlives card mounts without retaining native interaction changes. */
import { expect, test } from '@jest/globals';
import {
  ComfyQueueActions,
  QUEUE_PROGRESS_BAR_SETTING,
} from '../../../frontend/comfyui/ui/surface/frame/queue/ComfyQueueActions.js';

/** A ComfyUI whose stored settings are `stored`, with ComfyUI's own default of showing the bar. */
function comfy(stored: Record<string, unknown>) {
  const written: [string, unknown][] = [];
  const api = {
    fetchApi: async (route: string) =>
      ({
        ok: true,
        status: 200,
        json: async () => stored[decodeURIComponent(route.slice('/settings/'.length))] ?? null,
      }) as Response,
  };
  const app = {
    extensionManager: {
      setting: {
        get: (id: string) => stored[id] ?? true,
        set: async (id: string, value: unknown) => {
          stored[id] = value;
          written.push([id, value]);
        },
      },
    },
  };
  const actions = new ComfyQueueActions(
    api as ConstructorParameters<typeof ComfyQueueActions>[0],
    app,
    () => undefined,
  );
  return { actions, written };
}

test('a profile that never chose gets no run progress bar', async () => {
  const { actions, written } = comfy({});
  expect(actions.progressBar).toBe(true);

  await actions.withholdUnaskedProgressBar();

  expect(written).toEqual([[QUEUE_PROGRESS_BAR_SETTING, false]]);
  expect(actions.progressBar).toBe(false);
});

test('a stored choice is the person’s and is left alone, either way', async () => {
  for (const chosen of [true, false]) {
    const { actions, written } = comfy({ [QUEUE_PROGRESS_BAR_SETTING]: chosen });
    await actions.withholdUnaskedProgressBar();
    expect(written).toEqual([]);
    expect(actions.progressBar).toBe(chosen);
  }
});

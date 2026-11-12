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
import { expect, jest, test } from '@jest/globals';
import {
  CubeVersionToolboxController,
  type CubeVersionToolboxView,
} from '../../../frontend/comfyui/ui/affordance/CubeVersionToolboxController.js';
import type { CubeVersionToolboxModel } from '../../../frontend/comfyui/ui/affordance/ComfyCubeVersionToolboxPresenter.js';
import type { CubeVersionAvailabilityService } from '../../../frontend/comfyui/ui/cube/version/CubeVersionAvailabilityService.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';

test('coordinates lazy options and switches only the current selection', async () => {
  const anchor = document.createElement('button');
  const current = cube('1.0.0');
  const replacement = cube('2.0.0');
  const models: CubeVersionToolboxModel[] = [];
  const view: CubeVersionToolboxView = {
    present: (button, value) => {
      models.push(value);
      return button;
    },
    clear: jest.fn(),
    dispose: jest.fn(),
  };
  const list = jest.fn(async () => [
    { label: 'v2.0.0', value: '2.0.0', revisionRef: 'WORKTREE', current: true, raw: null },
    { label: 'v1.0.0', value: '1.0.0', revisionRef: 'abc', current: false, raw: null },
  ]);
  const switchVersion = jest.fn(async () => replacement);
  const controller = new CubeVersionToolboxController({
    availability: { list } as unknown as CubeVersionAvailabilityService,
    switcher: { switch: switchVersion },
    presenter: view,
    logger: console,
    reportError: jest.fn(),
  });

  controller.present(anchor, current);
  await Promise.resolve();
  await Promise.resolve();
  expect(list).toHaveBeenCalledWith('cube.test', '1.0.0');
  expect(models.at(-1)!.options).toHaveLength(2);
  models.at(-1)!.select('2.0.0');
  await Promise.resolve();
  await Promise.resolve();
  expect(switchVersion).toHaveBeenCalledWith(current, '2.0.0');
  expect(models.at(-1)!.currentVersion).toBe('2.0.0');
  expect(models.at(-1)!.options).toHaveLength(2);
  expect(list).toHaveBeenCalledTimes(1);
});

function cube(version: string): CubeNode {
  return {
    id: 'instance',
    title: 'Demo',
    pos: [0, 0],
    size: [400, 300],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: {
        instance_id: 'instance',
        cube_id: 'cube.test',
        cube_version: version,
      },
    },
    inputs: [],
    outputs: [],
    subgraph: {} as CubeNode['subgraph'],
    isSubgraphNode: () => true,
    serialize: () => ({}),
    connect: jest.fn(),
  };
}

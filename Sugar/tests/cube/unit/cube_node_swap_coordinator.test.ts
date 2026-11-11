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
/** Prove native face arrows share canonical permissions and report rejected actions. */
import { expect, jest, test } from '@jest/globals';
import { CubeNodeSwapCoordinator } from '../../../frontend/comfyui/ui/cube/node/CubeNodeSwapCoordinator.js';

test('translates both directions through one shared application owner', async () => {
  const canMove = jest.fn(() => true);
  const move = jest.fn(async () => undefined);
  const reportError = jest.fn();
  const coordinator = new CubeNodeSwapCoordinator({ reorder: { canMove, move }, reportError });
  expect(coordinator.canSwap({ instance_id: 'first' }, 'right')).toBe(true);
  await coordinator.swap({ instance_id: 'first' }, 'right');
  await coordinator.swap({ instance_id: 'second' }, 'left');
  expect(move.mock.calls).toEqual([
    ['first', 1],
    ['second', -1],
  ]);
  expect(reportError).not.toHaveBeenCalled();
});

test('keeps a canonically fixed segment disabled even when a Cube has an input', async () => {
  const canMove = jest.fn(() => false);
  const move = jest.fn(async () => undefined);
  const coordinator = new CubeNodeSwapCoordinator({
    reorder: { canMove, move },
    reportError: jest.fn(),
  });
  const metadata = { instance_id: 'first', graphSummary: { inputCount: 1 } };
  expect(coordinator.canSwap(metadata, 'right')).toBe(false);
  await coordinator.swap(metadata, 'right');
  expect(move).not.toHaveBeenCalled();
});

test('permits a source Cube when the canonical graph owner allows its segment', async () => {
  const move = jest.fn(async () => undefined);
  const coordinator = new CubeNodeSwapCoordinator({
    reorder: { canMove: () => true, move },
    reportError: jest.fn(),
  });
  expect(
    coordinator.canSwap({ instance_id: 'source', graphSummary: { inputCount: 0 } }, 'right'),
  ).toBe(true);
  await coordinator.swap({ instance_id: 'source' }, 'right');
  expect(move).toHaveBeenCalledWith('source', 1);
});

test('does not turn obsolete group markers or missing identifiers into actions', async () => {
  const canMove = jest.fn(() => true);
  const move = jest.fn(async () => undefined);
  const coordinator = new CubeNodeSwapCoordinator({
    reorder: { canMove, move },
    reportError: jest.fn(),
  });
  expect(coordinator.canSwap({ group_id: 'old' }, 'right')).toBe(false);
  await coordinator.swap({ instance_id: '  ' }, 'right');
  expect(canMove).not.toHaveBeenCalled();
  expect(move).not.toHaveBeenCalled();
});

test('reports backend or stale-workflow failures through SugarCubes feedback', async () => {
  const failure = new Error('Workflow changed');
  const reportError = jest.fn();
  const move = jest.fn(async () => {
    throw failure;
  });
  const coordinator = new CubeNodeSwapCoordinator({
    reorder: { canMove: () => true, move },
    reportError,
  });
  await coordinator.swap({ instance_id: 'first' }, 'right');
  expect(reportError).toHaveBeenCalledWith(failure);
});

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
/** Prove native face arrows move Cubes through the Cube chain and report refused moves. */
import { expect, jest, test } from '@jest/globals';
import { CubeNodeSwapCoordinator } from '../../../frontend/comfyui/ui/cube/node/CubeNodeSwapCoordinator.js';

test('translates both directions through the one chain', () => {
  const canMove = jest.fn(() => true);
  const move = jest.fn();
  const reportError = jest.fn();
  const coordinator = new CubeNodeSwapCoordinator({ chain: { canMove, move }, reportError });
  expect(coordinator.canSwap({ instance_id: 'first' }, 'right')).toBe(true);
  coordinator.swap({ instance_id: 'first' }, 'right');
  coordinator.swap({ instance_id: 'second' }, 'left');
  expect(move.mock.calls).toEqual([
    ['first', 1],
    ['second', -1],
  ]);
  expect(reportError).not.toHaveBeenCalled();
});

test('keeps an arrow off when the chain says the wiring cannot make that move', () => {
  const move = jest.fn();
  const coordinator = new CubeNodeSwapCoordinator({
    chain: { canMove: () => false, move },
    reportError: jest.fn(),
  });
  const metadata = { instance_id: 'first', graphSummary: { inputCount: 1 } };
  expect(coordinator.canSwap(metadata, 'right')).toBe(false);
  coordinator.swap(metadata, 'right');
  expect(move).not.toHaveBeenCalled();
});

test('does not turn obsolete group markers or missing identifiers into actions', () => {
  const canMove = jest.fn(() => true);
  const move = jest.fn();
  const coordinator = new CubeNodeSwapCoordinator({
    chain: { canMove, move },
    reportError: jest.fn(),
  });
  expect(coordinator.canSwap({ group_id: 'old' }, 'right')).toBe(false);
  coordinator.swap({ instance_id: '  ' }, 'right');
  expect(canMove).not.toHaveBeenCalled();
  expect(move).not.toHaveBeenCalled();
});

test('reports a refused move through SugarCubes feedback', () => {
  const failure = new Error('Upscale is wired to the Cube before it, so it cannot be first.');
  const reportError = jest.fn();
  const coordinator = new CubeNodeSwapCoordinator({
    chain: {
      canMove: () => true,
      move: () => {
        throw failure;
      },
    },
    reportError,
  });
  coordinator.swap({ instance_id: 'first' }, 'right');
  expect(reportError).toHaveBeenCalledWith(failure);
});

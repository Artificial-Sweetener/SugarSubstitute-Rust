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
/** Verify the persisted Cube-face state contract. */

import { describe, expect, test } from '@jest/globals';

import {
  createDefaultCubeSurfaceState,
  parseCubeSurfaceState,
  serializeCubeSurfaceState,
} from '../../../frontend/comfyui/ui/surface/CubeSurfaceState.js';

describe('CubeSurfaceState', () => {
  test('uses finite product defaults for old Cube instances', () => {
    expect(parseCubeSurfaceState(undefined)).toEqual(createDefaultCubeSurfaceState());
  });

  test('preserves order, per-card state, masonry sizing, and preview selection', () => {
    const state = parseCubeSurfaceState({
      schema: 3,
      node_order: ['9', '4'],
      cards: {
        '9': {
          authored_bypass: true,
          revealed: true,
          enabled_override: true,
          active_mode: 2,
        },
        '4': {
          authored_bypass: false,
          revealed: false,
          enabled_override: false,
          active_mode: 0,
        },
      },
      minimum_column_width: 280,
      gap: 14,
      preview: {
        visible: true,
        width: 360,
        selected_output: 'image',
      },
    });

    expect(serializeCubeSurfaceState(state)).toEqual({
      schema: 3,
      node_order: ['9', '4'],
      cards: {
        '9': {
          authored_bypass: true,
          revealed: true,
          enabled_override: true,
          active_mode: 2,
        },
        '4': {
          authored_bypass: false,
          revealed: false,
          enabled_override: false,
          active_mode: 0,
        },
      },
      minimum_column_width: 280,
      gap: 14,
      preview: {
        visible: true,
        width: 360,
        selected_output: 'image',
      },
    });
  });

  test('rejects invalid dynamic values at the persistence boundary', () => {
    expect(
      parseCubeSurfaceState({
        revealed: false,
        node_order: ['a', 7, 'a', 'b'],
        cards: {
          good: {
            authored_bypass: true,
            revealed: true,
            enabled_override: false,
            active_mode: 4,
          },
          invalid: { revealed: 'yes' },
        },
        minimum_column_width: Infinity,
        gap: -100,
        preview: { width: 'wide', selected_output: 99 },
      }),
    ).toEqual({
      ...createDefaultCubeSurfaceState(),
      nodeOrder: ['a', 'b'],
      cards: {
        good: {
          authoredBypass: true,
          revealed: true,
          enabledOverride: false,
          activeMode: 0,
        },
      },
      gap: 0,
    });
  });

  test('migrates legacy visibility into reveal state without retaining generic hiding', () => {
    expect(
      parseCubeSurfaceState({
        schema: 2,
        cards: {
          bypass: { visible: true, activation_control: false, active_mode: 0 },
          ordinary: { visible: false, activation_control: false, active_mode: 0 },
        },
      }).cards,
    ).toEqual({
      bypass: {
        authoredBypass: true,
        revealed: true,
        enabledOverride: null,
        activeMode: 0,
      },
      ordinary: {
        authoredBypass: false,
        revealed: false,
        enabledOverride: null,
        activeMode: 0,
      },
    });
  });

  test('ignores the removed whole-face reveal flag from schema-one instances', () => {
    expect(parseCubeSurfaceState({ schema: 1, revealed: false })).toEqual(
      createDefaultCubeSurfaceState(),
    );
  });
});

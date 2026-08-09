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
import { describe, expect, test } from '@jest/globals';
import { projectCubeVersionOptions } from '../../frontend/comfyui/ui/cube/version/CubeVersionOptionProjection.js';

describe('cube version option projection', () => {
  test('maps backend revision choices without owning history deduplication', () => {
    expect(
      projectCubeVersionOptions(
        [
          { revision_ref: 'WORKTREE', version: '2.0.0', current: true },
          { revision_ref: 'abc123', version: '1.0.0', current: false },
        ],
        '3.0.0',
      ),
    ).toEqual([
      {
        label: 'v2.0.0',
        value: '2.0.0',
        revisionRef: 'WORKTREE',
        current: true,
        raw: { revision_ref: 'WORKTREE', version: '2.0.0', current: true },
      },
      {
        label: 'v1.0.0',
        value: '1.0.0',
        revisionRef: 'abc123',
        current: false,
        raw: { revision_ref: 'abc123', version: '1.0.0', current: false },
      },
    ]);
  });

  test('uses the selected cube version only when revision history is empty', () => {
    expect(projectCubeVersionOptions([], 'v1.4.0')).toEqual([
      {
        label: 'v1.4.0',
        value: '1.4.0',
        revisionRef: 'WORKTREE',
        current: true,
        raw: null,
      },
    ]);
  });
});

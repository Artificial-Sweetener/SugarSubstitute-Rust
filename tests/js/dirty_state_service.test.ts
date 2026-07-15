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
import { DirtyStateService } from '../../web/comfyui/ui/graph/DirtyStateService.js';

describe('dirty state service', () => {
  test('separates implementation, cosmetic, and surface changes', () => {
    const service = new DirtyStateService();

    const result = service.evaluate({
      implementationCurrentHash: 'impl-next',
      implementationBaselineHash: 'impl-base',
      cosmeticCurrentHash: 'cosmetic-next',
      cosmeticBaselineHash: 'cosmetic-base',
      surfaceCurrentHash: 'surface-next',
      surfaceBaselineHash: 'surface-base',
      isKnown: true,
      missingSymbols: false,
      previousDirtyAt: null,
    });

    expect(result.implementationDirty).toBe(true);
    expect(result.cosmeticDirty).toBe(true);
    expect(result.surfaceValuesChanged).toBe(true);
    expect(result.implementationReasons).toEqual(['hash-mismatch']);
    expect(result.dirtyAt).toBeTruthy();
  });

  test('retains dirtyAt only for implementation changes', () => {
    const service = new DirtyStateService();

    const result = service.evaluate({
      implementationCurrentHash: 'same',
      implementationBaselineHash: 'same',
      cosmeticCurrentHash: 'changed',
      cosmeticBaselineHash: 'base',
      surfaceCurrentHash: 'same',
      surfaceBaselineHash: 'same',
      isKnown: true,
      missingSymbols: false,
      previousDirtyAt: '2024-01-01T00:00:00Z',
    });

    expect(result.implementationDirty).toBe(false);
    expect(result.cosmeticDirty).toBe(true);
    expect(result.surfaceValuesChanged).toBe(false);
    expect(result.dirtyAt).toBe(null);
  });
});

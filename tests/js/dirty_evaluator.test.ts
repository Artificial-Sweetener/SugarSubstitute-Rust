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
import { DirtyEvaluator } from '../../web/comfyui/ui/graph/DirtyEvaluator.js';

describe('dirty evaluator', () => {
  test('orders reasons deterministically', () => {
    const evaluator = new DirtyEvaluator();
    const result = evaluator.evaluate({
      currentHash: 'a',
      baselineHash: 'b',
      isKnown: false,
      missingSymbols: true,
      previousDirtyAt: null,
    });

    expect(result.reasons).toEqual(['unknown', 'missing-symbols', 'hash-mismatch']);
    expect(result.dirty).toBe(true);
    expect(result.dirtyAt).toBeTruthy();
  });

  test('retains dirtyAt when already dirty', () => {
    const evaluator = new DirtyEvaluator();
    const prior = '2024-01-01T00:00:00Z';
    const result = evaluator.evaluate({
      currentHash: 'a',
      baselineHash: 'b',
      isKnown: true,
      missingSymbols: false,
      previousDirtyAt: prior,
    });

    expect(result.dirty).toBe(true);
    expect(result.dirtyAt).toBe(prior);
  });

  test('clears dirtyAt when clean', () => {
    const evaluator = new DirtyEvaluator();
    const result = evaluator.evaluate({
      currentHash: 'a',
      baselineHash: 'a',
      isKnown: true,
      missingSymbols: false,
      previousDirtyAt: '2024-01-01T00:00:00Z',
    });

    expect(result.dirty).toBe(false);
    expect(result.dirtyAt).toBe(null);
    expect(result.reasons).toEqual([]);
  });
});

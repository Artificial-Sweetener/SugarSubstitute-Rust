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
import { BaselineResolver } from '../../frontend/comfyui/ui/graph/BaselineResolver.js';
import { BaselineStore } from '../../frontend/comfyui/ui/graph/BaselineStore.js';

describe('baseline resolver', () => {
  test('local baseline overrides definition', () => {
    const baselineStore = new BaselineStore();
    baselineStore.setDefinition('local/demo', { hash: 'def-hash', status: 'ready' });
    baselineStore.setLocalBaselineHash('inst-1', 'local-hash');
    const resolver = new BaselineResolver({ baselineStore });

    const result = resolver.resolve({
      cubeId: 'local/demo',
      instanceId: 'inst-1',
      missingSymbols: false,
    });

    expect(result).toEqual({
      baselineHash: 'local-hash',
      baselineSource: 'local',
      useDefinition: false,
    });
  });

  test('definition baseline is used when ready and symbols match', () => {
    const baselineStore = new BaselineStore();
    baselineStore.setDefinition('local/demo', { hash: 'def-hash', status: 'ready' });
    const resolver = new BaselineResolver({ baselineStore });

    const result = resolver.resolve({
      cubeId: 'local/demo',
      instanceId: 'inst-1',
      missingSymbols: false,
    });

    expect(result).toEqual({
      baselineHash: 'def-hash',
      baselineSource: 'definition',
      useDefinition: true,
    });
  });

  test('definition baseline is skipped when symbols are missing', () => {
    const baselineStore = new BaselineStore();
    baselineStore.setDefinition('local/demo', { hash: 'def-hash', status: 'ready' });
    const resolver = new BaselineResolver({ baselineStore });

    const result = resolver.resolve({
      cubeId: 'local/demo',
      instanceId: 'inst-1',
      missingSymbols: true,
    });

    expect(result).toEqual({
      baselineHash: null,
      baselineSource: 'local',
      useDefinition: false,
    });
  });

  test('definition errors fall back to local baseline', () => {
    const baselineStore = new BaselineStore();
    baselineStore.setDefinition('local/demo', { hash: 'def-hash', status: 'error' });
    baselineStore.setLocalBaselineHash('inst-1', 'local-hash');
    const resolver = new BaselineResolver({ baselineStore });

    const result = resolver.resolve({
      cubeId: 'local/demo',
      instanceId: 'inst-1',
      missingSymbols: false,
    });

    expect(result).toEqual({
      baselineHash: 'local-hash',
      baselineSource: 'local',
      useDefinition: false,
    });
  });
});

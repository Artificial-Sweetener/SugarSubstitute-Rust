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
/** Verify the focused native subgraph change integration boundary. */

import { jest } from '@jest/globals';

import { NativeSubgraphChangeObserver } from '../../frontend/comfyui/ui/surface/NativeSubgraphChangeObserver.js';

describe('NativeSubgraphChangeObserver', () => {
  test('chains native mutation methods and restores Comfy graph behavior', async () => {
    const previousAfterChange = jest.fn();
    const add = jest.fn<(node: string) => string>(() => 'added');
    const remove = jest.fn<(node: string) => string>(() => 'removed');
    const onChange = jest.fn();
    const subgraph = {
      onAfterChange: previousAfterChange,
      add,
      remove,
    };
    const observer = new NativeSubgraphChangeObserver(subgraph, onChange);

    subgraph.onAfterChange?.('graph', 'info');
    expect(subgraph.add('node')).toBe('added');
    expect(subgraph.remove('node')).toBe('removed');
    await Promise.resolve();

    expect(previousAfterChange).toHaveBeenCalledWith('graph', 'info');
    expect(add).toHaveBeenCalledWith('node');
    expect(remove).toHaveBeenCalledWith('node');
    expect(onChange).toHaveBeenCalledTimes(1);

    observer.dispose();
    expect(subgraph.onAfterChange).toBe(previousAfterChange);
    expect(subgraph.add).toBe(add);
    expect(subgraph.remove).toBe(remove);
  });

  test('does not overwrite a callback installed after attachment', () => {
    const subgraph = {
      onAfterChange: null as ((graph: unknown) => void) | null,
      add: jest.fn(),
      remove: jest.fn(),
    };
    const observer = new NativeSubgraphChangeObserver(subgraph, jest.fn());
    const replacement = jest.fn();
    subgraph.onAfterChange = replacement;

    observer.dispose();

    expect(subgraph.onAfterChange).toBe(replacement);
  });

  test('fails closed when Comfy lacks the native topology methods', () => {
    expect(() => new NativeSubgraphChangeObserver({ onAfterChange: null }, jest.fn())).toThrow(
      'Comfy native subgraph topology methods are unavailable',
    );
  });
});

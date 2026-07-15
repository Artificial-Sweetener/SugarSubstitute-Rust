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
import {
  CUBE_INSTANCE_HEADER_HEIGHT,
  CUBE_INSTANCE_PADDING,
  CUBE_INSTANCE_TOP_EXTRA,
  computeInstanceBounds,
  computeInnerBounds,
  expandBoundsToIncludeRect,
  inflateInstanceBounds,
  resolveChromeBoundsFromContent,
  resolveInstanceBounds,
  resolveNewInstanceBounds,
  writeCanonicalBounds,
} from '../../web/comfyui/ui/graph/CubeBounds.js';

describe('cube bounds', () => {
  test('computeInstanceBounds combines node and marker bounds', () => {
    const node = { pos: [10, 10], size: [20, 20] };
    const marker = { getBounding: () => [5, 5, 5, 5] };
    expect(computeInstanceBounds([node], [marker])).toEqual({
      minX: 5,
      minY: 5,
      maxX: 30,
      maxY: 30,
    });
  });

  test('computeInstanceBounds returns null when no bounds', () => {
    expect(computeInstanceBounds([], [])).toBeNull();
    expect(computeInstanceBounds([{}], null)).toBeNull();
  });

  test('computeInstanceBounds uses live node pos/size when getBounding is stale', () => {
    const node = {
      pos: [100, 120],
      size: [40, 50],
      getBounding: () => [10, 20, 40, 50],
    };

    expect(computeInstanceBounds([node], [])).toEqual({
      minX: 100,
      minY: 120,
      maxX: 140,
      maxY: 170,
    });
  });

  test('computeInstanceBounds uses collapsed visual rect for rolled-up nodes', () => {
    const originalLiteGraph = globalThis.LiteGraph;
    const originalTitleHeight = originalLiteGraph?.NODE_TITLE_HEIGHT;
    const originalCollapsedWidth = originalLiteGraph?.NODE_COLLAPSED_WIDTH;
    globalThis.LiteGraph = {
      ...(originalLiteGraph || {}),
      NODE_TITLE_HEIGHT: 30,
      NODE_COLLAPSED_WIDTH: 80,
    };
    try {
      const node = {
        flags: { collapsed: true },
        pos: [200, 90],
        size: [300, 200],
        _collapsed_width: 90,
        getBounding: () => [200, 60, 90, 30],
      };

      expect(computeInstanceBounds([node], [])).toEqual({
        minX: 200,
        minY: 60,
        maxX: 290,
        maxY: 90,
      });
    } finally {
      if (originalLiteGraph) {
        originalLiteGraph.NODE_TITLE_HEIGHT = originalTitleHeight;
        originalLiteGraph.NODE_COLLAPSED_WIDTH = originalCollapsedWidth;
        globalThis.LiteGraph = originalLiteGraph;
      } else {
        Reflect.deleteProperty(globalThis, 'LiteGraph');
      }
    }
  });

  test('inflateInstanceBounds uses padding and header defaults', () => {
    const inflated = inflateInstanceBounds({ minX: 0, minY: 0, maxX: 100, maxY: 50 });
    const padX = CUBE_INSTANCE_PADDING.x;
    const padY = CUBE_INSTANCE_PADDING.y;
    const padTop = padY + CUBE_INSTANCE_TOP_EXTRA;
    const padBottom = padY;
    expect(inflated).toEqual({
      x: 0 - padX,
      y: 0 - padTop - CUBE_INSTANCE_HEADER_HEIGHT,
      w: 100 + padX * 2,
      h: 50 + padTop + padBottom + CUBE_INSTANCE_HEADER_HEIGHT,
    });
  });

  test('resolveInstanceBounds prefers metadata bounds over group bounds', () => {
    const group = { _bounding: [10, 20, 30, 40] };
    const metadata = { bounds: { x: 1, y: 2, w: 3, h: 4, padding: { x: 1 } } };
    const resolved = resolveInstanceBounds({ group, metadata });
    expect(resolved).toEqual({
      x: 1,
      y: 2,
      w: 3,
      h: 4,
      padding: {
        x: 1,
        y: CUBE_INSTANCE_PADDING.y,
        top_extra: CUBE_INSTANCE_TOP_EXTRA,
      },
      header: {
        height: CUBE_INSTANCE_HEADER_HEIGHT,
      },
    });
  });

  test('resolveInstanceBounds falls back to group bounds', () => {
    const group = { _bounding: [10, 20, 30, 40] };
    expect(resolveInstanceBounds({ group, metadata: null })).toEqual({
      x: 10,
      y: 20,
      w: 30,
      h: 40,
      padding: {
        x: CUBE_INSTANCE_PADDING.x,
        y: CUBE_INSTANCE_PADDING.y,
        top_extra: CUBE_INSTANCE_TOP_EXTRA,
      },
      header: {
        height: CUBE_INSTANCE_HEADER_HEIGHT,
      },
    });
  });

  test('resolveInstanceBounds computes inflated bounds when needed', () => {
    const node = { pos: [10, 10], size: [10, 10] };
    const resolved = resolveInstanceBounds({ nodes: [node], markers: [] });
    const padX = CUBE_INSTANCE_PADDING.x;
    const padY = CUBE_INSTANCE_PADDING.y;
    const padTop = padY + CUBE_INSTANCE_TOP_EXTRA;
    const padBottom = padY;
    expect(resolved).toEqual({
      x: 10 - padX,
      y: 10 - padTop - CUBE_INSTANCE_HEADER_HEIGHT,
      w: 10 + padX * 2,
      h: 10 + padTop + padBottom + CUBE_INSTANCE_HEADER_HEIGHT,
    });
  });

  test('resolveNewInstanceBounds adds managed group visual titlebar clearance', () => {
    const marker = {
      pos: [3840, 3380],
      size: [270, 130],
      getBounding: () => [3840, 3350, 270, 160],
    };

    const resolved = resolveNewInstanceBounds({ nodes: [], markers: [marker] });

    expect(resolved).toEqual({
      x: 3830,
      y: 3290,
      w: 290,
      h: 230,
      padding: {
        x: CUBE_INSTANCE_PADDING.x,
        y: CUBE_INSTANCE_PADDING.y,
        top_extra: CUBE_INSTANCE_TOP_EXTRA,
      },
      header: {
        height: CUBE_INSTANCE_HEADER_HEIGHT,
      },
    });
  });

  test('resolveChromeBoundsFromContent applies standard chrome margins around content', () => {
    const node = { pos: [100, 120], size: [200, 80] };
    const marker = { pos: [50, 140], size: [40, 30] };

    const resolved = resolveChromeBoundsFromContent({
      nodes: [node],
      markers: [marker],
      padding: { x: 2, y: 2, top_extra: 0 },
      header: { height: 32 },
    });

    expect(resolved).toEqual({
      x: 40,
      y: 60,
      w: 270,
      h: 150,
      padding: {
        x: CUBE_INSTANCE_PADDING.x,
        y: CUBE_INSTANCE_PADDING.y,
        top_extra: CUBE_INSTANCE_TOP_EXTRA,
      },
      header: {
        height: CUBE_INSTANCE_HEADER_HEIGHT,
      },
    });
  });

  test('resolveChromeBoundsFromContent keeps live layout bounds when visual bounds are stale', () => {
    const node = {
      pos: [100, 120],
      size: [40, 50],
      getBounding: () => [110, 130, 5, 5],
    };

    const resolved = resolveChromeBoundsFromContent({ nodes: [node], markers: [] });

    expect(resolved).toEqual({
      x: 90,
      y: 60,
      w: 60,
      h: 120,
      padding: {
        x: CUBE_INSTANCE_PADDING.x,
        y: CUBE_INSTANCE_PADDING.y,
        top_extra: CUBE_INSTANCE_TOP_EXTRA,
      },
      header: {
        height: CUBE_INSTANCE_HEADER_HEIGHT,
      },
    });
  });

  test('computeInnerBounds uses padding and header metadata', () => {
    const bounds = {
      x: 0,
      y: 0,
      w: 100,
      h: 80,
      padding: { x: 10, y: 8, top_extra: 6 },
      header: { height: 20 },
    };
    expect(computeInnerBounds(bounds)).toEqual({
      x: 10,
      y: 34,
      w: 80,
      h: 38,
    });
  });

  test('computeInnerBounds remaps legacy default padding to tight defaults', () => {
    const bounds = {
      x: 0,
      y: 0,
      w: 100,
      h: 100,
      padding: { x: 12, y: 12, top_extra: 12 },
      header: { height: 32 },
    };
    expect(computeInnerBounds(bounds)).toEqual({
      x: CUBE_INSTANCE_PADDING.x,
      y: CUBE_INSTANCE_PADDING.y + CUBE_INSTANCE_TOP_EXTRA + CUBE_INSTANCE_HEADER_HEIGHT,
      w: 100 - CUBE_INSTANCE_PADDING.x * 2,
      h:
        100 - (CUBE_INSTANCE_PADDING.y * 2 + CUBE_INSTANCE_TOP_EXTRA + CUBE_INSTANCE_HEADER_HEIGHT),
    });
  });

  test('expandBoundsToIncludeRect expands just enough with padding', () => {
    const bounds = {
      x: 0,
      y: 0,
      w: 100,
      h: 50,
      padding: { x: 10, y: 12, top_extra: 5 },
      header: { height: 24 },
    };
    const rect = { x: 95, y: 40, w: 10, h: 10 };
    const expanded = expandBoundsToIncludeRect(bounds, rect, 2);
    expect(expanded).toEqual({
      x: 0,
      y: 0,
      w: 107,
      h: 52,
      padding: { x: 10, y: 12, top_extra: 5 },
      header: { height: 24 },
    });
  });

  test('writeCanonicalBounds updates typed-array group vectors', () => {
    const metadata = {
      bounds: {
        x: 0,
        y: 0,
        w: 1,
        h: 1,
        padding: { x: 2, y: 2, top_extra: 0 },
        header: { height: 32 },
      },
    };
    const group = {
      pos: new Float32Array([100, 200]),
      size: new Float32Array([300, 400]),
      _bounding: new Float32Array([100, 200, 300, 400]),
      properties: { sugarcubes: metadata },
    };
    const written = writeCanonicalBounds({
      group,
      metadata,
      bounds: { x: 10, y: 20, w: 30, h: 40 },
    });

    expect(written).toEqual({
      x: 10,
      y: 20,
      w: 30,
      h: 40,
      padding: { x: 2, y: 2, top_extra: 0 },
      header: { height: 32 },
    });
    expect(Array.from(group.pos)).toEqual([10, 20]);
    expect(Array.from(group.size)).toEqual([30, 40]);
    expect(Array.from(group._bounding)).toEqual([10, 20, 30, 40]);
    expect(metadata.bounds).toEqual(written);
  });
});

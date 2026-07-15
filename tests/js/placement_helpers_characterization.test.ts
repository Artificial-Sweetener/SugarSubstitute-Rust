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
import { describe, expect, jest, test } from '@jest/globals';
import {
  computePayloadBounds,
  drawGhostRect,
  getPlacementGroupLabel,
  readLayoutFlags,
  readLayoutStyle,
  resolveCollapsedPreviewSize,
  resolvePreviewRect,
  resolvePreviewSize,
} from '../../frontend/comfyui/ui/overlays/PlacementHelpers.js';

function createDrawingContext() {
  const context = {
    font: '11px serif',
    fillStyle: '',
    strokeStyle: '',
    globalAlpha: 1,
    lineWidth: 0,
    measureText: jest.fn(() => ({ width: 100 })),
    save: jest.fn(),
    restore: jest.fn(),
    setLineDash: jest.fn(),
    fillRect: jest.fn(),
    strokeRect: jest.fn(),
    fillText: jest.fn(),
  };
  return context as typeof context & CanvasRenderingContext2D;
}

describe('placement helper characterization', () => {
  test('layout flags and styles prefer direct values before legacy extra values', () => {
    const directFlags = { collapsed: true };
    const extraFlags = { collapsed: false };
    const directStyle = { color: '#123' };
    const extraStyle = { color: '#456' };

    expect(readLayoutFlags({ flags: directFlags, extra: { flags: extraFlags } })).toBe(directFlags);
    expect(readLayoutFlags({ extra: { flags: extraFlags } })).toBe(extraFlags);
    expect(readLayoutFlags({ flags: 'invalid' })).toBeNull();
    expect(readLayoutFlags(null)).toBeNull();
    expect(readLayoutStyle({ style: directStyle, extra: { style: extraStyle } })).toBe(directStyle);
    expect(readLayoutStyle({ extra: { style: extraStyle } })).toBe(extraStyle);
    expect(readLayoutStyle({ style: 1 })).toBeNull();
  });

  test('collapsed preview measurement uses LiteGraph typography and restores canvas font', () => {
    const ctx = createDrawingContext();

    const size = resolveCollapsedPreviewSize({ title: 'Measured title' }, [200, 80], ctx, {
      NODE_TITLE_HEIGHT: 24,
      NODE_COLLAPSED_WIDTH: 80,
      NODE_TEXT_SIZE: 12,
      NODE_FONT: 'Inter',
    });

    expect(size).toEqual([148, 24]);
    expect(ctx.measureText).toHaveBeenCalledWith('Measured title');
    expect(ctx.font).toBe('11px serif');
  });

  test('preview sizing and rectangles preserve expanded and collapsed geometry', () => {
    const ctx = createDrawingContext();
    const liteGraph = { NODE_TITLE_HEIGHT: 30, NODE_COLLAPSED_WIDTH: 80 };
    const expanded = { layout: { flags: { collapsed: false } } };
    const collapsed = {
      layout: { title: 'Cube', extra: { flags: { collapsed: true } } },
    };

    expect(resolvePreviewSize(expanded, [120, 60], ctx, liteGraph)).toEqual([120, 60]);
    expect(resolvePreviewRect(expanded, [10, 20], [120, 60], ctx, liteGraph)).toEqual({
      x: 10,
      y: 20,
      w: 120,
      h: 60,
    });
    expect(resolvePreviewRect(collapsed, [10, 20], [200, 60], ctx, liteGraph)).toEqual({
      x: 10,
      y: -10,
      w: 160,
      h: 30,
    });
  });

  test('payload bounds ignore invalid entries and include collapsed title geometry', () => {
    const ctx = createDrawingContext();
    ctx.measureText.mockReturnValue({ width: 30 } as TextMetrics);
    const entries = [
      { layout: { pos: [10, 20], size: [100, 50] } },
      {
        layout: {
          pos: [-20, 10],
          size: [200, 70],
          title: 'Collapsed',
          flags: { collapsed: true },
        },
      },
      { layout: { pos: ['invalid', 0], size: [10, 10] } },
      {},
    ];

    expect(
      computePayloadBounds(entries, ctx, {
        NODE_TITLE_HEIGHT: 30,
        NODE_COLLAPSED_WIDTH: 80,
      }),
    ).toEqual({ minX: -20, minY: -20, maxX: 110, maxY: 70 });
    expect(computePayloadBounds([{}, { layout: { pos: null, size: null } }])).toBeNull();
  });

  test('placement group labels follow instance, default, title, and fallback precedence', () => {
    const group = { title: 'Group title' };

    expect(getPlacementGroupLabel('Fallback', group, () => ({ instance_alias: 'Instance' }))).toBe(
      'Instance',
    );
    expect(getPlacementGroupLabel('Fallback', group, () => ({ default_alias: 'Default' }))).toBe(
      'Default',
    );
    expect(getPlacementGroupLabel('Fallback', group, () => null)).toBe('Group title');
    expect(getPlacementGroupLabel(' Fallback ', {}, () => null)).toBe('Fallback');
    expect(getPlacementGroupLabel(' ', {}, () => null)).toBeNull();
  });

  test('ghost drawing preserves scaled stroke, fill, alpha, and optional label behavior', () => {
    const ctx = createDrawingContext();

    drawGhostRect(
      ctx,
      { x: 10, y: 20, w: 100, h: 50 },
      { fill: '#123', stroke: '#456', alpha: 0.4 },
      2,
      'Preview',
    );

    expect(ctx.save).toHaveBeenCalledTimes(1);
    expect(ctx.lineWidth).toBe(1);
    expect(ctx.setLineDash).toHaveBeenNthCalledWith(1, [4, 2.5]);
    expect(ctx.fillRect).toHaveBeenCalledWith(10, 20, 100, 50);
    expect(ctx.strokeRect).toHaveBeenCalledWith(10, 20, 100, 50);
    expect(ctx.setLineDash).toHaveBeenNthCalledWith(2, []);
    expect(ctx.font).toBe('10px sans-serif');
    expect(ctx.fillText).toHaveBeenCalledWith('Preview', 13, 27);
    expect(ctx.restore).toHaveBeenCalledTimes(1);
  });

  test('ghost drawing is a no-op without a rectangle', () => {
    const ctx = createDrawingContext();

    drawGhostRect(ctx, null, { fill: '#123', stroke: '#456' }, 1, 'Ignored');

    expect(ctx.save).not.toHaveBeenCalled();
    expect(ctx.fillRect).not.toHaveBeenCalled();
  });
});

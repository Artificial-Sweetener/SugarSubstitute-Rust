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
/** Project reference dimension actions into the universal floating menu's value model. */
import type { ContextMenuEntry } from '../../menus/ContextMenu.js';
import {
  LANDSCAPE_RATIOS,
  PORTRAIT_RATIOS,
  ratioDimension,
  canonicalDimensions,
} from '../domain/DimensionPolicy.js';
import type { DimensionSide } from '../domain/DimensionPolicy.js';
import { GLOBAL_ASSOCIATION } from '../domain/UserPreset.js';
import type { PresetAssociation } from '../domain/UserPreset.js';
import type { UserPresetLibrary } from '../application/UserPresetLibrary.js';
import type { DimensionBinding } from '../application/CanvasDimensionFields.js';

/** Build side-aware dimensions, ratio orientations and matching persisted scopes without IO. */
export function dimensionMenuEntries(
  binding: DimensionBinding,
  side: DimensionSide | null,
  library: UserPresetLibrary,
  families: readonly PresetAssociation[],
  run: (operation: Promise<void>) => void,
  canvasEntries: readonly ContextMenuEntry[] = [],
): ContextMenuEntry[] {
  if (binding.width.disabled() || binding.height.disabled()) return [];
  const entries: ContextMenuEntry[] = [];
  for (const anchor of side ? [side] : (['width', 'height'] as const))
    entries.push({
      label: `Set ratio by ${anchor === 'width' ? 'Width' : 'Height'}`,
      children: [
        { label: 'Landscape', ratios: LANDSCAPE_RATIOS },
        { label: 'Portrait', ratios: PORTRAIT_RATIOS },
      ].map(({ label, ratios }) => ({
        label,
        children: ratios.map(([width, height]) => ({
          label: `${width}:${height}`,
          select: () => {
            const value = binding[anchor].read();
            const target =
              typeof value === 'number' ? ratioDimension(value, anchor, width, height) : null;
            if (target !== null) binding[anchor === 'width' ? 'height' : 'width'].write(target);
          },
        })),
      })),
    });
  entries.push({
    label: 'Swap width & height',
    select: () => {
      const width = binding.width.read(),
        height = binding.height.read();
      if (typeof width === 'number' && typeof height === 'number') {
        binding.width.write(height);
        binding.height.write(width);
      }
    },
  });
  entries.push(...canvasEntries);
  const scopes = [...families, GLOBAL_ASSOCIATION];
  if (scopes.some((scope) => library.dimensions(scope).length))
    entries.push({
      label: 'Set dimensions',
      children: [false, true].map((landscape) => ({
        label: landscape ? 'Landscape' : 'Portrait',
        children: scopes
          .map((scope) => ({ scope, presets: library.dimensions(scope) }))
          .filter(({ presets }) => presets.length)
          .flatMap(({ scope, presets }, index) => [
            ...(index > 0 ? [{ label: '', separator: true }] : []),
            {
              label: scope.scope === 'global' ? 'Global' : `For ${scope.label}`,
              section: true,
            },
            ...presets.map((preset) => {
              const width = landscape ? preset.payload.long_edge : preset.payload.short_edge;
              const height = landscape ? preset.payload.short_edge : preset.payload.long_edge;
              const dimensions = `${width} x ${height}`;
              const label = [
                dimensions,
                `${preset.payload.short_edge} x ${preset.payload.long_edge}`,
              ].includes(preset.label.trim())
                ? dimensions
                : `${preset.label} ${dimensions}`;
              return {
                label,
                select: () => {
                  binding.width.write(width);
                  binding.height.write(height);
                },
              };
            }),
          ]),
      })),
    });
  const width = binding.width.read(),
    height = binding.height.read();
  if (typeof width === 'number' && typeof height === 'number' && canonicalDimensions(width, height))
    entries.push(
      { label: '', separator: true },
      {
        label: 'Save current dimensions',
        children: [
          {
            label: 'Save globally',
            select: () => run(library.saveDimensions(width, height, GLOBAL_ASSOCIATION)),
          },
          ...families.slice(0, 1).map((scope) => ({
            label: `Save for ${scope.label}`,
            select: () => run(library.saveDimensions(width, height, scope)),
          })),
        ],
      },
    );
  return entries;
}

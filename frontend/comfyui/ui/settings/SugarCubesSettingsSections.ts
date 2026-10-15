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
/** Define SugarCubes settings organization independently of native and Fluent control rendering. */
export const sugarCubesSettingsSections = {
  appearance: { label: 'Appearance', priority: 1000 },
  stack: { label: 'Cube stack', priority: 900 },
  canvas: { label: 'Image canvas', priority: 800 },
  output: { label: 'Output files', priority: 700 },
  models: { label: 'Model library', priority: 600 },
  metadata: { label: 'Model metadata', priority: 500 },
  previews: { label: 'Model previews', priority: 400 },
  updates: { label: 'Model updates', priority: 300 },
  connections: { label: 'Workflow connections', priority: 200 },
  authoring: { label: 'Authoring', priority: 100 },
  packs: { label: 'Cube library', priority: 0 },
} as const;

/** Project shared section descriptors into Comfy-native settings tokens without changing value IDs. */
export function sugarCubesSettingsSection(
  section: keyof typeof sugarCubesSettingsSections,
  key: string,
): { category: string[]; sortOrder: number } {
  const descriptor = sugarCubesSettingsSections[section];
  return { category: ['SugarCubes', descriptor.label, key], sortOrder: descriptor.priority };
}

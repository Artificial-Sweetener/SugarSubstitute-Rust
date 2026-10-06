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
/** Compose node presets, dimension contributions and advanced visibility in reference order. */
import type { ContextMenuEntry } from '../../menus/ContextMenu.js';
import { inferDimensionPairs } from '../domain/DimensionPolicy.js';
import { GLOBAL_ASSOCIATION } from '../domain/UserPreset.js';
import type { PresetAssociation } from '../domain/UserPreset.js';
import { captureEditorInputs, applyEditorInputs } from '../application/EditorField.js';
import type { UserPresetLibrary } from '../application/UserPresetLibrary.js';
import type { NativeEditorCard } from '../infrastructure/NativeEditorCardAdapter.js';
import { dimensionMenuEntries } from './DimensionMenu.js';
import type { SaveNodePresetDialog } from './SaveNodePresetDialog.js';
import type { ModelContextContribution } from '../../model_library/ModelContextProjection.js';

/** Consume prepared libraries and explicit command collaborators instead of owning persistence. */
export function nodeActionEntries(
  card: NativeEditorCard,
  library: UserPresetLibrary,
  families: readonly PresetAssociation[],
  dialog: SaveNodePresetDialog,
  run: (operation: Promise<void>) => void,
  skipped: (names: readonly string[]) => void,
  modelContributions: readonly ModelContextContribution[] = [],
): ContextMenuEntry[] {
  const scopes = [...families, GLOBAL_ASSOCIATION];
  const presets = library.nodePresets(card.nodeType, scopes);
  const entries: ContextMenuEntry[] = [];
  if (presets.length) {
    const seen = new Set<string>();
    const populated = scopes.flatMap((scope) => {
      const selected = presets.filter(
        (preset) =>
          !seen.has(preset.id) &&
          preset.associations.some(
            (association) =>
              association.scope === scope.scope &&
              association.provider === scope.provider &&
              association.key === scope.key,
          ),
      );
      for (const preset of selected) seen.add(preset.id);
      return selected.length ? [{ scope, presets: selected }] : [];
    });
    entries.push({
      label: 'Apply preset',
      children: populated.flatMap(({ scope, presets }, index) => [
        ...(index > 0 ? [{ label: '', separator: true }] : []),
        ...(populated.length > 1 ? [{ label: scope.label, section: true }] : []),
        ...presets.map((preset) => ({
          label: preset.label,
          attributes: {
            title: `${card.nodeType} - ${Object.keys(preset.payload.inputs).length} inputs`,
          },
          select: () => {
            const names = applyEditorInputs(card.fields, preset.payload.inputs);
            if (names.length) skipped(names);
          },
        })),
      ]),
    });
  }
  const inputs = captureEditorInputs(card.fields);
  if (Object.keys(inputs).length) {
    if (entries.length) entries.push({ label: '', separator: true });
    entries.push({
      label: `Save current ${card.title} as preset...`,
      select: () =>
        run(
          (async () => {
            const result = await dialog.open(`Save ${card.title} preset`, [
              GLOBAL_ASSOCIATION,
              ...families,
            ]);
            if (result)
              await library.saveNode(result.name, card.nodeType, inputs, result.association);
          })(),
        ),
    });
  }
  const fieldNames = card.fields.map((field) => field.name);
  const contributions = modelContributions.map((contribution) => ({
    order: fieldNames.indexOf(contribution.fieldName),
    entries: contribution.entries,
  }));
  for (const pair of inferDimensionPairs(fieldNames)) {
    const width = card.fields.find((field) => field.name === pair.width),
      height = card.fields.find((field) => field.name === pair.height);
    if (!width || !height || width.kind !== 'number' || height.kind !== 'number') continue;
    const dimensions = dimensionMenuEntries({ width, height }, null, library, families, run);
    contributions.push({
      order: Math.min(fieldNames.indexOf(pair.width), fieldNames.indexOf(pair.height)),
      entries: dimensions,
    });
  }
  for (const contribution of contributions.sort((left, right) => left.order - right.order)) {
    if (!contribution.entries.length) continue;
    if (entries.length) entries.push({ label: '', separator: true });
    entries.push(...contribution.entries);
  }
  if (card.advanced) {
    if (entries.length) entries.push({ label: '', separator: true });
    entries.push({
      label: 'Show advanced inputs',
      checked: card.advanced.shown(),
      selection: 'multiple',
      select: () => card.advanced?.setShown(!card.advanced.shown()),
    });
  }
  return entries;
}

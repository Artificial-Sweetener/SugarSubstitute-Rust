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
/** Supply complete server-owned output settings at the HTTP test boundary. */
import type {
  OutputSavingPreferences,
  OutputSavingSnapshot,
} from '../../../frontend/comfyui/ui/output_saving/OutputSavingPreferences.js';

export const savingPreferences: OutputSavingPreferences = {
  format: 'sugarcubes.output_saving',
  schema_version: 1,
  organization: { output_root: null, path_pattern: '{date}/{run}_{cube#}_{workflow}_{source}' },
  jpeg: { enabled: false, sizing_mode: 'quality', quality: 92, target_size_kib: 800 },
  transfer: { preferred_format: 'companion_jpeg' },
  persistence_mode: 'all',
  manage_workflow: false,
  save_previews: true,
};

/** Retain literal server text to prove dynamic output paths never become markup. */
export function savingSnapshot(preferences = savingPreferences): OutputSavingSnapshot {
  return {
    preferences,
    examplePath: 'E:/Outputs/<script>literal</script>.png',
    tokens: ['{run}', '{cube#}', '{source}'],
  };
}

/** Model only the actual JSON transport envelope rather than private UI state. */
export function savingResponse(preferences = savingPreferences) {
  const snapshot = savingSnapshot(preferences);
  return {
    response: { ok: true },
    data: {
      success: true,
      data: { preferences, example_path: snapshot.examplePath, tokens: snapshot.tokens },
    },
  };
}

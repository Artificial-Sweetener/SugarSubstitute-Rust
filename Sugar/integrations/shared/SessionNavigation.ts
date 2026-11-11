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
/** Validate independent scene, batch-grid and source navigation within its occurrence. */
import type { SessionEntry } from '../contracts/SessionProtocol.generated.js';

/** Keep navigation intent separate from the exact artifact retained for final inspection. */
export function sessionNavigation(entry: SessionEntry): boolean {
  const navigation = entry.view.navigation;
  if (navigation.sceneOverview && navigation.sceneId !== null) return false;
  if (
    navigation.sourceId !== null &&
    !entry.sources.some((source) => source.id === navigation.sourceId)
  )
    return false;
  if (
    navigation.sceneId !== null &&
    !entry.submissions.some((item) => item.scene?.sceneId === navigation.sceneId) &&
    !entry.outputs.some((item) => item.scene?.sceneId === navigation.sceneId)
  )
    return false;
  if (navigation.batchOverview) return true;
  const outputs = entry.outputs.filter(
    (item) =>
      (navigation.sourceId === null || item.sourceId === navigation.sourceId) &&
      (navigation.sceneOverview || (item.scene?.sceneId ?? null) === navigation.sceneId),
  );
  return outputs.length === 0
    ? navigation.batchIndex === 0
    : outputs.some((item) => item.batchIndex === navigation.batchIndex);
}

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
/** Decode bounded JSON while rejecting duplicate object members before semantic validation. */
/**
 * Carry a desktop checkpoint between its versions. Version 2 is version 1 with each entry's view
 * naming where its workflow's masonry is scrolled; SugarCubes holds every checkpoint as version 2.
 */
import type {
  SessionCheckpoint as FirstCheckpoint,
  SessionView as FirstView,
} from '../contracts/SessionProtocol.generated.js';
import type {
  SessionCheckpoint,
  SessionView,
} from '../contracts/SessionCheckpointV2Protocol.generated.js';

/** Read a version-1 checkpoint as version 2, naming no scroll. */
export function upgradeDesktopCheckpoint(value: FirstCheckpoint): SessionCheckpoint {
  return {
    ...value,
    version: 2,
    entries: value.entries.map((entry) => ({ ...entry, view: { ...entry.view, scroll: null } })),
  };
}

/**
 * Write a checkpoint for its host: as version 2 when the host takes it and an entry names a
 * scroll, as version 1 otherwise, so a host that reads only version 1 always reads it.
 */
export function encodeDesktopCheckpoint(
  value: SessionCheckpoint,
  hostTakesScroll: boolean,
): string {
  if (hostTakesScroll && value.entries.some((entry) => entry.view.scroll !== null))
    return JSON.stringify(value);
  const first: FirstCheckpoint = {
    ...value,
    version: 1,
    entries: value.entries.map((entry) => ({ ...entry, view: firstView(entry.view) })),
  };
  return JSON.stringify(first);
}

/** A version-2 view without its scroll, member by member as version 1 names them. */
function firstView(view: SessionView): FirstView {
  return {
    canvasEnabled: view.canvasEnabled,
    masonryEnabled: view.masonryEnabled,
    editorWidth: view.editorWidth,
    route: view.route,
    follow: view.follow,
    mode: view.mode,
    linked: view.linked,
    selection: view.selection,
    comparison: view.comparison,
    cameras: view.cameras,
    navigation: view.navigation,
    sourcePreview: view.sourcePreview,
    sharedInspection: view.sharedInspection,
  };
}

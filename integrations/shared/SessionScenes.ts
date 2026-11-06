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
/** Validate coherent scene-group facts across accepted pending prompts and final receipts. */
import type { SessionEntry } from '../contracts/SessionProtocol.generated.js';

/** Keep one scene identity per group/order and one immutable scene description per identity. */
export function sessionScenes(entry: SessionEntry): boolean {
  const slots = new Map<string, string>();
  const facts = new Map<string, string>();
  const counts = new Map<string, number>();
  for (const item of [...entry.submissions, ...entry.outputs]) {
    const scene = item.scene;
    if (scene === null) continue;
    const slot = `${scene.groupId}/${String(scene.order)}`;
    const identity = `${scene.groupId}/${scene.sceneId}`;
    const description = JSON.stringify([scene.order, scene.count, scene.title]);
    if (
      (slots.has(slot) && slots.get(slot) !== scene.sceneId) ||
      (facts.has(identity) && facts.get(identity) !== description) ||
      (counts.has(scene.groupId) && counts.get(scene.groupId) !== scene.count)
    )
      return false;
    slots.set(slot, scene.sceneId);
    facts.set(identity, description);
    counts.set(scene.groupId, scene.count);
  }
  return true;
}

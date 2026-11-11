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
/** Supply the canonical backend envelope used by browser graph contract proofs. */
export function graphResponse() {
  return {
    schema_version: 1,
    workflow_semantic_hash: 'a'.repeat(64),
    instances: ['first', 'second'].map((instanceId, index) => ({
      instance_id: instanceId,
      node_id: String(index + 1),
      definition_id: `definition-${instanceId}`,
      cube_id: `local/personal/${instanceId}`,
      cube_version: '1.0.0',
      instance_alias: instanceId,
      execution_mode: 0,
    })),
    segments: [
      { instance_ids: ['first', 'second'], reorderable: true, boundary_node_ids: [] as string[] },
    ],
    workflow: {
      nodes: ['first', 'second'].map((instanceId, index) => ({
        id: index + 1,
        pos: [index * 244, 0] as unknown[],
        size: [220, 160],
        properties: { sugarcubes_cube: { instance_id: instanceId } },
      })),
    },
  };
}

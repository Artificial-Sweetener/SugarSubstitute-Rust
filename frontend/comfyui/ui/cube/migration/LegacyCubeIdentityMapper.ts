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
/** Map persisted group-era Cube identity into native Cube-node ownership. */

import { isRecord } from '../../types/common.js';
import type { UnknownRecord } from '../../types/common.js';
import type { CubeIdentity } from '../CubePlacementService.js';
import type { LegacyCubePlan } from './LegacyCubeWorkflowExtractor.js';

/** Preserve a legacy Cube's identity and presentation metadata during migration. */
export function mapLegacyCubeIdentity(plan: LegacyCubePlan): CubeIdentity {
  const metadata = cloneRecord(plan.metadata);
  const definition = readRecord(metadata.definition);
  const instance = readRecord(metadata.instance);
  const defaultAlias =
    readString(definition.default_alias) || readString(metadata.default_alias) || plan.title;
  return {
    cubeId: plan.cubeId || readString(definition.cube_id) || readString(metadata.cube_id),
    cubeVersion:
      plan.cubeVersion || readString(definition.cube_version) || readString(metadata.cube_version),
    instanceId: plan.key,
    defaultAlias,
    instanceAlias:
      readString(instance.instance_alias) ||
      readString(metadata.instance_alias) ||
      plan.title ||
      defaultAlias,
    metadata,
  };
}

/** Clone JSON-safe metadata before assigning it to a native Cube node. */
function cloneRecord(value: UnknownRecord): UnknownRecord {
  const parsed: unknown = JSON.parse(JSON.stringify(value));
  return isRecord(parsed) ? parsed : {};
}

/** Read an optional metadata section without allowing dynamic values to leak. */
function readRecord(value: unknown): UnknownRecord {
  return isRecord(value) ? value : {};
}

/** Read one persisted non-empty string. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

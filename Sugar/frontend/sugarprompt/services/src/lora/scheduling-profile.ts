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
/** The existing profile owner receives compatible prepared scheduling authority rather than inspecting host object graphs. */
import { validateFeatureContext } from '../features/profile.js';
import type { FeatureContext } from '../features/contracts.js';
import type { PreparedLoraSchedulingContext } from './scheduled-contracts.js';
import { assertPreparedLoraSchedulingContext } from './scheduled-context.js';

/** Workflow eligibility requires both proven field support and a context bound to the current catalog; library permissions stay profile-owned. */
export function schedulingFeatureContext(
  base: FeatureContext,
  context: PreparedLoraSchedulingContext | null,
  catalogRevision: string | null,
): FeatureContext {
  validateFeatureContext(base);
  if (context) assertPreparedLoraSchedulingContext(context);
  if (base.mode === 'library') return base;
  return Object.freeze({
    ...base,
    loraScheduling:
      base.loraScheduling === true &&
      context?.mode === 'workflow' &&
      context.supported &&
      context.catalogRevision === catalogRevision,
  });
}

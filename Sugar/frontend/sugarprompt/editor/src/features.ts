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
/** CodeMirror consumes one service-resolved immutable feature profile through a reconfigurable facet. */
import { Facet } from '@codemirror/state';
import {
  defaultPreferences,
  resolveFeatureProfile,
  type FeatureProfile,
} from '../../services/src/index.js';

/** Standalone native editors default to library permissions until a host supplies a profile. */
const defaultProfile = resolveFeatureProfile(defaultPreferences, {
  mode: 'library',
});

/** The component owns profile input; state fields observe it without recalculating decisions. */
export const featureProfile = Facet.define<FeatureProfile, FeatureProfile>({
  /** One component input replaces the default; multiple owners are an invariant violation. */
  combine(values) {
    if (values.length > 1) throw new Error('Multiple feature profile owners');
    return values[0] ?? defaultProfile;
  },
});

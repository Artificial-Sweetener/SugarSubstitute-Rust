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
/** Carry explicit dialog state between discovery coordination and the isolated Vue view. */

import type { ModelDiscoveryPlan } from './ModelDiscovery.js';

export interface ModelDiscoveryViewState {
  readonly artifactKind: string;
  readonly plan: ModelDiscoveryPlan | null;
  readonly loading: boolean;
  readonly downloading: boolean;
  readonly selected: string;
  readonly providers: Readonly<Record<string, string>>;
  readonly status: string;
  readonly credential: boolean;
  readonly credentialBusy: boolean;
  readonly credentialError: string;
}
export interface ModelDiscoveryViewActions {
  select(identity: string): void;
  provider(identity: string, providerId: string): void;
  cancelCredential(): void;
  publicOnly(): void;
  saveCredential(key: string): void;
}
export interface MountedModelDiscoveryView {
  update(state: ModelDiscoveryViewState): void;
  dispose(): void;
}

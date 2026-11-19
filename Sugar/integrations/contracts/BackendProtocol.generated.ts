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
/** Generated from backend.schema.json by scripts/integrations/generate-backend-contract.ts. */
/** Describe the schema-defined BackendStatus wire value. */
export type BackendStatus = 'running' | 'stopped' | 'retry-ready' | 'retrying';
/** Describe the schema-defined BackendStateMessage wire value. */
export type BackendStateMessage = {
  readonly protocol: 'sugarcubes.backend';
  readonly version: 1;
  readonly session: string;
  readonly type: 'state';
  readonly revision: number;
  readonly status: BackendStatus;
};
/** Describe the schema-defined BackendWatchMessage wire value. */
export type BackendWatchMessage = {
  readonly protocol: 'sugarcubes.backend';
  readonly version: 1;
  readonly session: string;
  readonly type: 'watch';
};
/** Describe the schema-defined BackendRetryMessage wire value. */
export type BackendRetryMessage = {
  readonly protocol: 'sugarcubes.backend';
  readonly version: 1;
  readonly session: string;
  readonly type: 'retry';
  readonly revision: number;
};
/** Describe the schema-defined BackendClientMessage wire value. */
export type BackendClientMessage = BackendWatchMessage | BackendRetryMessage;
/** Describe every version-one backend message. */
export type BackendMessage = BackendStateMessage | BackendWatchMessage | BackendRetryMessage;

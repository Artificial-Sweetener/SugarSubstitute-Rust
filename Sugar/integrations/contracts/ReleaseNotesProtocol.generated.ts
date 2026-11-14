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
/** Generated from release-notes.schema.json by scripts/integrations/generate-release-notes-contract.ts. */
/** Describe the schema-defined ReleaseNotesFailure wire value. */
export type ReleaseNotesFailure = 'unavailable' | 'not-published';
/** Describe the schema-defined ReleaseNotesRelease wire value. */
export type ReleaseNotesRelease = { readonly version: string; readonly notes: string };
/** Describe the schema-defined ReleaseNotesStateMessage wire value. */
export type ReleaseNotesStateMessage = {
  readonly protocol: 'sugarcubes.release-notes';
  readonly version: 1;
  readonly session: string;
  readonly type: 'state';
  readonly revision: number;
  readonly product: string;
  readonly currentVersion: string;
};
/** Describe the schema-defined ReleaseNotesOpenMessage wire value. */
export type ReleaseNotesOpenMessage = {
  readonly protocol: 'sugarcubes.release-notes';
  readonly version: 1;
  readonly session: string;
  readonly type: 'open';
};
/** Describe the schema-defined ReleaseNotesNotesMessage wire value. */
export type ReleaseNotesNotesMessage = {
  readonly protocol: 'sugarcubes.release-notes';
  readonly version: 1;
  readonly session: string;
  readonly type: 'notes';
  readonly request: string;
  readonly releases: readonly ReleaseNotesRelease[];
  readonly omitted: number;
};
/** Describe the schema-defined ReleaseNotesFailedMessage wire value. */
export type ReleaseNotesFailedMessage = {
  readonly protocol: 'sugarcubes.release-notes';
  readonly version: 1;
  readonly session: string;
  readonly type: 'failed';
  readonly request: string;
  readonly reason: ReleaseNotesFailure;
};
/** Describe the schema-defined ReleaseNotesWatchMessage wire value. */
export type ReleaseNotesWatchMessage = {
  readonly protocol: 'sugarcubes.release-notes';
  readonly version: 1;
  readonly session: string;
  readonly type: 'watch';
};
/** Describe the schema-defined ReleaseNotesLoadMessage wire value. */
export type ReleaseNotesLoadMessage = {
  readonly protocol: 'sugarcubes.release-notes';
  readonly version: 1;
  readonly session: string;
  readonly type: 'load';
  readonly request: string;
  readonly since: string;
};
/** Describe the schema-defined ReleaseNotesHostMessage wire value. */
export type ReleaseNotesHostMessage =
  | ReleaseNotesStateMessage
  | ReleaseNotesOpenMessage
  | ReleaseNotesNotesMessage
  | ReleaseNotesFailedMessage;
/** Describe the schema-defined ReleaseNotesClientMessage wire value. */
export type ReleaseNotesClientMessage = ReleaseNotesWatchMessage | ReleaseNotesLoadMessage;
/** Describe every version-one release notes message. */
export type ReleaseNotesMessage =
  | ReleaseNotesStateMessage
  | ReleaseNotesOpenMessage
  | ReleaseNotesNotesMessage
  | ReleaseNotesFailedMessage
  | ReleaseNotesWatchMessage
  | ReleaseNotesLoadMessage;

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
/** Generated from updates.schema.json by scripts/integrations/generate-updates-contract.ts. */
/** Describe the schema-defined UpdatesChannel wire value. */
export type UpdatesChannel = 'stable' | 'beta';
/** Describe the schema-defined UpdatesFailure wire value. */
export type UpdatesFailure = 'check' | 'update';
/** Describe the schema-defined UpdatesPhase wire value. */
export type UpdatesPhase = 'check' | 'download' | 'verify' | 'install' | 'start';
/** Describe the schema-defined UpdatesScheduledRestart wire value. */
export type UpdatesScheduledRestart = 'none' | 'after-workflow';
/** Describe the schema-defined UpdatesRestartTiming wire value. */
export type UpdatesRestartTiming = 'now' | 'after-workflow';
/** Describe the schema-defined UpdatesRejection wire value. */
export type UpdatesRejection = 'busy' | 'not-ready' | 'unavailable';
/** Describe the schema-defined UpdatesCheck wire value. */
export type UpdatesCheck = { readonly completedAt: number };
/** Describe the schema-defined UpdatesStatus wire value. */
export type UpdatesStatus =
  | { readonly state: 'up-to-date' }
  | { readonly state: 'checking' }
  | { readonly state: 'available'; readonly version: string }
  | { readonly state: 'ready'; readonly version: string }
  | { readonly state: 'failed'; readonly during: UpdatesFailure };
/** Describe the schema-defined UpdatesCause wire value. */
export type UpdatesCause = { readonly message: string };
/** Describe the schema-defined UpdatesReport wire value. */
export type UpdatesReport = {
  readonly version: string;
  readonly phase: UpdatesPhase;
  readonly causes: readonly UpdatesCause[];
};
/** Describe the schema-defined UpdatesState wire value. */
export type UpdatesState = {
  readonly currentVersion: string;
  readonly channel: UpdatesChannel;
  readonly lastSuccessfulCheck?: UpdatesCheck;
  readonly status: UpdatesStatus;
  readonly report?: UpdatesReport;
  readonly previousVersionRunning: boolean;
  readonly workflowRunning: boolean;
  readonly scheduledRestart: UpdatesScheduledRestart;
};
/** Describe the schema-defined UpdatesStateMessage wire value. */
export type UpdatesStateMessage = {
  readonly protocol: 'sugarcubes.updates';
  readonly version: 1;
  readonly session: string;
  readonly type: 'state';
  readonly revision: number;
  readonly update: UpdatesState;
};
/** Describe the schema-defined UpdatesRejectedMessage wire value. */
export type UpdatesRejectedMessage = {
  readonly protocol: 'sugarcubes.updates';
  readonly version: 1;
  readonly session: string;
  readonly type: 'rejected';
  readonly request: string;
  readonly reason: UpdatesRejection;
};
/** Describe the schema-defined UpdatesAcceptedMessage wire value. */
export type UpdatesAcceptedMessage = {
  readonly protocol: 'sugarcubes.updates';
  readonly version: 1;
  readonly session: string;
  readonly type: 'accepted';
  readonly request: string;
};
/** Describe the schema-defined UpdatesWatchMessage wire value. */
export type UpdatesWatchMessage = {
  readonly protocol: 'sugarcubes.updates';
  readonly version: 1;
  readonly session: string;
  readonly type: 'watch';
};
/** Describe the schema-defined UpdatesCheckMessage wire value. */
export type UpdatesCheckMessage = {
  readonly protocol: 'sugarcubes.updates';
  readonly version: 1;
  readonly session: string;
  readonly type: 'check';
  readonly request: string;
};
/** Describe the schema-defined UpdatesRestartMessage wire value. */
export type UpdatesRestartMessage = {
  readonly protocol: 'sugarcubes.updates';
  readonly version: 1;
  readonly session: string;
  readonly type: 'restart';
  readonly request: string;
  readonly when: UpdatesRestartTiming;
};
/** Describe the schema-defined UpdatesHostMessage wire value. */
export type UpdatesHostMessage =
  | UpdatesStateMessage
  | UpdatesAcceptedMessage
  | UpdatesRejectedMessage;
/** Describe the schema-defined UpdatesClientMessage wire value. */
export type UpdatesClientMessage =
  | UpdatesWatchMessage
  | UpdatesCheckMessage
  | UpdatesRestartMessage;
/** Describe every version-one update status message. */
export type UpdatesMessage =
  | UpdatesStateMessage
  | UpdatesAcceptedMessage
  | UpdatesRejectedMessage
  | UpdatesWatchMessage
  | UpdatesCheckMessage
  | UpdatesRestartMessage;

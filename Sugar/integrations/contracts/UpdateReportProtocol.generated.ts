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
/** Generated from update-report.schema.json by scripts/integrations/generate-update-report-contract.ts. */
/** Describe the schema-defined UpdateReportStateMessage wire value. */
export type UpdateReportStateMessage = {
  readonly protocol: 'sugarcubes.update-report';
  readonly version: 1;
  readonly session: string;
  readonly type: 'state';
  readonly revision: number;
  readonly available: boolean;
};
/** Describe the schema-defined UpdateReportWatchMessage wire value. */
export type UpdateReportWatchMessage = {
  readonly protocol: 'sugarcubes.update-report';
  readonly version: 1;
  readonly session: string;
  readonly type: 'watch';
};
/** Describe the schema-defined UpdateReportOpenMessage wire value. */
export type UpdateReportOpenMessage = {
  readonly protocol: 'sugarcubes.update-report';
  readonly version: 1;
  readonly session: string;
  readonly type: 'open';
  readonly revision: number;
};
/** Describe the schema-defined UpdateReportClientMessage wire value. */
export type UpdateReportClientMessage = UpdateReportWatchMessage | UpdateReportOpenMessage;
/** Describe every version-one update report message. */
export type UpdateReportMessage =
  | UpdateReportStateMessage
  | UpdateReportWatchMessage
  | UpdateReportOpenMessage;

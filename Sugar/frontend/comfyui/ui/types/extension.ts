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
/** Define the host extension lifecycle and application capabilities at the ComfyUI boundary. */
import type { ComfyApp, ComfyExtension, ComfyNodeDefinition } from '/scripts/app.js';
import type { SettingsManager } from '../settings/HostSettingsController.js';
import type { UnknownRecord } from './common.js';
import type { ComfyApplication } from './graph.js';

/** Describe the application services used by SugarCubes' composition root. */
export interface SugarCubesHostApp extends ComfyApplication {
  ui?: { settings?: SettingsManager };
  clean?(): void;
  registerExtension?(extension: UnknownRecord): void;
}

/** Preserve the public host lifecycle surface independently of runtime composition. */
export interface SugarCubesExtension extends ComfyExtension, UnknownRecord {
  addCustomNodeDefs(definitions: Record<string, ComfyNodeDefinition>): Promise<void>;
  beforeRegisterVueAppNodeDefs(definitions: ComfyNodeDefinition[]): void;
  registerCustomNodes(): void;
  refreshComboInNodes(): void;
  setup(): Promise<void>;
  beforeConfigureGraph(graphData?: unknown): void;
  afterConfigureGraph(missingNodeTypes: unknown[], comfyApp: ComfyApp): void;
  getCanvasMenuItems(canvas: unknown): unknown[];
  getNodeMenuItems(node: unknown): unknown[];
}

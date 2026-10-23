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
/** Generated from panel.schema.json by scripts/integrations/generate-panel-contract.ts. */
/** Describe the schema-defined PanelPresentation wire value. */
export type PanelPresentation = {
  readonly pluginMode: boolean;
  readonly visualStyle: 'comfy' | 'fluent' | 'spectrum' | 'photopro';
  readonly photoProPalette: 'black' | 'dark-gray' | 'medium-gray' | 'light-gray';
  readonly accentColor: string | null;
  readonly resizeCornerSize: number;
};
/** Describe the schema-defined PanelReady wire value. */
export type PanelReady = {
  readonly protocol: 'sugarcubes.panel';
  readonly version: 3;
  readonly session: string;
  readonly type: 'ready';
};
/** Describe the schema-defined PanelConfigure wire value. */
export type PanelConfigure = {
  readonly protocol: 'sugarcubes.panel';
  readonly version: 3;
  readonly session: string;
  readonly type: 'configure';
  readonly presentation: PanelPresentation;
};
/** Describe the schema-defined PanelApplied wire value. */
export type PanelApplied = {
  readonly protocol: 'sugarcubes.panel';
  readonly version: 3;
  readonly session: string;
  readonly type: 'applied';
};
/** Describe the schema-defined PanelPing wire value. */
export type PanelPing = {
  readonly protocol: 'sugarcubes.panel';
  readonly version: 3;
  readonly session: string;
  readonly type: 'ping';
};
/** Describe the schema-defined PanelPong wire value. */
export type PanelPong = {
  readonly protocol: 'sugarcubes.panel';
  readonly version: 3;
  readonly session: string;
  readonly type: 'pong';
  readonly connected: boolean;
};
/** Describe the schema-defined PanelRejected wire value. */
export type PanelRejected = {
  readonly protocol: 'sugarcubes.panel';
  readonly version: 3;
  readonly session: string;
  readonly type: 'rejected';
  readonly reason: 'unsupported-presentation' | 'presentation-failed';
};
/** Describe every version-three embedded panel message. */
export type PanelMessage =
  | PanelReady
  | PanelConfigure
  | PanelApplied
  | PanelPing
  | PanelPong
  | PanelRejected;

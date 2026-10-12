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
/** Describe the browser workspace allocation independently of host DOM and workflow documents. */

export interface CanvasWorkspacePreferences {
  readonly visible: boolean;
  /** Use zero for the automatic forty-percent allocation; store accepted user widths in CSS pixels. */
  readonly width: number;
}

export interface CanvasWorkspaceRectangle {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export interface CanvasWorkspaceAllocation {
  readonly mode: 'docked' | 'drawer';
  readonly visible: boolean;
  readonly minimumWidth: number;
  readonly maximumWidth: number;
  readonly canvas: CanvasWorkspaceRectangle;
  readonly content: CanvasWorkspaceRectangle;
}

/** Keep useful docked content and require explicit drawer activation below the workspace breakpoint. */
export function allocateCanvasWorkspace(
  width: number,
  height: number,
  preferences: CanvasWorkspacePreferences,
  drawerOpen: boolean,
): CanvasWorkspaceAllocation {
  if (![width, height, preferences.width].every((value) => Number.isFinite(value) && value >= 0))
    throw new RangeError('Canvas workspace dimensions are invalid.');
  const mode = width < 960 ? 'drawer' : 'docked';
  const visible =
    preferences.visible && width > 0 && height > 0 && (mode === 'docked' || drawerOpen);
  const maximum = Math.max(0, Math.min(width, mode === 'docked' ? width - 360 : width - 24));
  const minimum = Math.min(320, maximum);
  const requested = preferences.width || width * (mode === 'docked' ? 0.4 : 0.8);
  const paneWidth = visible ? Math.min(width, maximum, Math.max(320, requested)) : 0;
  const inset = mode === 'docked' ? paneWidth : 0;
  return Object.freeze({
    mode,
    visible,
    minimumWidth: minimum,
    maximumWidth: maximum,
    canvas: Object.freeze({ x: 0, y: 0, width: paneWidth, height }),
    content: Object.freeze({ x: inset, y: 0, width: Math.max(0, width - inset), height }),
  });
}

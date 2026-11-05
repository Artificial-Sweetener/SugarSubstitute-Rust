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
/** Own Substitute's Mica wash model: translucent theme fills painted over the native backdrop. */

/** One translucent fill as 8-bit sRGB channels and an 8-bit alpha, matching the desktop painter. */
interface WashColor {
  readonly red: number;
  readonly green: number;
  readonly blue: number;
  readonly alpha: number;
}

/** CSS colors for the two washes; the tab bar and the native canvas stay unwashed Mica. */
export interface DesktopMaterialWash {
  /** Workspace body wash beneath masonry, the cube stack and the Comfy graph, grid included. */
  readonly body: string;
  /** Workflow chrome wash shared by the selected tab and the global override toolbar row. */
  readonly chrome: string;
}

const DARK_BODY: WashColor = { red: 32, green: 32, blue: 32, alpha: 150 };
const LIGHT_BODY: WashColor = { red: 251, green: 251, blue: 251, alpha: 188 };
const DARK_CHROME: WashColor = { red: 44, green: 44, blue: 44, alpha: 150 };
const LIGHT_CHROME: WashColor = { red: 252, green: 252, blue: 252, alpha: 196 };

/** Root custom property carrying the body wash from the frame owner to washed workspace surfaces. */
export const DESKTOP_BODY_WASH_PROPERTY = '--sc-desktop-body-wash';
/** Root custom property carrying the chrome wash to the selected tab and the override toolbar. */
export const DESKTOP_CHROME_WASH_PROPERTY = '--sc-desktop-chrome-wash';

/** Format one wash with four-decimal alpha so CSS composites the painter's exact 8-bit alpha. */
function css({ red, green, blue, alpha }: WashColor): string {
  return `rgba(${String(red)}, ${String(green)}, ${String(blue)}, ${(alpha / 255).toFixed(4)})`;
}

/** Select the washes for the native window theme that DWM uses for the Mica backdrop. */
export function desktopMaterialWash(darkTheme: boolean): DesktopMaterialWash {
  return Object.freeze({
    body: css(darkTheme ? DARK_BODY : LIGHT_BODY),
    chrome: css(darkTheme ? DARK_CHROME : LIGHT_CHROME),
  });
}

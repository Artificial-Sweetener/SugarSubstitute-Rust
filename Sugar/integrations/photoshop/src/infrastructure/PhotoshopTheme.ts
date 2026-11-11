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
/** Translate Photoshop theme changes into the existing PhotoPro palette preference. */
import type { PanelPresentation } from '../../../contracts/PanelProtocol.generated.js';

/** Describe the native theme API omitted by the pinned UXP declarations. */
declare const document: {
  readonly theme: {
    getCurrent(): unknown;
    readonly onUpdated: {
      addListener(changed: () => void): void;
      removeListener(changed: () => void): void;
    };
  };
};

/** Map Photoshop's interface levels while retaining Black as a manual PhotoPro choice. */
export function readPhotoshopPresentation(): PanelPresentation {
  const theme = document.theme.getCurrent();
  if (theme !== 'darkest' && theme !== 'dark' && theme !== 'light' && theme !== 'lightest')
    throw new Error('Photoshop returned an unsupported interface theme.');
  const palettes = {
    darkest: 'dark-gray',
    dark: 'medium-gray',
    light: 'light-gray',
    lightest: 'light-gray',
  } as const;
  return {
    pluginMode: true,
    visualStyle: 'photopro',
    photoProPalette: palettes[theme],
    accentColor: '#d6d6d6',
    resizeCornerSize: 0,
  };
}

/** Observe native theme changes and release the exact listener on panel destruction. */
export function watchPhotoshopTheme(
  changed: (value: PanelPresentation) => void,
  report: (error: unknown) => void,
): () => void {
  const refresh = (): void => {
    try {
      changed(readPhotoshopPresentation());
    } catch (error: unknown) {
      report(new Error('SugarCubes could not synchronize the Photoshop theme.', { cause: error }));
    }
  };
  document.theme.onUpdated.addListener(refresh);
  return () => {
    document.theme.onUpdated.removeListener(refresh);
  };
}

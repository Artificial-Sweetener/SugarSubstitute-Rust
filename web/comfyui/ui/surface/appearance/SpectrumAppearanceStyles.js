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
/** Present native Comfy fields with Spectrum 2 foundations while retaining host interaction owners. */
import { nativeAppearanceCss } from './NativeAppearanceStyles.js';
import { spectrumTokenDeclarations } from './SpectrumThemeRuntime.js';
/** Apply Adobe's field boundaries and focus treatment to the shared native geometry. */
export function spectrumAppearanceCss(dark) {
    const field = ':is([data-sc-number-spinner], [data-sc-style-combo-frame], [data-sc-style-edit-frame]:not(:has(textarea)), textarea, .sc-model-field)';
    const scope = ':is([data-sc-style-node], [data-sc-style-surface])';
    return (nativeAppearanceCss({
        tokens: spectrumTokenDeclarations(dark),
        canvas: 'var(--spectrum-gray-50)',
        cardFill: 'var(--spectrum-gray-100)',
        cardStroke: 'var(--spectrum-gray-300)',
        controlFill: 'var(--sc-field-fill)',
        controlStroke: 'var(--sc-field-stroke)',
        controlBottomStroke: 'var(--sc-field-bottom-stroke)',
        radius: '12px',
    }, dark) +
        `
    [data-sc-style-node] [data-testid="node-title"] { font-weight:600; }
    ${scope} ${field} {
      border:var(--sc-field-border-width) solid var(--sc-field-stroke);
      border-radius:var(--sc-field-radius);
    }
    ${scope} ${field}:hover {
      --sc-control-stroke:var(--sc-field-hover-stroke); --sc-control-bottom-stroke:var(--sc-field-hover-bottom-stroke);
      border-color:var(--sc-field-hover-stroke); background:var(--sc-field-fill);
    }
    ${scope} ${field}:focus-within {
      --sc-control-stroke:var(--sc-field-focus-stroke); --sc-control-bottom-stroke:var(--sc-field-focus-bottom-stroke);
      border-color:var(--sc-field-focus-stroke); box-shadow:var(--sc-field-focus-shadow);
    }
    ${scope} ${field}:is(:focus-visible,:has(:focus-visible)) {
      outline:var(--sc-field-focus-outline); outline-offset:var(--sc-field-focus-outline-offset);
    }
    [data-sc-style-popup], [data-sc-style-popup] [role="option"] { border-radius:var(--sc-control-radius); }
    [data-sc-boolean-segments] { border-radius:var(--sc-control-radius); }
  `);
}

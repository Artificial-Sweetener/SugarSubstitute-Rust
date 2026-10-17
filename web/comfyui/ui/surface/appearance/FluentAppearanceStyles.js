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
/** Supply Microsoft's presentation palette to the shared native-card stylesheet. */
import { fluentTokenDeclarations } from './FluentThemeRuntime.js';
import { nativeAppearanceCss } from './NativeAppearanceStyles.js';
import { fluentSliderThumbCss } from './FluentSliderThumb.js';
/** Preserve Fluent's established card geometry and palette in either host color scheme. */
export function fluentAppearanceCss(dark) {
    return (fluentSliderThumbCss() +
        nativeAppearanceCss({
            tokens: fluentTokenDeclarations(dark),
            canvas: dark ? '#202020' : '#f3f3f3',
            cardFill: dark ? '#2b2b2b' : '#fbfbfb',
            cardStroke: dark ? 'rgba(0,0,0,.10)' : 'rgba(0,0,0,.06)',
            rowStroke: 'var(--sc-card-stroke)',
            controlFill: dark ? '#373737' : '#ffffff',
            controlStroke: dark ? 'rgba(255,255,255,.07)' : 'rgba(0,0,0,.07)',
            controlBottomStroke: dark ? 'rgba(255,255,255,.45)' : 'rgba(0,0,0,.45)',
            radius: '4px',
        }, dark));
}

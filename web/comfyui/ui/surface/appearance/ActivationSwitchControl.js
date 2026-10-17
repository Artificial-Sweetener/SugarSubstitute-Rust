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
/** Normalize activation chrome while the card-state service owns node enablement. */
import { ensureCubeFaceActivationStyles } from '../CubeFaceActivationStyles.js';
/** Retain the established native/Fluent switch without relying on Comfy's lazy stylesheet. */
export function createNativeActivationSwitch(document) {
    ensureCubeFaceActivationStyles(document);
    const root = document.createElement('label');
    root.style.display = 'inline-flex';
    root.style.alignItems = 'center';
    root.style.gap = '8px';
    const state = document.createElement('span');
    state.className = 'sugarcubes-cube-face__activation-label';
    const trackRoot = document.createElement('div');
    trackRoot.className = 'p-toggleswitch p-component';
    trackRoot.style.position = 'relative';
    trackRoot.setAttribute('data-pc-name', 'toggleswitch');
    trackRoot.setAttribute('data-pc-section', 'root');
    trackRoot.setAttribute('data-p-disabled', 'false');
    const control = document.createElement('input');
    control.type = 'checkbox';
    control.setAttribute('role', 'switch');
    control.className = 'p-toggleswitch-input';
    control.setAttribute('data-pc-section', 'input');
    const track = document.createElement('div');
    track.className = 'p-toggleswitch-slider';
    track.setAttribute('data-pc-section', 'slider');
    const thumb = document.createElement('div');
    thumb.className = 'p-toggleswitch-handle';
    thumb.setAttribute('data-pc-section', 'handle');
    track.append(thumb);
    trackRoot.append(control, track);
    root.append(state, trackRoot);
    return {
        root,
        control,
        synchronize: (checked, actionLabel, stateLabel) => {
            state.textContent = stateLabel;
            control.checked = checked;
            control.setAttribute('aria-checked', String(checked));
            control.setAttribute('aria-label', actionLabel);
            trackRoot.setAttribute('data-p-checked', String(checked));
            trackRoot.classList.toggle('p-toggleswitch-checked', checked);
        },
    };
}

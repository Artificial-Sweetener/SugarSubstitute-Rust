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
/** Present Cube-owned activation through Comfy's Nodes 2.0 switch vocabulary. */
/** Build one labeled PrimeVue-style switch without taking ownership of activation state. */
export function createCubeFaceActivationControl(documentRef, card, onActivationChange) {
    const control = documentRef.createElement('label');
    control.className = 'sugarcubes-cube-face__activation';
    control.dataset.cubeCardActivation = card.id;
    const stateLabel = documentRef.createElement('span');
    stateLabel.className = 'sugarcubes-cube-face__activation-label';
    const switchRoot = documentRef.createElement('div');
    switchRoot.className = 'p-toggleswitch p-component';
    switchRoot.style.position = 'relative';
    switchRoot.setAttribute('data-pc-name', 'toggleswitch');
    switchRoot.setAttribute('data-pc-section', 'root');
    switchRoot.setAttribute('data-p-disabled', 'false');
    const input = documentRef.createElement('input');
    input.type = 'checkbox';
    input.setAttribute('role', 'switch');
    input.className = 'p-toggleswitch-input';
    input.setAttribute('data-pc-section', 'input');
    const slider = documentRef.createElement('div');
    slider.className = 'p-toggleswitch-slider';
    slider.setAttribute('data-pc-section', 'slider');
    const handle = documentRef.createElement('div');
    handle.className = 'p-toggleswitch-handle';
    handle.setAttribute('data-pc-section', 'handle');
    slider.append(handle);
    switchRoot.append(input, slider);
    control.append(stateLabel, switchRoot);
    updateCubeFaceActivationControl(control, card);
    control.addEventListener('pointerdown', (event) => event.stopPropagation());
    control.addEventListener('click', (event) => event.stopPropagation());
    input.addEventListener('change', () => {
        updateCubeFaceActivationControl(control, { ...card, enabled: input.checked });
        onActivationChange(card.node, input.checked);
    });
    return control;
}
/** Reconcile shared activation chrome without replacing the user's focused native control. */
export function updateCubeFaceActivationControl(control, card) {
    const input = control.querySelector('input');
    const stateLabel = control.querySelector('.sugarcubes-cube-face__activation-label');
    const switchRoot = control.querySelector('.p-toggleswitch');
    if (!input || !stateLabel || !switchRoot)
        throw new Error('Cube activation control is incomplete.');
    const actionLabel = `${card.enabled ? 'Disable' : 'Enable'} ${card.label}`;
    control.dataset.enabled = String(card.enabled);
    stateLabel.textContent = card.enabled ? 'Enabled' : 'Disabled';
    input.checked = card.enabled;
    input.setAttribute('aria-checked', String(card.enabled));
    input.setAttribute('aria-label', actionLabel);
    control.title = actionLabel;
    switchRoot.setAttribute('data-p-checked', String(card.enabled));
    switchRoot.classList.toggle('p-toggleswitch-checked', card.enabled);
}

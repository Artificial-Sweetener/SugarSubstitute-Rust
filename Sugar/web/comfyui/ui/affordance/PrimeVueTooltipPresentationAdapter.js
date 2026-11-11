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
/** Isolate the PrimeVue tooltip state used by Comfy's lazy host tooltips. */
const TOOLTIP_VALUE_PROPERTY = '$_ptooltipValue';
const TOOLTIP_ID_PROPERTY = '$_ptooltipId';
/** Own the narrow PrimeVue compatibility contract required to relabel a host button. */
export class PrimeVueTooltipPresentationAdapter {
    #document;
    #logger;
    #warnedTargets = new WeakSet();
    /** Bind browser and observability collaborators at the host adapter boundary. */
    constructor(options) {
        this.#document = options.document;
        this.#logger = options.logger;
    }
    /** Capture the host-owned values before SugarCubes presents a Cube action. */
    capture(target) {
        return {
            ariaLabel: target.getAttribute('aria-label'),
            title: target.getAttribute('title'),
            hadTooltipValue: Reflect.has(target, TOOLTIP_VALUE_PROPERTY),
            tooltipValue: Reflect.get(target, TOOLTIP_VALUE_PROPERTY),
        };
    }
    /** Present one label through PrimeVue without creating a competing native tooltip. */
    present(target, label) {
        target.setAttribute('aria-label', label);
        target.removeAttribute('title');
        if (Reflect.has(target, TOOLTIP_VALUE_PROPERTY)) {
            Reflect.set(target, TOOLTIP_VALUE_PROPERTY, label);
        }
        else if (!this.#warnedTargets.has(target)) {
            this.#warnedTargets.add(target);
            this.#logger.warn('SugarCubes: Comfy PrimeVue tooltip compatibility contract is unavailable.');
        }
        this.#replaceVisibleTooltipText(target, label);
    }
    /** Restore the exact host values when the action no longer targets a Cube. */
    restore(target, state) {
        restoreAttribute(target, 'aria-label', state.ariaLabel);
        restoreAttribute(target, 'title', state.title);
        if (state.hadTooltipValue)
            Reflect.set(target, TOOLTIP_VALUE_PROPERTY, state.tooltipValue);
        else
            Reflect.deleteProperty(target, TOOLTIP_VALUE_PROPERTY);
        if (typeof state.tooltipValue === 'string') {
            this.#replaceVisibleTooltipText(target, state.tooltipValue);
        }
    }
    /** Synchronize a tooltip that was already mounted when selection changed. */
    #replaceVisibleTooltipText(target, label) {
        const tooltipId = Reflect.get(target, TOOLTIP_ID_PROPERTY);
        if (typeof tooltipId !== 'string' || tooltipId.length === 0)
            return;
        const tooltip = this.#document.getElementById(tooltipId);
        const text = tooltip?.querySelector('[data-pc-section="text"], .p-tooltip-text');
        if (text && text.textContent !== label)
            text.textContent = label;
    }
}
/** Restore one optional host attribute without manufacturing an empty value. */
function restoreAttribute(element, name, value) {
    if (value === null)
        element.removeAttribute(name);
    else
        element.setAttribute(name, value);
}

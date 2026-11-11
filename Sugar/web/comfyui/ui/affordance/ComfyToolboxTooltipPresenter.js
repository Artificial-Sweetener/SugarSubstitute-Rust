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
/** Present a Comfy-styled tooltip for one SugarCubes-owned toolbox action. */
const TOOLTIP_ATTRIBUTE = 'data-sugarcubes-toolbox-tooltip';
/** Own hover, focus, positioning, and cleanup for one portalled action tooltip. */
export class ComfyToolboxTooltipPresenter {
    #document;
    #target = null;
    #label = '';
    #tooltip = null;
    /** Bind tooltip presentation to the current browser document. */
    constructor(document) {
        this.#document = document;
    }
    /** Attach one literal label to a toolbox action. */
    bind(target, label) {
        if (this.#target === target && this.#label === label)
            return;
        this.clear();
        this.#target = target;
        this.#label = label;
        target.addEventListener('pointerenter', this.#show);
        target.addEventListener('pointerleave', this.#hide);
        target.addEventListener('focus', this.#show);
        target.addEventListener('blur', this.#hide);
        target.addEventListener('click', this.#hide);
    }
    /** Remove the active tooltip and its target listeners. */
    clear() {
        this.#hide();
        this.#target?.removeEventListener('pointerenter', this.#show);
        this.#target?.removeEventListener('pointerleave', this.#hide);
        this.#target?.removeEventListener('focus', this.#show);
        this.#target?.removeEventListener('blur', this.#hide);
        this.#target?.removeEventListener('click', this.#hide);
        this.#target = null;
        this.#label = '';
    }
    /** Release all owned browser state. */
    dispose() {
        this.clear();
    }
    /** Mount the same semantic DOM classes used by Comfy's PrimeVue tooltips. */
    #show = () => {
        if (!this.#target || this.#tooltip)
            return;
        const tooltip = this.#document.createElement('div');
        tooltip.setAttribute(TOOLTIP_ATTRIBUTE, '');
        tooltip.setAttribute('role', 'tooltip');
        tooltip.className = 'p-tooltip p-component p-tooltip-top';
        tooltip.style.position = 'fixed';
        tooltip.style.display = 'block';
        tooltip.style.zIndex = '110000';
        tooltip.style.pointerEvents = 'none';
        const arrow = this.#document.createElement('div');
        arrow.className = 'p-tooltip-arrow';
        const text = this.#document.createElement('div');
        text.className = 'p-tooltip-text';
        text.textContent = this.#label;
        tooltip.append(arrow, text);
        this.#document.body.append(tooltip);
        this.#tooltip = tooltip;
        this.#position();
        this.#document.defaultView?.addEventListener('resize', this.#hide);
        this.#document.defaultView?.addEventListener('scroll', this.#hide, true);
    };
    /** Position above the action when space permits and below it otherwise. */
    #position() {
        if (!this.#target || !this.#tooltip)
            return;
        const anchor = this.#target.getBoundingClientRect();
        const tooltip = this.#tooltip;
        const windowRef = this.#document.defaultView;
        const viewportWidth = windowRef?.innerWidth ?? this.#document.documentElement.clientWidth;
        const tooltipWidth = tooltip.offsetWidth;
        const tooltipHeight = tooltip.offsetHeight;
        const left = Math.max(4, Math.min(anchor.left + anchor.width / 2 - tooltipWidth / 2, viewportWidth - tooltipWidth - 4));
        const topPosition = anchor.top - tooltipHeight - 4;
        const placeAbove = topPosition >= 4;
        tooltip.classList.toggle('p-tooltip-top', placeAbove);
        tooltip.classList.toggle('p-tooltip-bottom', !placeAbove);
        tooltip.style.left = `${left}px`;
        tooltip.style.top = `${placeAbove ? topPosition : anchor.bottom + 4}px`;
        const arrow = tooltip.querySelector('.p-tooltip-arrow');
        if (arrow)
            arrow.style.left = `${anchor.left + anchor.width / 2 - left}px`;
    }
    /** Remove one transient tooltip and its viewport listeners. */
    #hide = () => {
        this.#tooltip?.remove();
        this.#tooltip = null;
        this.#document.defaultView?.removeEventListener('resize', this.#hide);
        this.#document.defaultView?.removeEventListener('scroll', this.#hide, true);
    };
}

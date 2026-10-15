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
/** Animate toolbar disclosure while retaining native controls and their value owners. */
import { fluentMotion } from '../../surface/appearance/FluentThemeRuntime.js';
import { setSurfaceGeometryMoving } from '../../surface/motion/SurfaceGeometryMotion.js';
/** Own compact presentation, focus access, and one interruptible width animation. */
export class OverrideToolbarDisclosure {
    store;
    #root;
    #fields;
    #button;
    #icon;
    #geometry;
    #reportError;
    #collapsed = false;
    #effects = [];
    #generation = 0;
    #frame = null;
    #direction = 'right';
    #inline = false;
    /** Bind one native widget strip to its chrome without remounting controls. */
    constructor(root, fields, button, geometry, reportError, store) {
        this.store = store;
        this.#root = root;
        this.#fields = fields;
        this.#button = button;
        this.#geometry = geometry;
        this.#reportError = reportError;
        this.#collapsed = store.readCollapsed();
        this.#icon = root.ownerDocument.createElement('span');
        this.#icon.setAttribute('aria-hidden', 'true');
        button.append(this.#icon);
        button.addEventListener('click', this.#toggle);
        this.#present();
        fields.hidden = this.#collapsed;
    }
    /** Coalesce settled size changes from native mounts and host reattachment. */
    refresh() {
        if (this.#frame !== null)
            return;
        const windowRef = this.#root.ownerDocument.defaultView;
        if (!windowRef?.requestAnimationFrame)
            return;
        this.#frame = windowRef.requestAnimationFrame(() => {
            this.#frame = null;
            if (this.#effects.length || !this.#root.isConnected)
                return;
            this.#measure();
        });
    }
    /** Mirror the disclosure affordance when native docking anchors expansion on the right. */
    setExpansionDirection(direction) {
        if (direction === this.#direction)
            return;
        this.finish();
        this.#direction = direction;
        this.#present();
        this.refresh();
    }
    /** Present inline fields without changing the user's saved compact-toolbar preference. */
    setInline(inline) {
        if (this.#inline === inline)
            return;
        this.finish();
        this.#inline = inline;
        this.#present();
        this.#fields.hidden = this.#collapsed && !inline;
        this.refresh();
    }
    /** Commit the requested resting presentation before a drag or teardown. */
    finish() {
        this.#generation += 1;
        for (const effect of this.#effects)
            effect.cancel();
        this.#effects = [];
        delete this.#root.dataset.scOverrideMoving;
        this.#fields.hidden = this.#collapsed && !this.#inline;
        setSurfaceGeometryMoving(this.#root, false);
    }
    /** Release effects and queued geometry work without touching relation state. */
    dispose() {
        this.finish();
        if (this.#frame !== null)
            this.#root.ownerDocument.defaultView?.cancelAnimationFrame(this.#frame);
        this.#frame = null;
        this.#button.removeEventListener('click', this.#toggle);
    }
    /** Blend from the currently painted width, including reversals during travel. */
    #toggle = () => {
        if (this.#inline)
            return;
        const current = this.#root.getBoundingClientRect();
        const margin = this.#root.ownerDocument.defaultView?.getComputedStyle(this.#fields).marginRight ?? '0px';
        const opacity = this.#effects.length
            ? this.#root.ownerDocument.defaultView?.getComputedStyle(this.#fields).opacity
            : this.#collapsed
                ? '0'
                : '1';
        this.finish();
        this.#collapsed = !this.#collapsed;
        this.store.writeCollapsed(this.#collapsed);
        this.#present();
        if (this.#collapsed &&
            (this.#fields.contains(this.#root.ownerDocument.activeElement) ||
                this.#root
                    .querySelector('.sc-overrides-grip')
                    ?.contains(this.#root.ownerDocument.activeElement)))
            this.#button.focus();
        this.#fields.hidden = this.#collapsed;
        const targetMargin = this.#root.ownerDocument.defaultView?.getComputedStyle(this.#fields).marginRight ?? '0px';
        const target = this.#root.getBoundingClientRect();
        this.#geometry(target.width, target.height);
        const reduced = this.#root.ownerDocument.defaultView?.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
        if (reduced || current.width <= 0 || typeof this.#root.animate !== 'function')
            return;
        this.#fields.hidden = false;
        this.#root.dataset.scOverrideMoving = '';
        setSurfaceGeometryMoving(this.#root, true);
        const generation = this.#generation;
        try {
            const timing = {
                duration: fluentMotion.duration,
                easing: fluentMotion.easing,
                fill: 'both',
            };
            this.#effects.push(this.#root.animate([{ width: `${current.width}px` }, { width: `${target.width}px` }], timing));
            this.#effects.push(this.#fields.animate([
                { opacity: opacity || (this.#collapsed ? '1' : '0'), marginRight: margin },
                { opacity: this.#collapsed ? 0 : 1, marginRight: targetMargin },
            ], timing));
            void Promise.all(this.#effects.map((effect) => effect.finished)).then(() => {
                if (generation !== this.#generation)
                    return;
                this.finish();
                this.#measure();
            }, (error) => {
                if (generation !== this.#generation)
                    return;
                this.finish();
                this.#reportError(error);
            });
        }
        catch (error) {
            // Partial setup can leave an effect whose cancellation rejects its completion promise.
            for (const effect of this.#effects)
                void effect.finished.catch((failure) => {
                    if (generation === this.#generation)
                        this.#reportError(failure);
                });
            this.finish();
            this.#reportError(error);
        }
    };
    /** Keep the pin available while excluding compacted fields from focus and input. */
    #present() {
        const collapsed = this.#collapsed && !this.#inline;
        this.#root.dataset.collapsed = String(collapsed);
        this.#fields.inert = collapsed;
        this.#button.setAttribute('aria-expanded', String(!collapsed));
        const label = this.#collapsed ? 'Expand global overrides' : 'Collapse global overrides';
        this.#button.setAttribute('aria-label', label);
        this.#button.title = label;
        this.#icon.className =
            this.#collapsed === (this.#direction === 'right')
                ? 'icon-[lucide--chevron-right]'
                : 'icon-[lucide--chevron-left]';
    }
    /** Publish only settled toolbar bounds, never animated intermediate sizes. */
    #measure() {
        const rectangle = this.#root.getBoundingClientRect();
        this.#geometry(rectangle.width, rectangle.height);
    }
}

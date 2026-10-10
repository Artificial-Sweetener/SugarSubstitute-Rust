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
/** Own a reversible drawer timeline independently of row reorder transforms and Vue lifetimes. */
import { fluentMotion } from '../appearance/FluentThemeRuntime.js';
/** Slide the rail and cascade its cubes and Add control without exceeding settled positions. */
export class CubeStackDrawerMotion {
    #element;
    #initialInert;
    #effects = [];
    #position = 0;
    #target = false;
    #generation = 0;
    #pending = null;
    #disposed = false;
    /** Hold a newly mounted rail out of sight until its owning surface begins presentation. */
    constructor(element) {
        this.#element = element;
        this.#initialInert = element.inert;
        this.#paint(false);
    }
    /** Respect current reduced-motion settings and hosts without compositor animation support. */
    get canAnimate() {
        return (typeof this.#element.animate === 'function' &&
            !this.#element.ownerDocument.defaultView?.matchMedia?.('(prefers-reduced-motion: reduce)')
                .matches);
    }
    /** Keep settled interactions independent of ordinary row, selection, and appearance refreshes. */
    get settledVisible() {
        return this.#position === fluentMotion.duration && this.#effects.length === 0;
    }
    /** Reverse the same timeline from its current time; resolve false for superseded requests. */
    setVisible(visible) {
        if (this.#disposed)
            return Promise.resolve(false);
        if (this.#target === visible && this.#pending)
            return this.#pending;
        const generation = ++this.#generation;
        this.#target = visible;
        const time = this.#effects[0]?.currentTime;
        if (typeof time === 'number')
            this.#position = time;
        if (!this.canAnimate ||
            (visible ? this.#position >= fluentMotion.duration : this.#position <= 0)) {
            this.#complete(visible);
            return Promise.resolve(true);
        }
        this.#element.inert = true;
        this.#element.dataset.stackMotion = visible ? 'enter' : 'exit';
        if (!this.#effects.length)
            this.#createEffects();
        for (const effect of this.#effects) {
            effect.updatePlaybackRate(visible ? 1 : -1);
            effect.play();
        }
        const pending = Promise.allSettled(this.#effects.map((effect) => effect.finished)).then((results) => {
            if (generation !== this.#generation || this.#disposed)
                return false;
            const failed = results.find((result) => result.status === 'rejected');
            if (failed?.status === 'rejected')
                throw new Error('Cube stack drawer animation failed.', { cause: failed.reason });
            this.#complete(visible);
            return true;
        });
        this.#pending = pending;
        return pending;
    }
    /** Release compositor effects immediately before renderer, graph, or document teardown. */
    dispose() {
        this.#disposed = true;
        this.#generation += 1;
        this.#cancelEffects();
        this.#element.style.removeProperty('opacity');
        this.#element.style.removeProperty('translate');
        this.#element.inert = this.#initialInert;
        delete this.#element.dataset.stackMotion;
    }
    /** Cascade cubes followed by Add on one duration, preserving independent reorder transforms. */
    #createEffects() {
        const add = (element, frames, easing) => {
            const effect = element.animate(frames, {
                duration: fluentMotion.duration,
                easing,
                fill: 'both',
            });
            effect.pause();
            effect.currentTime = this.#position;
            this.#effects.push(effect);
        };
        add(this.#element, [
            { translate: '-16px 0', opacity: 0 },
            { translate: '0 0', opacity: 1 },
        ], fluentMotion.easing);
        const cascadeItems = [
            ...this.#element.querySelectorAll('.sc-cube-stack__rows > .sc-cube-stack-row'),
        ];
        const addButton = this.#element.querySelector('.sc-cube-stack__add');
        if (addButton)
            cascadeItems.push(addButton);
        cascadeItems.forEach((item, index) => {
            const delay = Math.min(0.25, index * 0.055);
            const start = { translate: '0 32px', opacity: 0, offset: 0 };
            add(item, [
                ...(delay ? [start] : []),
                { ...start, offset: delay, easing: fluentMotion.easing },
                { translate: '0 0', opacity: 1, offset: 1 },
            ], 'linear');
        });
    }
    /** Commit the exact endpoint before clearing effects that could interfere with row removal. */
    #complete(visible) {
        this.#position = visible ? fluentMotion.duration : 0;
        this.#pending = null;
        this.#paint(visible);
        this.#cancelEffects();
        delete this.#element.dataset.stackMotion;
    }
    /** Keep an outgoing rail inert while preserving the painted endpoint during deferred disposal. */
    #paint(visible) {
        this.#element.style.opacity = visible ? '1' : '0';
        // Even zero translation creates a containing block for the viewport-positioned drag overlay.
        this.#element.style.translate = visible ? 'none' : '-16px 0';
        this.#element.inert = visible ? this.#initialInert : true;
    }
    /** Cancel only owned effects; their finished promises are consumed by the current request. */
    #cancelEffects() {
        for (const effect of this.#effects.splice(0))
            effect.cancel();
    }
}

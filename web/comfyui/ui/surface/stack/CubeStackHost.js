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
/** Bind one Vue rail to masonry, accepted appearance, persisted preferences and graph use cases. */
import { readOwnedVisualStyle, subscribeVisualStyle } from '../appearance/VisualStyleBoundary.js';
import { applyOwnedTheme } from '../appearance/ThemeService.js';
import { CubeStackPreferences } from './CubeStackPreferences.js';
import { projectCubeStackRows } from './CubeStackProjection.js';
import { ensureCubeStackStyles } from './CubeStackStyles.js';
import { mountCubeStackView } from './vue/CubeStackVueRuntime.js';
import { CubeStackScrollSelection } from './CubeStackScrollSelection.js';
import { CubeStackDrawerMotion } from './CubeStackDrawerMotion.js';
/** Retain one presentation application for a mounted surface and release it before native teardown. */
export class CubeStackHost {
    frameControls;
    #options;
    #preferences;
    #actions;
    #subscriptions = [];
    #view = null;
    #element = null;
    #mounted = null;
    #motion = null;
    #surfaceVisible = true;
    #reorder = null;
    #unsubscribeReorder = null;
    #selectedId = null;
    #scrollSelection = null;
    #started = false;
    #disposed = false;
    #frameListeners = new Set();
    #frameSignature = '';
    /** Compose typed intent handlers while preference and graph services keep their own authority. */
    constructor(options) {
        this.#options = options;
        this.#preferences = new CubeStackPreferences(options.app);
        const preferences = this.#preferences;
        const host = this;
        this.frameControls = {
            get available() {
                return host.#frameAvailable();
            },
            get action() {
                return host.#frameAction();
            },
            activate: async () => {
                if (!this.#frameAvailable())
                    return;
                if (this.#frameAction() === 'add')
                    options.openCart();
                else
                    await preferences.setCompact(!preferences.snapshot.compact);
            },
            menu: () => this.presentationMenu(),
            subscribe: (listener) => {
                this.#frameListeners.add(listener);
                return () => this.#frameListeners.delete(listener);
            },
        };
        this.#actions = {
            navigate: (id) => this.#navigate(id),
            previewOrder: (order) => this.#view?.previewCubeOrder(order?.map((id) => `cube:${id}`) ?? null),
            reorder: (id, order) => this.#mutate((service) => service.reorder(id, order)),
            move: (id, delta) => this.#mutate((service) => service.move(id, delta)),
            openCart: options.openCart,
            setCompact: (compact) => void this.#preferences.setCompact(compact).catch(options.reportError),
            hide: () => void this.#preferences.setVisible(false).catch(options.reportError),
            remove: options.remove,
            contextMenu: options.contextMenu,
            reportError: options.reportError,
        };
    }
    /** Restore persisted preferences and subscribe only to the authoritative presentation owners. */
    setup() {
        if (this.#started || this.#disposed)
            return;
        this.#preferences.setup();
        this.#started = true;
        ensureCubeStackStyles(this.#options.document);
        this.#subscriptions.push(this.#preferences.subscribe(() => this.#reconcile()), this.#options.appearance.subscribeAppearance(() => this.#reconcile()), subscribeVisualStyle(this.#options.document, () => this.#reconcile()));
        if (this.#options.frame)
            this.#subscriptions.push(this.#options.frame.subscribe(() => this.#reconcile(false)));
    }
    /** Expose rail preference intents to the frame without creating another persistence owner. */
    presentationMenu() {
        const { visible, compact } = this.#preferences.snapshot;
        return [
            {
                label: 'Show Cube stack',
                checked: visible,
                selection: 'multiple',
                select: () => {
                    void this.#preferences.setVisible(!visible).catch(this.#options.reportError);
                },
            },
            {
                label: 'Compact Cube stack',
                checked: compact,
                selection: 'multiple',
                select: () => {
                    void this.#preferences.setCompact(!compact).catch(this.#options.reportError);
                },
            },
        ];
    }
    /** Attach to already projected sections without triggering graph candidate discovery. */
    ready(view, preparing = false) {
        if (this.#disposed)
            return;
        const restoring = this.#view !== view && !preparing;
        if (this.#view !== view) {
            this.release();
            this.#view = view;
            this.#surfaceVisible = !preparing;
            this.#scrollSelection = new CubeStackScrollSelection(view.element, (id) => {
                this.#selectedId = id;
                this.#mounted?.update(this.#state());
            });
        }
        this.#reconcile(!restoring);
    }
    /** Start the rail on the owning masonry transition and retain it through reverse travel. */
    async transition(entering) {
        this.#surfaceVisible = entering;
        this.#publishFrame();
        await this.#present(entering && this.#stackVisible());
    }
    /** Refresh literal row presentation after an identified card or canonical permission change. */
    valuesChanged() {
        this.#reconcile();
    }
    /** Release preview, reserved width, Vue controls and graph subscriptions with the current surface. */
    release() {
        this.#options.closeTransient();
        this.#unmount();
        this.#unsubscribeReorder?.();
        this.#unsubscribeReorder = null;
        this.#reorder = null;
        this.#view = null;
        this.#selectedId = null;
        this.#scrollSelection?.dispose();
        this.#scrollSelection = null;
        this.#publishFrame();
    }
    /** Restore every retained subscription before extension or document teardown. */
    dispose() {
        if (this.#disposed)
            return;
        this.release();
        this.#disposed = true;
        for (const unsubscribe of this.#subscriptions.splice(0))
            unsubscribe();
        this.#preferences.dispose();
        this.#frameListeners.clear();
    }
    /** Match one rail lifetime to the current mounted surface and accepted visibility preference. */
    #reconcile(animate = true) {
        const view = this.#view;
        if (!this.#started || this.#disposed || !view) {
            this.#publishFrame();
            return;
        }
        const reorder = this.#options.getReorder();
        if (this.#reorder !== reorder) {
            this.#unsubscribeReorder?.();
            this.#reorder = reorder;
            this.#unsubscribeReorder = reorder?.subscribe(() => this.#reconcile()) ?? null;
        }
        if (!this.#stackVisible()) {
            this.#view?.previewCubeOrder(null);
            this.#view?.setStackWidth(0);
            if (!this.#motion?.canAnimate)
                this.#unmount();
            else {
                const element = this.#element;
                void this.#present(false)
                    .then((completed) => {
                    if (completed && this.#element === element && !this.#stackVisible())
                        this.#unmount();
                })
                    .catch(this.#options.reportError);
            }
            this.#publishFrame();
            return;
        }
        const container = view.element.parentElement;
        if (!container)
            return;
        const state = this.#state();
        view.setCubeOrder(state.rows.map((row) => `cube:${row.id}`));
        if (!this.#element) {
            this.#element = this.#options.document.createElement('aside');
            this.#element.className = 'sc-cube-stack-host';
            container.append(this.#element);
        }
        const appearance = this.#options.appearance.appearance;
        this.#element.dataset.appearance = state.appearance;
        this.#element.dataset.colorScheme = appearance.dark ? 'dark' : 'light';
        applyOwnedTheme(this.#element, appearance.style !== 'comfy' || this.#options.frame?.enabled ? appearance.dark : null);
        if (this.#mounted)
            this.#mounted.update(state);
        else {
            this.#mounted = mountCubeStackView(this.#element, state, this.#actions);
            this.#motion = new CubeStackDrawerMotion(this.#element);
            this.#mounted.setInteractive(false);
        }
        view.setStackWidth(this.#options.frame?.enabled ? (state.compact ? 58 : 224) : state.compact ? 82 : 248);
        if (this.#surfaceVisible)
            void this.#present(true, animate).catch(this.#options.reportError);
        this.#scrollSelection?.refresh(state.rows.map((row) => row.id));
        this.#publishFrame();
    }
    /** Offer the cart for empty masonry and compact controls for a presented stack. */
    #frameAvailable() {
        return (!this.#disposed &&
            this.#view !== null &&
            this.#surfaceVisible &&
            (!this.#hasCubes() || (this.#mounted !== null && this.#preferences.snapshot.visible)));
    }
    /** Use the mounted top-level projection as the sole cube-presence authority. */
    #hasCubes() {
        return this.#view?.sections.some((section) => section.cube !== null) === true;
    }
    /** Keep an empty workflow out of rail layout without changing persisted preferences. */
    #stackVisible() {
        return this.#preferences.snapshot.visible && this.#hasCubes();
    }
    /** Resolve the toolbar intent independently of whether graph mode currently disables it. */
    #frameAction() {
        return !this.#hasCubes() ? 'add' : this.#preferences.snapshot.compact ? 'expand' : 'collapse';
    }
    /** Publish only changed availability or intent to frame affordances. */
    #publishFrame() {
        const signature = `${this.#frameAvailable()}:${this.#frameAction()}`;
        if (signature === this.#frameSignature)
            return;
        this.#frameSignature = signature;
        for (const listener of this.#frameListeners)
            listener();
    }
    /** Derive all row facts from the current projection and exact canonical analysis snapshot. */
    #state() {
        return {
            rows: projectCubeStackRows(this.#view?.sections ?? [], this.#reorder?.analysis ?? null, this.#selectedId, this.#options.resolveSource, this.#reorder?.available === true),
            compact: this.#preferences.snapshot.compact,
            pending: this.#reorder?.pending === true,
            appearance: this.#options.appearance.appearance.style !== 'comfy'
                ? this.#options.appearance.appearance.style
                : this.#options.frame?.enabled
                    ? readOwnedVisualStyle(this.#options.document)
                    : 'comfy',
        };
    }
    /** Restore filtered sections through the search owner before scrolling to stable identity. */
    #navigate(instanceId) {
        if (!this.#state().rows.some((row) => row.id === instanceId))
            return;
        this.#options.revealSearchResults();
        this.#scrollSelection?.navigate(instanceId);
        this.#view?.scrollToSection(`cube:${instanceId}`);
        this.#reconcile();
    }
    /** Settle the accepted graph projection before Vue clears its temporary drag permutation. */
    async #mutate(work) {
        const reorder = this.#reorder;
        if (!reorder)
            throw new Error('Cube graph reorder is unavailable.');
        await work(reorder);
        await reorder.refresh();
        await this.#options.refreshSurface();
        this.#reconcile();
    }
    /** Suspend captured drag intent while motion owns geometry and re-enable only the latest entry. */
    async #present(visible, animate = true) {
        const motion = this.#motion;
        const mounted = this.#mounted;
        if (!motion || !mounted)
            return false;
        if (!animate) {
            motion.restore(visible);
            mounted.setInteractive(visible);
            return true;
        }
        if (visible && motion.settledVisible) {
            mounted.setInteractive(true);
            return true;
        }
        if (!motion.canAnimate)
            mounted.setInteractive(visible);
        else
            mounted.setInteractive(false);
        const completed = await motion.setVisible(visible);
        if (completed && motion === this.#motion && visible)
            mounted.setInteractive(true);
        return completed;
    }
    /** Remove owned presentation while retaining preferences for the next eligible masonry surface. */
    #unmount() {
        this.#motion?.dispose();
        this.#motion = null;
        this.#view?.previewCubeOrder(null);
        this.#view?.setStackWidth(0);
        this.#mounted?.dispose();
        this.#mounted = null;
        this.#element?.remove();
        this.#element = null;
    }
}

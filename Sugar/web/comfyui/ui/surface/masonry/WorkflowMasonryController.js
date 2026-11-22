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
/** Own the reversible workflow presentation state machine and renderer fallback. */
import { NativeRendererAcquisition } from '../NativeRendererAcquisition.js';
import { requireCubeIdentity } from '../../cube/node/ComfyCubeNodeFactory.js';
import { WorkflowMasonryView } from './WorkflowMasonryView.js';
import { WorkflowMasonryTransition } from './WorkflowMasonryTransition.js';
import { WorkflowMasonryGraphReveal } from './WorkflowMasonryGraphReveal.js';
import { WorkflowMasonryGraphVisibility } from './WorkflowMasonryGraphVisibility.js';
import { markMasonryMounted } from './WorkflowMasonryStyles.js';
import { markWorkflowSwitch } from '../../core/WorkflowSwitchTiming.js';
import { prepareNativePresentation } from '../NativePresentationReady.js';
import { SurfacePresentationBoundary } from '../motion/SurfacePresentationBoundary.js';
import { MasonryScrollKeeper } from './MasonryScrollKeeper.js';
import { measureWorkflowMasonryGraph, workflowMasonryGraphCards, } from './WorkflowMasonryGraphGeometry.js';
/** Keep layout choice transient and restore graph presentation on every failure path. */
export class WorkflowMasonryController {
    #options;
    #transition = new WorkflowMasonryTransition();
    #settled;
    #acquisition;
    #reveal = null;
    #view = null;
    #container = null;
    /** The masonry surface left in place while one workflow's view is replaced by the next. */
    #switchCover = null;
    #desired = false;
    #suspensions = new Set();
    #generation = 0;
    #mounting = null;
    #topology = '';
    /** The exact nodes the mounted cards present; a replaced graph reuses ids, not objects. */
    #nodes = [];
    #renderer = null;
    #links = null;
    #graphVisibility;
    /** Keeps the mounted workflow's masonry scroll. */
    #scroll = null;
    /** Bind host capabilities without importing host globals or owning graph mutations. */
    constructor(options) {
        this.#options = options;
        this.#graphVisibility = new WorkflowMasonryGraphVisibility(options.getContainer);
        this.#acquisition = new NativeRendererAcquisition(options.document);
        this.#settled = new SurfacePresentationBoundary(options.document);
    }
    /** Expose the requested presentation mode for button and future stack navigation. */
    get active() {
        return this.#desired && this.#suspensions.size === 0;
    }
    /** Retain graph-only control exclusion until the departing masonry surface is released. */
    get presenting() {
        return ((this.#desired && !this.#suspensions.has('app') && !this.#suspensions.has('subgraph')) ||
            this.#view !== null);
    }
    /** Restore a committed workflow choice without replaying an explicit user-toggle transition. */
    async restore(desired) {
        if (this.#desired === desired)
            return;
        this.#desired = desired;
        const generation = ++this.#generation;
        this.#transition.cancel();
        this.#releaseView(this.#retainGraphSuppression());
        if (this.#retainGraphSuppression())
            this.#graphVisibility.setHidden(true);
        this.#options.onStateChange();
        if (this.active && this.#options.isNodes2())
            await this.#show(generation, false);
    }
    /** Release native mounts during host transitions without changing the user's view choice. */
    async setSuspended(reason, suspended) {
        if (this.#suspensions.has(reason) === suspended)
            return;
        if (suspended)
            this.#suspensions.add(reason);
        else
            this.#suspensions.delete(reason);
        const generation = ++this.#generation;
        this.#transition.cancel();
        if (reason === 'configure')
            markWorkflowSwitch(suspended ? 'configure-start' : 'configure-end');
        // While a masonry is replaced by the next workflow's, its surface stays covered: the cards
        // leave, but the area never drops to the bare graph container between the two workflows.
        if (reason === 'configure' && suspended && this.#container && this.#retainGraphSuppression())
            this.#coverSwitch(this.#container);
        this.#releaseView(this.#retainGraphSuppression());
        if (this.#retainGraphSuppression())
            this.#graphVisibility.setHidden(true);
        this.#options.onStateChange();
        if (!this.active || !this.#options.isNodes2())
            return;
        // Mode/editor changes precede native visibility; configuration resumes after the observed load commits.
        if (reason !== 'configure')
            await this.#settled.settled(1);
        if (generation === this.#generation && this.active)
            await this.#show(generation, false);
    }
    /** Reverse the presentation with one authoritative state transition. */
    async toggle() {
        if (!this.#options.isNodes2() || this.#suspensions.size)
            return;
        this.#desired = !this.#desired;
        const generation = ++this.#generation;
        this.#acquisition.cancel();
        this.#settled.cancel();
        this.#links?.hold(!this.#desired);
        this.#options.onStateChange();
        if (!this.#desired) {
            this.#graphVisibility.setHidden(false);
            try {
                if (!this.#view?.element.hasAttribute('data-sc-masonry-transition'))
                    await this.#view?.settlePresentation();
                if (generation !== this.#generation)
                    return;
                await this.#animate(false);
                if (generation !== this.#generation)
                    return;
                const renderer = this.#renderer;
                this.#graphVisibility.holdInput();
                this.#disposePresentation();
                renderer?.requestSlotLayoutSync?.();
                if (this.#links) {
                    await this.#settled.settled();
                    if (generation !== this.#generation || !this.#container)
                        return;
                    this.#links.dispose();
                    this.#links = null;
                    this.#links = this.#options.createLinkMotion?.(this.#container) ?? null;
                    this.#links?.hold(true);
                    await this.#links?.run(false, this.#reducedMotion());
                }
            }
            finally {
                if (generation === this.#generation)
                    this.#releaseView();
            }
            return;
        }
        await this.#show(generation, true);
    }
    /** Mount the current graph only while this presentation request still owns the lifecycle. */
    async #show(generation, animate) {
        try {
            if (!this.#view) {
                const renderer = await this.#acquisition.acquire(() => this.#options.getRenderer());
                if (!renderer)
                    return;
                if (generation !== this.#generation || !this.active || !this.#options.isNodes2())
                    return;
                const container = this.#options.getContainer();
                if (!container)
                    throw new Error('Comfy graph container is not mounted.');
                const sections = this.#options.getSections();
                this.#renderer = renderer;
                this.#topology = topologySignature(sections);
                this.#nodes = cardNodes(sections);
                this.#view = new WorkflowMasonryView({
                    document: this.#options.document,
                    sections,
                    cardState: this.#options.getCardState(),
                    getSources: () => workflowMasonryGraphCards(container, this.#view?.sections ?? sections),
                    openCube: (cube) => this.#options.openCube(cube),
                    onCardsChanged: () => {
                        if (this.#view && this.#desired)
                            this.#options.onSurfaceReady?.(this.#view, animate);
                    },
                });
                this.#container = container;
                this.#scroll = new MasonryScrollKeeper(this.#view.element, this.#options.bindScroll(), this.#options.reportError);
                this.#reveal ??= new WorkflowMasonryGraphReveal();
                this.#reveal.reconcile(container, sections);
                markWorkflowSwitch('masonry-show');
                const sources = this.#view.prepareSources();
                container.append(this.#view.backdrop, this.#view.element, this.#view.tail);
                this.#uncoverSwitch();
                markMasonryMounted(container);
                // Appearance owners mark the surface while it is empty; marked after the cards arrive,
                // every card is restyled again.
                prepareNativePresentation(this.#view.element);
                this.#options.onSurfaceMounting?.(this.#view, animate);
                // The cards are still inside the layers; `inert` would restyle them all before they leave.
                this.#graphVisibility.holdInput();
                this.#mounting = this.#view.mountCards(renderer, sources);
            }
            await this.#mounting;
            markWorkflowSwitch('masonry-cards-mounted');
            if (generation !== this.#generation || !this.#desired)
                return;
            if (this.#view?.element.hasAttribute('data-sc-masonry-preparing')) {
                this.#renderer?.requestSlotLayoutSync?.();
                await this.#view.settlePresentation();
            }
            if (generation !== this.#generation || !this.#desired)
                return;
            if (this.#view) {
                this.#options.onSurfaceReady?.(this.#view, animate);
                await this.#animate(true, animate);
                if (generation === this.#generation)
                    this.#scroll?.settle();
            }
            if (generation === this.#generation && this.#desired) {
                this.#graphVisibility.setHidden(true);
                markWorkflowSwitch('masonry-ready');
            }
        }
        catch (error) {
            if (generation !== this.#generation)
                return;
            this.leaveImmediately();
            this.#options.reportError(error);
        }
        finally {
            // The input hold placed before the cards left the layers belongs to this run: a run that
            // still owns the lifecycle but ended before hiding the graph gives the layers their input
            // back here instead of leaving them held for a later run to find.
            if (generation === this.#generation && !this.#graphVisibility.hidden)
                this.#graphVisibility.releaseInput();
        }
    }
    /** Restore graph immediately before graph replacement, renderer teardown, or application disposal. */
    leaveImmediately() {
        this.#desired = false;
        this.#generation += 1;
        this.#transition.cancel();
        this.#settled.cancel();
        this.#releaseView();
        this.#options.onStateChange();
    }
    /** Explain the unsupported renderer once, only when it interrupts active masonry. */
    rendererChanged() {
        const unsupported = !this.#options.isNodes2();
        if (unsupported && (this.#desired || this.#view)) {
            this.leaveImmediately();
            try {
                this.#options.notifyRendererFallback();
            }
            catch (error) {
                this.#options.reportError(error);
            }
        }
        this.#options.onStateChange();
    }
    /** Address sections by identity so a Cube stack can share this navigation surface. */
    scrollToSection(id) {
        this.#view?.scrollToSection(id);
    }
    /** Reconcile card presentation without replaying transitions or closing the optional-node menu. */
    async graphChanged() {
        if (!this.#desired || !this.#view || !this.#renderer)
            return;
        const generation = this.#generation;
        await this.#mounting;
        if (generation !== this.#generation || !this.#desired || !this.#view)
            return;
        const sections = this.#options.getSections();
        if (this.#container)
            this.#reveal?.reconcile(this.#container, sections);
        const topology = topologySignature(sections);
        const nodes = cardNodes(sections);
        if (this.#topology === topology &&
            nodes.length === this.#nodes.length &&
            nodes.every((node, index) => node === this.#nodes[index])) {
            this.#options.onSurfaceValuesChanged?.();
            return;
        }
        this.#topology = topology;
        this.#nodes = nodes;
        try {
            this.#mounting = this.#view.refresh(sections, this.#renderer);
            await this.#mounting;
            if (generation === this.#generation && this.#desired && this.#view)
                this.#options.onSurfaceReady?.(this.#view, false);
        }
        catch (error) {
            if (generation !== this.#generation)
                return;
            this.leaveImmediately();
            this.#options.reportError(error);
        }
    }
    /** Assemble native cards from actual graph bounds without changing saved node positions. */
    async #animate(entering, animate = true) {
        const view = this.#view;
        const container = this.#container;
        if (!view || !container)
            return;
        this.#graphVisibility.setInert(true);
        view.setTransitioning(true);
        const reducedMotion = !animate || this.#reducedMotion();
        if (reducedMotion && this.#links) {
            const links = this.#links;
            this.#links = null;
            links.dispose();
        }
        if (!reducedMotion && !this.#links)
            this.#links = this.#options.createLinkMotion?.(container) ?? null;
        const noodles = entering ? this.#links?.run(true, reducedMotion) : undefined;
        await Promise.all([
            noodles,
            animate ? this.#options.onSurfaceTransition?.(entering) : undefined,
            this.#transition.run({
                surface: view.element,
                backdrop: view.backdrop,
                cards: view.transitionCards(),
                measureGraph: () => measureWorkflowMasonryGraph(container, view.sections),
                ...(this.#reveal ? { reveal: this.#reveal } : {}),
            }, entering, reducedMotion),
        ]);
        if (entering && !view.element.hasAttribute('data-sc-masonry-transition'))
            view.setTransitioning(false, animate);
    }
    /** Cover `container` with the masonry surface until the next view mounts its own. */
    #coverSwitch(container) {
        this.#uncoverSwitch();
        const cover = this.#options.document.createElement('div');
        cover.className =
            'sugarcubes-workflow-masonry__backdrop sugarcubes-workflow-masonry__switch-cover';
        container.append(cover);
        this.#switchCover = cover;
    }
    /** Remove the switch cover, once the next view's surface or the graph takes its place. */
    #uncoverSwitch() {
        this.#switchCover?.remove();
        this.#switchCover = null;
    }
    /** Remove presentation resources while preserving canonical graph and edit state. */
    #releaseView(retainGraphSuppression = false) {
        this.#acquisition.cancel();
        if (!retainGraphSuppression) {
            this.#uncoverSwitch();
            this.#graphVisibility.setHidden(false);
        }
        this.#settled.cancel();
        const links = this.#links;
        this.#links = null;
        try {
            links?.dispose();
        }
        finally {
            this.#reveal?.dispose();
            this.#reveal = null;
            this.#disposePresentation();
            this.#container = null;
        }
    }
    /** Return borrowed controls before recapturing settled native noodle endpoints. */
    #disposePresentation() {
        this.#scroll?.dispose();
        this.#scroll = null;
        // Rail reservation is part of the animation's coordinate space until card travel completes.
        if (this.#view)
            this.#options.onSurfaceLeaving?.();
        this.#view?.dispose();
        this.#view = null;
        this.#mounting = null;
        this.#renderer = null;
    }
    /** Respect the document's current motion preference on each transition phase. */
    #reducedMotion() {
        return (this.#options.document.defaultView?.matchMedia?.('(prefers-reduced-motion: reduce)')
            .matches ?? false);
    }
    /** Cover graph replacement until the committed destination can safely expose its own view. */
    #retainGraphSuppression() {
        return ((this.#desired || this.#suspensions.has('configure')) &&
            !this.#suspensions.has('app') &&
            !this.#suspensions.has('subgraph'));
    }
}
/** List the node objects behind every card, in presentation order. */
function cardNodes(sections) {
    return sections.flatMap((section) => section.cards.map((card) => card.node));
}
/** Fingerprint presentation membership without treating widget edits as remount requests. */
function topologySignature(sections) {
    return JSON.stringify(sections.map((section) => [
        section.id,
        section.label,
        section.cube ? requireCubeIdentity(section.cube) : null,
        section.cards.map((card) => [
            card.id,
            card.node.type,
            card.label,
            card.columnSpan,
            card.visible,
            card.enabled,
            card.showActivationControl,
        ]),
    ]));
}

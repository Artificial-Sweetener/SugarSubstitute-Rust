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
/** Own reversible model-picker mounts independently of theme and layout preferences. */
import { ModelDetectorDialog } from './ModelDetectorDialog.js';
import { ModelPickerSession } from './ModelPickerSession.js';
import { ModelMenu } from './ModelMenu.js';
import { ComfySettingStore } from '../settings/workspace/ComfySettingStore.js';
import { settingsCatalogFor } from '../settings/workspace/SettingsCatalog.js';
import { isRecord } from '../types/common.js';
import { HostSurfaceObserver } from '../core/HostSurfaceObserver.js';
import { createComfyRendererModeChangeSource, resolveComfyRendererMode, } from '../core/ComfyRendererMode.js';
import { ModelPickerAppearance } from './ModelPickerAppearance.js';
import { readComfyModelFields } from './ComfyModelFieldAdapter.js';
import { ComfyNodeVNodeIndex } from '../surface/ComfyVueTree.js';
import { modelPickerChoices, orderModelsByTitle } from './ModelPickerChoices.js';
import { mountModelPicker } from './vue/ModelPickerVueRuntime.js';
import { mountModelGallery } from './vue/ModelGalleryVueRuntime.js';
import { openModelChooser } from './vue/ModelChooserVueRuntime.js';
import { ModelPickerSessions } from './ModelPickerSessions.js';
import { ModelEmptyPickerDiscovery } from './ModelEmptyPickerDiscovery.js';
import { ModelPageActivity } from './ModelPageActivity.js';
import { comfyModelCompatibilityContext } from './ComfyModelCompatibilityContext.js';
import { mountedModelContextContributions } from './ModelMountedContextContributions.js';
import { resolveCubeInstalledModel } from './CubeInstalledModelResolution.js';
import { registerNativeFieldEnhancement } from '../surface/NativeFieldPresentation.js';
/** Persist model-picker presentation separately from visual style. */
export const MODEL_PICKER_SETTING = 'SugarCubes.Models.EnhancedPicker';
/** Follow node and renderer lifetimes while keeping Comfy authoritative for field values. */
export class ModelPickerHost {
    #options;
    #sessions;
    #menu = null;
    #discovery;
    #page;
    #appearance;
    #observer = null;
    #popups = new Set();
    #mounts = new Map();
    #published = new WeakMap();
    #dirty = new Set();
    #snapshot = null;
    #enabled = false;
    #active = false;
    #setup = false;
    #disposed = false;
    #frame = null;
    #poll = null;
    #loadFailures = 0;
    #unsubscribe = null;
    #unsubscribeCatalog = null;
    #generation = 0;
    #resolutionPasses = 0;
    /** Prepare discovery without mounting until the host preference enables it. */
    constructor(options) {
        this.#options = options;
        this.#sessions = new ModelPickerSessions(options.document, () => this.#snapshot, options.app);
        this.#page = options.document.defaultView
            ? new ModelPageActivity(options.document.defaultView, () => this.#reconcile(), () => this.dispose())
            : null;
        this.#discovery = options.discovery
            ? new ModelEmptyPickerDiscovery({
                ...options.discovery,
                document: options.document,
                snapshot: () => this.#snapshot,
                sessions: this.#sessions,
                compatibility: (binding, snapshot) => comfyModelCompatibilityContext(options.app, binding, snapshot),
                reportError: options.reportError,
            })
            : null;
        this.#appearance = new ModelPickerAppearance(options.document, options.app);
    }
    /** Register the independent Nodes 2.0 preference and owned lifecycle subscriptions. */
    setup() {
        if (this.#setup || this.#disposed)
            return;
        this.#setup = true;
        const app = isRecord(this.#options.app) ? this.#options.app : {};
        const ui = isRecord(app.ui) ? app.ui : {};
        const settings = isRecord(ui.settings) ? ui.settings : {};
        if (typeof settings.addSetting !== 'function')
            throw new Error('Comfy settings are unavailable for model pickers.');
        settings.addSetting.call(settings, {
            id: MODEL_PICKER_SETTING,
            name: 'Enhanced model pickers',
            type: 'hidden',
            defaultValue: true,
            onChange: (value) => this.setEnabled(value === true),
        });
        settingsCatalogFor(this.#options.app).contribute('model_sources.model_library.enhanced_picker', {
            controls: [
                {
                    kind: 'switch',
                    value: new ComfySettingStore(this.#options.app).value(MODEL_PICKER_SETTING, (value) => value !== false),
                },
            ],
        });
        this.#observer = new HostSurfaceObserver(this.#options.document.body, '.lg-node,.sc-model-popup', (changes) => {
            if (!this.#active)
                return;
            for (const { element, kind } of changes) {
                if (element.matches('.sc-model-popup')) {
                    if (kind === 'removed') {
                        this.#appearance.release(element);
                        this.#popups.delete(element);
                    }
                    else {
                        this.#popups.add(element);
                        this.#appearance.track(element);
                    }
                }
                else if (!element.closest('.sc-model-mount,[data-sc-floating-surface]'))
                    this.#dirty.add(element);
            }
            if (this.#dirty.size)
                this.#schedule();
        }, {
            content: true,
            attributes: ['aria-expanded'],
            exclude: '.sc-model-mount,[data-sc-floating-surface]',
        });
        this.#options.document.addEventListener('focusout', this.#nativeFocusOut);
        this.#unsubscribe = createComfyRendererModeChangeSource(this.#options.app).subscribe(() => this.#reconcile());
        this.#page?.start();
        const value = typeof settings.getSettingValue === 'function'
            ? settings.getSettingValue.call(settings, MODEL_PICKER_SETTING)
            : true;
        this.setEnabled(value !== false);
    }
    /** Suspend enhancement without changing any workflow widget value. */
    setEnabled(enabled) {
        this.#enabled = enabled;
        this.#reconcile();
    }
    /** Contribute mounted field actions without rediscovering models or duplicating commands. */
    contextContributions(root) {
        return mountedModelContextContributions(this.#mounts.values(), root, this.#options.app, this.#snapshot, () => this.#resolveCubeFields());
    }
    /**
     * Mount the picker's popup surface and wall for an owner that keeps its own candidates.
     *
     * The surface is laid out by its container and themed like every picker popup, whether or not
     * field pickers are enabled.
     */
    mountGallery(events) {
        const element = this.#options.document.createElement('div');
        element.className = 'sc-model-popup sc-model-popup-inline';
        const gallery = mountModelGallery(element, events, this.#shown);
        this.#appearance.track(element);
        return {
            element,
            update: (items, active) => gallery.update(items, active),
            neighbor: (active, key) => gallery.neighbor(active, key),
            dispose: () => {
                gallery.dispose();
                this.#appearance.release(element);
                element.remove();
            },
        };
    }
    /**
     * Let a person choose one installed model of a kind from the picker popup opened at a point.
     *
     * The popup has its own search box and folders. It resolves with the chosen model, or null
     * when it is dismissed or the catalog cannot be read.
     */
    async chooseModel(kind, label, anchor) {
        const snapshot = this.#snapshot ?? (await this.#options.catalog.load());
        if (this.#disposed)
            return null;
        const items = orderModelsByTitle(snapshot.items.filter((item) => item.kind === kind));
        return new Promise((resolve) => {
            // A model chosen here belongs to no workflow field, so a chosen version applies nowhere.
            const session = new ModelPickerSession(this.#options.document, () => null);
            const request = {
                items,
                label,
                anchor,
                context: this.#context(),
                session,
                shown: this.#shown,
            };
            const chooser = openModelChooser(this.#options.document, request, (item) => {
                if (chooser.popup)
                    this.#appearance.release(chooser.popup);
                resolve(item);
            });
            if (chooser.popup)
                this.#appearance.track(chooser.popup);
        });
    }
    /**
     * The rows about one model, for a menu SugarCubes opens outside the picker, such as a prompt's
     * LoRA banner: the same rows every model picker surface shows.
     */
    modelMenu(item) {
        if (this.#disposed)
            return [];
        // A model chosen here belongs to no workflow field, so a chosen version applies nowhere.
        this.#menu ??= new ModelMenu(this.#context(), new ModelPickerSession(this.#options.document, () => null));
        return this.#menu.project(item);
    }
    /**
     * Every wall this host mounts reports the models it has on screen, and the model library looks
     * those up before the rest. A failed report offers a person nothing to act on; the next
     * report asks again.
     */
    #shown = (items) => {
        void this.#options.catalog.prioritize?.(items).catch(() => undefined);
    };
    /** The owners behind every model menu this host opens. */
    #context() {
        return {
            actions: this.#options.catalog,
            updates: this.#options.updates,
            feedback: this.#options.feedback,
            detectors: this.#options.detectors,
            outputs: this.#options.outputs,
            detectorDialog: (item) => new ModelDetectorDialog(this.#options.document, item, this.#options.detectors),
        };
    }
    /** Release mounted Vue apps, pending discovery, and temporary native-control annotations. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#menu?.dispose();
        this.#menu = null;
        this.#page?.dispose();
        this.#deactivate();
        this.#observer?.disconnect();
        this.#appearance.dispose();
        this.#unsubscribe?.();
        this.#options.document.removeEventListener('focusout', this.#nativeFocusOut);
    }
    /** Activate only under the supported renderer, keeping preference across renderer switches. */
    #reconcile() {
        if (this.#disposed)
            return;
        const active = this.#enabled &&
            (this.#page?.active ?? true) &&
            resolveComfyRendererMode(this.#options.app, null, this.#options.document) === 'vue';
        if (!active) {
            this.#deactivate();
            return;
        }
        if (this.#active) {
            this.#queue(this.#options.document.body);
            this.#schedule();
            return;
        }
        this.#active = true;
        this.#generation += 1;
        this.#appearance.activate();
        void this.#load(false);
    }
    /** Retain usable controls while retrying unavailable background catalog requests. */
    async #load(refresh) {
        const generation = this.#generation;
        try {
            const snapshot = await this.#options.catalog.load(refresh);
            if (!this.#active || this.#disposed || generation !== this.#generation)
                return;
            this.#loadFailures = 0;
            this.#snapshot = snapshot;
            for (const mount of this.#mounts.values())
                this.#publishChoices(mount, snapshot);
            this.#queue(this.#options.document.body);
            this.#schedule();
            if (!this.#unsubscribeCatalog)
                this.#unsubscribeCatalog = this.#options.catalog.watch((next) => {
                    const previous = this.#snapshot;
                    this.#snapshot = next;
                    for (const mount of this.#mounts.values())
                        this.#publishChoices(mount, next);
                    // Which widgets are model fields, and what they offer, follows the model files. New
                    // names, words and pictures for the same files reach the mounted pickers above and
                    // change nothing about the nodes, so the nodes are not read again.
                    if (previous && sameModelFiles(previous, next))
                        return;
                    this.#queue(this.#options.document.body);
                    this.#schedule();
                }, this.#options.reportError);
            if (snapshot.scanning)
                this.#reloadAfter(500);
        }
        catch (error) {
            if (!this.#active || this.#disposed || generation !== this.#generation)
                return;
            if (this.#loadFailures === 0)
                this.#options.reportError(error);
            this.#loadFailures += 1;
            this.#reloadAfter(Math.min(30_000, 1_000 * 2 ** Math.min(this.#loadFailures - 1, 5)));
        }
    }
    /** Schedule one lifecycle-owned retry without discarding a previous successful presentation. */
    #reloadAfter(delay) {
        if (this.#poll !== null)
            this.#options.document.defaultView?.clearTimeout(this.#poll);
        this.#poll =
            this.#options.document.defaultView?.setTimeout(() => {
                this.#poll = null;
                void this.#load(true);
            }, delay) ?? null;
    }
    /** Discover only affected native cards and newly attached projected-card mounts. */
    #queue(element) {
        const root = element.closest('.lg-node');
        if (root)
            this.#dirty.add(root);
        for (const card of element.querySelectorAll('.lg-node'))
            this.#dirty.add(card);
        if (element.matches('.lg-node'))
            this.#dirty.add(element);
    }
    /** Coalesce DOM discovery, preserving existing mounts and releasing removed controls. */
    #schedule() {
        if (this.#frame !== null || !this.#active || !this.#snapshot)
            return;
        this.#frame =
            this.#options.document.defaultView?.requestAnimationFrame(() => {
                this.#frame = null;
                if (!this.#active || !this.#snapshot)
                    return;
                try {
                    if (this.#resolutionPasses > 0)
                        for (const mount of [...this.#mounts.values()].sort((left, right) => Number(left.kind === 'loras') - Number(right.kind === 'loras')))
                            resolveCubeInstalledModel(this.#options.app, mount.binding, this.#snapshot);
                    this.#discovery?.reconcile();
                    this.#sessions.reconcile();
                    for (const [anchor, mount] of this.#mounts)
                        if (!anchor.isConnected || !mount.root.isConnected)
                            this.#release(anchor);
                    const nodes = new ComfyNodeVNodeIndex();
                    for (const root of this.#dirty) {
                        if (!root.isConnected)
                            continue;
                        const bindings = readComfyModelFields(root, this.#snapshot, nodes);
                        const live = new Set(bindings.map((binding) => binding.anchor));
                        for (const [anchor, mount] of this.#mounts)
                            if (mount.root === root && !live.has(anchor))
                                this.#release(anchor);
                        for (const binding of bindings) {
                            const compatibilityKey = JSON.stringify(comfyModelCompatibilityContext(this.#options.app, binding, this.#snapshot));
                            const current = this.#mounts.get(binding.anchor);
                            if (current &&
                                current.kind === binding.kind &&
                                current.compatibilityKey === compatibilityKey &&
                                sameOptions(current.options, binding.options) &&
                                (binding.options.length > 0 || current.scanning === this.#snapshot.scanning))
                                continue;
                            if (!binding.enhancementReady())
                                continue;
                            if (current)
                                this.#release(binding.anchor);
                            resolveCubeInstalledModel(this.#options.app, binding, this.#snapshot);
                            const container = this.#options.document.createElement('div');
                            container.className = 'sc-model-mount';
                            const releasePresentation = registerNativeFieldEnhancement(binding.anchor, container);
                            this.#appearance.track(container);
                            binding.anchor.after(container);
                            const choices = modelPickerChoices(this.#snapshot.items, binding.kind, binding.options, binding.optionLabel);
                            const picker = mountModelPicker(container, binding, choices, this.#options.catalog, this.#options.feedback, this.#options.updates, this.#options.detectors, this.#options.outputs, () => this.#snapshot
                                ? comfyModelCompatibilityContext(this.#options.app, binding, this.#snapshot)
                                : null, this.#sessions.get(root, binding));
                            const selected = binding.readValue();
                            const releaseDiscovery = this.#discovery?.bind(root, binding, (action) => picker.updateDiscovery(action)) ??
                                (() => undefined);
                            void this.#options.catalog
                                .prioritize([
                                ...choices.filter((item) => item.value === selected),
                                ...choices.filter((item) => item.value !== selected),
                            ])
                                .catch(this.#options.reportError);
                            this.#mounts.set(binding.anchor, {
                                binding,
                                inputName: binding.inputName ?? binding.label,
                                root,
                                anchor: binding.anchor,
                                container,
                                kind: binding.kind,
                                options: binding.options,
                                picker,
                                releaseDiscovery,
                                releasePresentation,
                                scanning: this.#snapshot.scanning,
                                compatibilityKey,
                            });
                            const mount = this.#mounts.get(binding.anchor);
                            if (mount)
                                this.#publishChoices(mount, this.#snapshot);
                        }
                    }
                    this.#dirty.clear();
                    if (this.#resolutionPasses > 0) {
                        this.#resolutionPasses -= 1;
                        this.#queue(this.#options.document.body);
                        this.#schedule();
                    }
                }
                catch (error) {
                    this.#options.reportError(error);
                    this.#deactivate();
                }
            }) ?? null;
    }
    /** Apply one shared resolution to still-live mounts without publishing stale asynchronous choices. */
    #publishChoices(mount, snapshot) {
        if (mount.kind !== 'ultralytics') {
            const choices = modelPickerChoices(snapshot.items, mount.kind, mount.options, mount.binding.optionLabel);
            // A publication that changed none of this field's models leaves the field as it is.
            const shown = this.#published.get(mount);
            if (shown?.length === choices.length && shown.every((item, index) => item === choices[index]))
                return;
            this.#published.set(mount, choices);
            mount.picker.update(choices);
            return;
        }
        void this.#options.detectors
            .resolve(mount.options, snapshot.revision)
            .then((items) => {
            if (this.#active &&
                this.#snapshot?.revision === snapshot.revision &&
                this.#mounts.get(mount.anchor) === mount)
                mount.picker.update(items);
        })
            .catch(this.#options.reportError);
    }
    /** Resolve checkpoint fields before addons, letting native Vue callbacks settle between passes. */
    #resolveCubeFields() {
        this.#resolutionPasses = 2;
        this.#queue(this.#options.document.body);
        this.#schedule();
    }
    /** Reveal native controls and release their presentation lifetime. */
    #release(anchor) {
        const mount = this.#mounts.get(anchor);
        if (!mount)
            return;
        mount.releaseDiscovery();
        mount.picker.dispose();
        this.#appearance.release(mount.container);
        mount.container.remove();
        mount.releasePresentation();
        this.#mounts.delete(anchor);
    }
    /** Cancel owned work and reveal current native host state on disable or renderer change. */
    #deactivate() {
        this.#discovery?.close();
        this.#sessions.clear();
        this.#generation += 1;
        this.#active = false;
        this.#unsubscribeCatalog?.();
        this.#unsubscribeCatalog = null;
        for (const anchor of this.#mounts.keys())
            this.#release(anchor);
        this.#dirty.clear();
        this.#appearance.deactivate();
        for (const popup of this.#popups)
            this.#appearance.release(popup);
        this.#popups.clear();
        if (this.#frame !== null)
            this.#options.document.defaultView?.cancelAnimationFrame(this.#frame);
        if (this.#poll !== null)
            this.#options.document.defaultView?.clearTimeout(this.#poll);
        this.#frame = null;
        this.#poll = null;
        this.#loadFailures = 0;
    }
    /** Defer discovery until focus has settled outside the native control. */
    #nativeFocusOut = (event) => {
        if (!this.#active || !(event.target instanceof HTMLElement))
            return;
        this.#queue(event.target);
        this.#schedule();
    };
}
/** Remount when the host changes its legal options, preserving their exact order. */
function sameOptions(left, right) {
    return left.length === right.length && left.every((value, index) => value === right[index]);
}
/** Whether two snapshots list the same model files in the same state of discovery. */
export function sameModelFiles(left, right) {
    return (left.scanning === right.scanning &&
        left.items.length === right.items.length &&
        left.items.every((item, index) => {
            const other = right.items[index];
            return (item === other ||
                (other !== undefined &&
                    item.id === other.id &&
                    item.kind === other.kind &&
                    item.value === other.value));
        }));
}

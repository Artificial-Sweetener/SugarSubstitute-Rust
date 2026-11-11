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
/** Present the complete Cube cart through the shared SugarCubes modal and captured application session. */
import { ModalShell } from '../dialogs/ModalShell.js';
import { mountCubeStackCart } from './vue/CubeStackCartRuntime.js';
import { ensureCubeStackCartStyles } from './CubeStackCartStyles.js';
/** Own one modal lifetime while the factory and session retain graph, catalog and draft policy. */
export class CubeStackCartHost {
    #options;
    #shell;
    #capture = null;
    #mounted = null;
    #reset = null;
    #refreshButton = null;
    #refreshing = false;
    #releaseSession = null;
    #releaseAppearance = null;
    #generation = 0;
    #open = false;
    /** Bind accepted appearance and the existing modal's focus, dismissal and feedback owners. */
    constructor(options) {
        this.#options = options;
        this.#shell = new ModalShell({
            adapter: {
                getDocument: () => options.document,
                getWindow: () => options.document.defaultView,
            },
            appearanceSource: options.appearance,
            dialogClassName: 'sc-cube-stack-cart-dialog',
        });
        ensureCubeStackCartStyles(options.document);
    }
    /** Open the full existing stack and optionally stage one removal for explicit cart review. */
    async open(removeInstanceId = null) {
        if (this.#open)
            return;
        this.#open = true;
        const generation = ++this.#generation;
        const result = this.#shell.open({
            title: 'Add cubes',
            body: 'Loading cubes…',
            confirmLabel: 'Apply',
            cancelLabel: 'Cancel',
            onConfirm: () => {
                void this.#apply();
            },
        });
        this.#shell.setConfirmEnabled(false);
        let closed = false;
        const finished = result.then(() => {
            closed = true;
            if (generation === this.#generation)
                this.#release();
        });
        try {
            const capture = await this.#options.factory.open();
            if (closed || generation !== this.#generation) {
                capture.session.dispose();
                return;
            }
            this.#capture = capture;
            if (removeInstanceId)
                capture.session.remove(`existing:${removeInstanceId}`);
            const root = this.#options.document.createElement('div');
            root.style.cssText = 'display:flex; flex:1; min-height:0;';
            this.#shell.setBody(root);
            this.#mounted = mountCubeStackCart(root, this.#state(capture), {
                add: (key, before = null) => this.#edit(() => capture.session.add(key, before)),
                remove: (id) => this.#edit(() => capture.session.remove(id)),
                reorder: (ids) => this.#edit(() => capture.session.reorder(ids)),
            });
            const reset = this.#options.document.createElement('button');
            reset.type = 'button';
            reset.textContent = 'Reset';
            reset.className =
                'p-button p-component p-button-text p-button-secondary sc-cube-stack-cart-reset';
            reset.addEventListener('click', () => this.#edit(() => capture.session.reset()));
            this.#shell.elements.actions?.prepend(reset);
            this.#reset = reset;
            const refresh = this.#options.document.createElement('button');
            refresh.type = 'button';
            refresh.textContent = 'Refresh';
            refresh.className = 'p-button p-component p-button-text sc-cube-stack-cart-refresh';
            refresh.addEventListener('click', () => {
                void this.#refreshLibrary();
            });
            this.#shell.elements.closeButton?.before(refresh);
            this.#refreshButton = refresh;
            this.#releaseSession = capture.session.subscribe(() => this.#refresh());
            this.#releaseAppearance = this.#options.appearance.subscribeAppearance(() => this.#refresh());
            this.#refresh();
            this.#mounted.focusSearch();
            await finished;
        }
        catch (error) {
            this.#options.reportError(error);
            if (generation === this.#generation && !closed) {
                this.#shell.setBody('The Cube cart could not be opened.');
                this.#shell.setError(message(error));
                this.#shell.setConfirmEnabled(false);
                await finished;
            }
        }
    }
    /** Retire delayed opening/planning responses before workflow or renderer teardown. */
    close() {
        this.#generation += 1;
        this.#shell.close(null);
        this.#release();
    }
    /** Refresh only the mounted session and accepted appearance, preserving search and stable row controls. */
    #refresh() {
        if (!this.#capture)
            return;
        this.#mounted?.update(this.#state(this.#capture));
        this.#shell.setConfirmEnabled(this.#capture.session.changed && !this.#refreshing);
        this.#shell.setBusy(this.#capture.session.pending);
        if (this.#reset)
            this.#reset.disabled =
                this.#capture.session.pending || this.#refreshing || !this.#capture.session.changed;
        if (this.#refreshButton)
            this.#refreshButton.disabled = this.#refreshing || this.#capture.session.pending;
    }
    /** Project immutable captured rows onto independent stable draft identities. */
    #state(capture) {
        const rows = capture.session.entries.map((entry) => {
            const original = entry.source === 'existing'
                ? capture.existingRows.get(entry.instanceId)
                : capture.artifactRows.get(entry.catalogKey);
            if (!original)
                throw new Error('Cube cart row lost its captured identity.');
            return { ...original, id: entry.draftId };
        });
        return {
            library: capture.library,
            rows,
            pending: capture.session.pending || this.#refreshing,
            appearance: this.#options.appearance.appearance.style,
        };
    }
    /** Surface synchronous draft failures through SugarCubes feedback without native browser dialogs. */
    #edit(edit) {
        if (this.#refreshing)
            return;
        try {
            edit();
            this.#shell.setError('');
        }
        catch (error) {
            this.#shell.setError(message(error));
            this.#options.reportError(error);
        }
    }
    /** Refresh the current library without retiring staged artifacts, search or Cancel. */
    async #refreshLibrary() {
        const capture = this.#capture;
        if (!capture || this.#refreshing || capture.session.pending)
            return;
        this.#refreshing = true;
        this.#shell.setError('');
        this.#refresh();
        try {
            await capture.refreshLibrary();
        }
        catch (error) {
            if (this.#capture === capture) {
                this.#shell.setError(message(error));
                this.#options.reportError(error);
            }
        }
        finally {
            if (this.#capture === capture) {
                this.#refreshing = false;
                this.#refresh();
            }
        }
    }
    /** Keep errors in the edited cart and close only after one accepted native application. */
    async #apply() {
        const capture = this.#capture;
        if (!capture || this.#refreshing || capture.session.pending)
            return;
        this.#shell.setError('');
        try {
            if (await capture.session.apply()) {
                this.#shell.close(true);
                await this.#options.onApplied();
            }
        }
        catch (error) {
            if (this.#capture === capture)
                this.#shell.setError(message(error));
            this.#options.reportError(error);
        }
    }
    /** Release every mounted control and subscription with its owning modal lifetime. */
    #release() {
        this.#releaseSession?.();
        this.#releaseSession = null;
        this.#releaseAppearance?.();
        this.#releaseAppearance = null;
        this.#capture?.session.dispose();
        this.#capture = null;
        this.#mounted?.dispose();
        this.#mounted = null;
        this.#reset?.remove();
        this.#reset = null;
        this.#refreshButton?.remove();
        this.#refreshButton = null;
        this.#refreshing = false;
        this.#open = false;
    }
}
/** Preserve an actionable failure message without rendering dynamic markup. */
function message(error) {
    return error instanceof Error ? error.message : String(error);
}

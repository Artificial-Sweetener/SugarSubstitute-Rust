import { canBrowseModelVersions, modelContextEntries } from './ModelContextMenu.js';
import { modelOutputThumbnailMenu } from './ModelOutputThumbnailMenu.js';
import { projectModelContext } from './ModelContextProjection.js';
/**
 * One model menu for the picker field, every wall card, the update badge and the prompt's LoRA
 * banner: the same rows, the same commands, whichever surface opened it.
 */
export class ModelMenu {
    options;
    session;
    disabled;
    beforeFamily;
    #proposals;
    #unsubscribe;
    #listeners = new Set();
    #controller = null;
    #dialog = null;
    #disposed = false;
    /** Bind the owners, the session that applies a chosen version, and the caller's hooks. */
    constructor(options, session, disabled = () => false, beforeFamily = () => undefined) {
        this.options = options;
        this.session = session;
        this.disabled = disabled;
        this.beforeFamily = beforeFamily;
        this.#proposals = options.updates?.read().proposals ?? [];
        this.#unsubscribe = options.updates?.subscribe((snapshot) => {
            this.#proposals = snapshot.proposals;
            for (const listener of this.#listeners)
                listener();
        });
    }
    /** The current update offers, in catalog order. */
    get proposals() {
        return this.#proposals;
    }
    /** Observe changed update offers. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Match only authoritative catalog identities rather than loosely matching display labels. */
    offer(item) {
        return this.#proposals.find((proposal) => proposal.modelId === item.id);
    }
    /** The rows about `item`; the update badge's own menu holds only its two update decisions. */
    entries(item, updateOnly = false) {
        const proposal = this.offer(item);
        const { actions, detectors, outputs } = this.options;
        return updateOnly && proposal
            ? [
                { label: 'Dismiss', enabled: true, command: 'dismiss', url: null },
                {
                    label: `Don't check for updates for ${proposal.modelName}`,
                    enabled: true,
                    command: 'disable-updates',
                    url: null,
                },
            ]
            : modelContextEntries(item, item.kind === 'ultralytics'
                ? !!detectors && !!this.options.detectorDialog
                : !!actions && /^[a-f0-9]{64}$/.test(item.id), !!proposal, false).map((entry) => entry.command === 'output-thumbnail' && outputs
                ? modelOutputThumbnailMenu(outputs.choices(), outputs.activeChoice())
                : entry);
    }
    /** The rows about `item` as shared menu entries, for menus that are not the picker's own. */
    project(item, updateOnly = false) {
        return projectModelContext(this.entries(item, updateOnly), (entry) => this.command(item, entry));
    }
    /** Browse a verified version family independently of update notification state. */
    family(item) {
        const { actions, feedback, updates } = this.options;
        if (!actions || !this.session || !canBrowseModelVersions(item, !!this.offer(item)))
            return;
        this.beforeFamily();
        this.#dialog?.close();
        void this.session
            .versions(item, actions, (severity, message) => feedback?.(severity, message))
            .finally(() => {
            void updates?.load().catch((error) => {
                feedback?.('error', error instanceof Error ? error.message : 'Model updates are unavailable.');
            });
        });
    }
    /** Execute one row through its authoritative owner and report how it ended. */
    command(item, entry) {
        const { actions, outputs, updates, feedback, detectorDialog } = this.options;
        if (entry.command === 'detector-thumbnail' && detectorDialog) {
            this.beforeFamily();
            this.#dialog?.close();
            const current = detectorDialog(item);
            this.#dialog = current;
            void current.open().finally(() => {
                if (this.#dialog === current)
                    this.#dialog = null;
            });
            return;
        }
        if (entry.command === 'versions') {
            this.family(item);
            return;
        }
        let status = '';
        void this.#run(async (signal) => {
            if (entry.command === 'refresh')
                await actions?.refreshMetadata(item.id, (message) => {
                    status = message;
                }, signal);
            else if (entry.command === 'output-thumbnail' && entry.imageId && outputs && actions) {
                const image = await outputs.image(entry.imageId, signal);
                if (signal.aborted)
                    return;
                await actions.replaceThumbnail(item.id, image, signal);
                if (!signal.aborted)
                    status = 'Model thumbnail updated.';
            }
            else if (entry.command === 'dismiss')
                await updates?.dismiss(item.id);
            else if (entry.command === 'disable-updates')
                await updates?.disableModel(item.id);
        }).then((error) => {
            if (this.#disposed)
                return;
            if (error)
                feedback?.('error', error);
            else if (status)
                feedback?.('info', status);
        });
    }
    /** Release the update subscription, a running command and an open detector dialog. */
    dispose() {
        this.#disposed = true;
        this.#unsubscribe?.();
        this.#listeners.clear();
        this.#controller?.abort();
        this.#dialog?.close();
    }
    /** Run one command at a time; resolve with its failure message, or null. */
    async #run(operation) {
        if (this.#controller || this.disabled())
            return null;
        const current = new AbortController();
        this.#controller = current;
        try {
            await operation(current.signal);
            return null;
        }
        catch (failure) {
            if (current.signal.aborted)
                return null;
            console.error('SugarCubes model action failed.', failure);
            return failure instanceof Error ? failure.message : 'The model action failed.';
        }
        finally {
            this.#controller = null;
        }
    }
}

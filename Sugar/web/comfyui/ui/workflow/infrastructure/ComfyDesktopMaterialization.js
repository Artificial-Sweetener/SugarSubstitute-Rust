import { restoreDesktopResults } from '../../execution/application/WorkflowDesktopResultImport.js';
import { decodeComfyWorkflowSources } from '../../execution/infrastructure/ComfyWorkflowSourceDecoder.js';
import { DesktopSessionRefusal, } from '../domain/DesktopSession.js';
/** Keep fresh binding proposal, metadata reads, restore-only publication and acknowledgement ordering together. */
export class ComfyDesktopMaterialization {
    options;
    #phase = null;
    #cancellation = null;
    #restoringSettings = false;
    constructor(options) {
        this.options = options;
    }
    active() {
        return this.#phase !== null;
    }
    resolving() {
        return this.#phase === 'outputs';
    }
    restoringSettings() {
        return this.#restoringSettings;
    }
    cancel() {
        this.#cancellation?.cancel();
    }
    /**
     * Bind host-adopted durable entries to the tabs Comfy itself restored, then import references
     * and view intent. Comfy owns the tabs: an entry whose tab Comfy did not reopen is left out, and
     * the returned bindings name only the entries that were bound.
     */
    async materialize(checkpoint, cancellation) {
        this.#phase = 'documents';
        this.#cancellation = cancellation;
        this.options.bindings.changed();
        try {
            const contexts = this.options.documents.match(checkpoint.entries, () => !cancellation.cancelled() && !this.options.retired());
            const entries = checkpoint.entries.filter((entry) => contexts.has(entry.id));
            this.options.assertCurrent(cancellation);
            this.options.bindings.synchronize(this.options.execution.sessions.read());
            for (const entry of entries) {
                const context = contexts.get(entry.id);
                if (!context)
                    throw new DesktopSessionRefusal('stale', 'Restored Comfy workflow binding was retired.');
                this.options.bindings.bind({
                    entryId: entry.id,
                    runtimeSession: context.sessionId,
                    generation: context.generation,
                    graphRevision: entry.document.graphRevision,
                    sourceRevision: entry.sourceRevision,
                    navigationRevision: entry.navigationRevision,
                });
                this.options.identities.restoreSources(entry.id, entry.sources);
                this.options.identities.restoreResults(entry.id, entry.submissions, entry.outputs);
                this.options.submissions.restore(entry.id, entry.submissions);
                this.options.input.set(entry.id, entry.input);
            }
            this.options.assets.clear();
            for (const asset of checkpoint.assets)
                this.options.assets.set(asset.digest, asset);
            this.#phase = 'outputs';
            await this.options.publishObservation();
            this.options.assertCurrent(cancellation);
            for (const entry of entries) {
                this.options.assertCurrent(cancellation);
                const context = contexts.get(entry.id);
                if (!context)
                    throw new DesktopSessionRefusal('stale', 'Restored occurrence is unavailable.');
                const binding = this.options.bindings.binding(context.sessionId);
                const restored = await restoreDesktopResults(this.options.artifacts, entry, binding, context, checkpoint.id, cancellation);
                this.options.assertCurrent(cancellation);
                if (!this.options.execution.results.restoreSession(context, restored))
                    throw new DesktopSessionRefusal('stale', 'A genuine live run superseded desktop materialization.');
                this.options.execution.restoreSubmissionOrder(context.sessionId, restored.runOrder);
                this.options.presentation.restore(context.sessionId, {
                    canvas: entry.view.canvasEnabled,
                    masonry: entry.view.masonryEnabled,
                    editorWidth: entry.view.editorWidth,
                });
                this.options.canvas.restore(context, entry, decodeComfyWorkflowSources(JSON.parse(entry.document.graphJson)));
            }
            const shown = this.options.execution.sessions.read().active?.sessionId;
            const active = entries.find((entry) => contexts.get(entry.id)?.sessionId === shown);
            if (active)
                this.options.route?.choose(active.view.route);
            if (checkpoint.route === 'settings') {
                if (!this.options.settingsRoute)
                    throw new DesktopSessionRefusal('unavailable', 'Settings workspace route is unavailable.');
                this.#restoringSettings = true;
                const controller = new AbortController();
                const stop = cancellation.subscribe(() => controller.abort());
                try {
                    await this.options.settingsRoute.show(controller.signal);
                }
                finally {
                    stop();
                    this.#restoringSettings = false;
                }
                this.options.assertCurrent(cancellation);
            }
            this.#phase = null;
            this.options.refresh(true);
            const restored = new Set(entries.map((entry) => entry.id));
            return this.options.bindings
                .fence()
                .bindings.filter((binding) => restored.has(binding.entryId));
        }
        catch (error) {
            if (error instanceof DesktopSessionRefusal && error.code === 'stale')
                this.options.superseded();
            throw error;
        }
        finally {
            this.#phase = null;
            this.#cancellation = null;
            this.options.refresh(false);
            this.options.bindings.changed();
        }
    }
}

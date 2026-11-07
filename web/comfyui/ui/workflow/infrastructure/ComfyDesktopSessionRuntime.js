import { decodeComfyWorkflowSources } from '../../execution/infrastructure/ComfyWorkflowSourceDecoder.js';
import { WorkflowOutputIdentities } from '../../execution/application/WorkflowOutputIdentities.js';
import { WorkflowSubmissionSnapshots } from '../../execution/application/WorkflowSubmissionSnapshots.js';
import { WorkflowDesktopOutputs } from '../../execution/application/WorkflowDesktopOutputs.js';
import { NativeSubgraphChangeObserver } from '../../surface/NativeSubgraphChangeObserver.js';
import { isRecord } from '../../types/common.js';
import { DesktopWorkspaceBindings } from '../application/DesktopWorkspaceBindings.js';
import { DesktopStartupIntent } from '../application/DesktopStartupIntent.js';
import { DesktopSessionRefusal, } from '../domain/DesktopSession.js';
import { ComfyDesktopDocuments } from './ComfyDesktopDocuments.js';
import { ComfyDesktopReadiness } from './ComfyDesktopReadiness.js';
import { ComfyDesktopCapture } from './ComfyDesktopCapture.js';
import { ComfyDesktopMaterialization } from './ComfyDesktopMaterialization.js';
/** Keep desktop capture independent of graph generation, history routing and editor insertion. */
export class ComfyDesktopSessionRuntime {
    options;
    bindings;
    #documents;
    #readiness;
    #identities;
    #submissions = new WorkflowSubmissionSnapshots();
    #outputs;
    #input = new Map();
    #assets = new Map();
    #graphViews = new Map();
    #capture;
    #materialization;
    #stops = [];
    #graph = null;
    #acknowledged = null;
    #refreshing = false;
    #disposed = false;
    #available = true;
    #intent = new DesktopStartupIntent();
    #publishObservation = null;
    constructor(options) {
        this.options = options;
        this.bindings = new DesktopWorkspaceBindings(options.webview, options.nextId);
        this.#identities = new WorkflowOutputIdentities(options.nextId);
        this.#outputs = new WorkflowDesktopOutputs(this.#identities, this.#submissions, options.artifacts);
        this.#documents = new ComfyDesktopDocuments({
            app: options.app,
            sessions: options.execution.sessions,
            adapter: options.execution.workflowDocuments,
        });
        this.#readiness = new ComfyDesktopReadiness(options.app, options.execution.sessions);
        this.#capture = new ComfyDesktopCapture({
            ...options,
            documents: this.#documents,
            bindings: this.bindings,
            outputs: this.#outputs,
            identities: this.#identities,
            submissions: this.#submissions,
            input: this.#input,
            assets: this.#assets,
            previousId: () => this.#acknowledged?.id ?? null,
            assertCurrent: (cancellation) => this.#assertCurrent(cancellation),
        });
        this.#materialization = new ComfyDesktopMaterialization({
            ...options,
            documents: this.#documents,
            bindings: this.bindings,
            identities: this.#identities,
            submissions: this.#submissions,
            input: this.#input,
            assets: this.#assets,
            publishObservation: async () => {
                await this.#publishObservation?.();
            },
            refresh: (prime) => this.#refresh(true, prime),
            assertCurrent: (cancellation) => this.#assertCurrent(cancellation),
            retired: () => this.#disposed,
            superseded: () => {
                this.#intent.authored();
            },
        });
    }
    /** Register before Comfy finishes initial tab restoration and return without awaiting native recovery. */
    install() {
        if (this.#disposed || this.#stops.length)
            return;
        const { execution, canvas, presentation, route } = this.options;
        this.#documents.install();
        this.#readiness.install();
        if (this.#readiness.read().committed)
            this.#intent.committed(this.#contextSignature());
        this.#stops.push(execution.sessions.subscribe(() => {
            if (this.#materialization.resolving()) {
                this.#intent.authored();
                this.#materialization.cancel();
            }
            this.#intent.workflow(this.#contextSignature(), execution.sessions.read().loading, this.#materialization.active());
            this.#refresh(true);
        }), execution.results.subscribe(() => this.#refresh(false)), execution.subscribeGeneration((event) => this.#generation(event)), execution.bindDocumentRevision((session) => {
            this.#refresh(true);
            return this.bindings.binding(session).graphRevision;
        }), this.#readiness.subscribe(() => {
            if (this.#readiness.read().committed)
                this.#intent.committed(this.#contextSignature());
            this.#refresh(true);
            this.bindings.changed();
        }), canvas.subscribe(() => this.#refresh(false)), presentation.subscribe(() => this.#refresh(false)));
        if (route)
            this.#stops.push(route.subscribe(() => this.#refresh(true)));
        if (this.options.settingsRoute)
            this.#stops.push(this.options.settingsRoute.subscribe(() => {
                if (!this.#materialization.restoringSettings() &&
                    this.options.settingsRoute?.read() === 'settings') {
                    this.#intent.settingsOpened();
                    this.#materialization.cancel();
                }
                this.bindings.changed();
            }));
        const root = isRecord(this.options.app) ? this.options.app.rootGraph : null;
        if (!isRecord(root) || typeof root.add !== 'function' || typeof root.remove !== 'function')
            throw new TypeError('Comfy graph change observation is unavailable.');
        this.#graph = new NativeSubgraphChangeObserver(root, () => {
            const changed = this.#intent.authored();
            this.#refresh(true);
            if (changed)
                this.bindings.changed();
        }, () => this.#readiness.read().committed &&
            !execution.sessions.read().loading &&
            !this.#materialization.active());
        this.#refresh(true);
    }
    fence() {
        return this.bindings.fence();
    }
    readiness() {
        const state = this.#readiness.read();
        const sessions = this.options.execution.sessions.read();
        return {
            committed: state.committed &&
                !sessions.loading &&
                sessions.active !== null &&
                !this.#materialization.active() &&
                this.#intent.captureAllowed() &&
                this.#available &&
                !this.#disposed,
            workflowPersistence: state.workflowPersistence,
        };
    }
    subscribe(listener) {
        return this.bindings.subscribe(listener);
    }
    /** Bind the protocol client's bounded native admission to the current page lifetime. */
    bindObservationDelivery(publish) {
        if (this.#publishObservation)
            throw new Error('Desktop observation delivery is already bound.');
        this.#publishObservation = publish;
        return () => {
            if (this.#publishObservation === publish)
                this.#publishObservation = null;
        };
    }
    /** Capture only a committed workspace; the protocol client fences later external completion. */
    async capture(id, revision, cancellation) {
        if (!this.readiness().committed)
            throw new DesktopSessionRefusal('not-committed', 'Comfy desktop capture requires committed startup.');
        this.#refresh(true);
        return this.#capture.capture(id, revision, cancellation);
    }
    /** Admit materialization only after actual Comfy startup and explicit persistence consent. */
    async materialize(checkpoint, cancellation) {
        if (!this.readiness().committed)
            throw new DesktopSessionRefusal('not-committed', 'Comfy desktop materialization requires committed startup.');
        if (!checkpoint.entries.length)
            throw new DesktopSessionRefusal('unsupported', 'Empty desktop checkpoint seeds from the committed Comfy workspace.');
        if (!this.#intent.restoreAllowed())
            throw new DesktopSessionRefusal('stale', 'Explicit workflow intent superseded startup restoration.');
        return this.#materialization.materialize(checkpoint, cancellation);
    }
    /** Advance only after the host confirms durable activation; page-local capture never grants durability. */
    acknowledged(id, revision) {
        if (!this.#acknowledged || revision > this.#acknowledged.revision) {
            this.#acknowledged = { id, revision };
            this.#intent.acknowledged();
        }
    }
    /** Retire owned observations and delayed requests before the page releases its actual graph owners. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#materialization.cancel();
        for (const stop of this.#stops.splice(0))
            stop();
        this.#graph?.dispose();
        this.#graph = null;
        this.#documents.dispose();
        this.#readiness.dispose();
        this.#submissions.dispose();
        this.#identities.dispose();
        this.bindings.dispose();
        this.#input.clear();
        this.#assets.clear();
        this.#graphViews.clear();
    }
    /** Record accepted ownership before early final replay; restored receipts never enter this execution path. */
    #generation(event) {
        if (event.type !== 'accepted' || this.#disposed)
            return;
        this.#intent.authored();
        if (this.#materialization.active())
            this.#materialization.cancel();
        this.#refresh(true);
        const binding = this.bindings.binding(event.run.session.sessionId);
        const captured = {
            ...binding,
            graphRevision: event.submission.graphRevision ?? binding.graphRevision,
        };
        const member = event.run.scene;
        const scene = member?.run.scenes.find((scene) => scene.key === member.key);
        this.#submissions.accept({
            entryId: binding.entryId,
            backend: this.options.scope.backend,
            promptId: event.run.promptId.slice('prompt:'.length),
            runId: this.#identities.run(binding.entryId, event.run.promptId.slice('prompt:'.length)),
            runOrder: event.run.order,
            graphRevision: captured.graphRevision,
            sources: this.#identities.sources(captured, event.submission.sources),
            scene: member && scene
                ? {
                    groupId: this.#identities.scene(binding.entryId, member.run.id, null),
                    sceneId: this.#identities.scene(binding.entryId, member.run.id, member.key),
                    title: scene.title,
                    order: scene.order,
                    count: member.run.scenes.length,
                }
                : null,
        });
        this.bindings.changed();
    }
    /** Advance semantic observation revisions from authoritative snapshots, never from preview transport frames. */
    #refresh(graphs, prime = false) {
        if (this.#disposed || this.#materialization.active() || this.#refreshing)
            return;
        this.#refreshing = true;
        try {
            const snapshot = this.options.execution.sessions.read();
            if (prime)
                this.#intent.restored(this.#contextSignature());
            this.bindings.synchronize(snapshot);
            const bindings = this.bindings.fence().bindings;
            for (const session of this.#graphViews.keys())
                if (!snapshot.open.some((context) => context.sessionId === session))
                    this.#graphViews.delete(session);
            this.#submissions.synchronize(bindings);
            this.#identities.retain(bindings.map((binding) => binding.entryId));
            for (const id of this.#input.keys())
                if (!bindings.some((binding) => binding.entryId === id))
                    this.#input.delete(id);
            if (!this.#readiness.read().committed || snapshot.loading)
                return;
            const observe = (session, owner, signature) => {
                if (prime)
                    this.bindings.prime(session, owner, signature);
                else
                    this.bindings.observe(session, owner, signature);
            };
            for (const context of snapshot.open) {
                if (graphs) {
                    const document = this.#documents.capture(context, this.bindings.binding(context.sessionId).graphRevision);
                    const { graphRevision: _revision, view: _view, ...authored } = document;
                    this.#graphViews.set(context.sessionId, document.view);
                    observe(context.sessionId, 'graph', JSON.stringify(authored));
                    this.#identities.sources(this.bindings.binding(context.sessionId), decodeComfyWorkflowSources(JSON.parse(document.graphJson)));
                }
                const result = this.options.execution.results.read(context.sessionId);
                if (result)
                    this.#outputs.retain(this.bindings.binding(context.sessionId), result, new Set(this.options.execution
                        .outstandingRuns()
                        .filter((run) => run.session.sessionId === context.sessionId &&
                        run.session.generation === context.generation)
                        .map((run) => run.promptId.slice('prompt:'.length))));
                observe(context.sessionId, 'source', JSON.stringify([
                    result?.outputs,
                    result?.scenes?.id,
                    result?.scenes?.scenes,
                    this.#submissions.read(this.bindings.binding(context.sessionId).entryId),
                ]));
                observe(context.sessionId, 'navigation', JSON.stringify([
                    this.options.canvas.signature(context),
                    this.options.presentation.capture(context.sessionId),
                    this.#graphViews.get(context.sessionId),
                ]));
            }
            if (!this.#available) {
                this.#available = true;
                this.bindings.changed();
            }
        }
        catch (error) {
            if (this.#available) {
                this.#available = false;
                this.bindings.changed();
            }
            this.options.report(error);
        }
        finally {
            this.#refreshing = false;
        }
    }
    #assertCurrent(cancellation) {
        if (this.#disposed || cancellation.cancelled())
            throw new DesktopSessionRefusal('cancelled', 'Desktop session operation was retired.');
    }
    /** Distinguish later explicit workflow activation/replacement from a settings-only startup interruption. */
    #contextSignature() {
        const snapshot = this.options.execution.sessions.read();
        return JSON.stringify([
            snapshot.active?.sessionId,
            snapshot.open.map((context) => [
                context.sessionId,
                context.generation,
                context.path,
                context.title,
            ]),
        ]);
    }
}

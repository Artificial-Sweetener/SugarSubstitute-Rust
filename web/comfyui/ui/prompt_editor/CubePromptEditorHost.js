import { PromptWidgetBinding } from './PromptWidgetBinding.js';
import { ComfyCanvasEditorKeyBoundary } from './ComfyCanvasEditorKeyBoundary.js';
import { preparePromptLoraCatalog } from './PromptLoraCatalog.js';
import { registerPromptEditorSurfaceOwner } from './PromptEditorSurface.js';
import { setCubePromptWidgetHeight, clearCubePromptWidgetHeight, } from '../surface/CubePromptWidgetHeightStore.js';
/** Share native documents while renderer mounts and workflow ownership remain explicit. */
export class CubePromptEditorHost {
    appearance;
    #document;
    #catalog;
    #report;
    #sessions = new Map();
    #retired = new WeakSet();
    #configuring = false;
    #restoreSurfaces = new Map();
    #release = null;
    #regions = null;
    #releaseRegions = null;
    #catalogSnapshot = null;
    #loraCatalog = null;
    #stopCatalog = null;
    #canvasKeys = null;
    #sceneQueue = null;
    /** Inject existing model catalog and feedback ownership. */
    constructor(document, catalog, report, appearance) {
        this.appearance = appearance;
        this.#document = document;
        this.#catalog = catalog;
        this.#report = report;
    }
    /** Enable prompt enhancement, regional links and the canvas key boundary before Cube fields render. */
    setup(regions = null, canvas = null) {
        this.#release ??= registerPromptEditorSurfaceOwner(this.#document, this);
        const scenes = canvas?.scenes;
        this.#sceneQueue ??= scenes ? (scene) => scenes.queueScene(scene) : null;
        if (canvas && !this.#canvasKeys) {
            this.#canvasKeys = new ComfyCanvasEditorKeyBoundary(canvas.app, this.#document, (error) => canvas.logger.error('[SugarCubes] Prompt editor canvas key boundary unavailable.', {
                error,
            }));
            this.#canvasKeys.install();
        }
        if (regions && !this.#regions) {
            this.#regions = regions;
            const releaseHover = regions.connectPrompts(this);
            const releaseAccent = regions.accent.subscribe(() => {
                const accent = regions.accent.current();
                for (const session of this.#sessions.values())
                    session.editor?.setRegionAccent(accent);
            });
            this.#releaseRegions = () => {
                releaseHover();
                releaseAccent();
            };
        }
    }
    /** Acquire a field presentation without duplicating its native editor or serialized value. */
    mount(surface) {
        if (this.#retired.has(surface.widget)) {
            // Cards of a replaced workflow can re-render during their deferred teardown; keep them
            // native. A widget whose node is still in its graph once configuration ends was retained.
            if (this.#configuring || !belongsToGraph(surface.node))
                return null;
            this.#retired.delete(surface.widget);
        }
        const hidden = surface.textarea.hidden;
        const display = surface.textarea.style.getPropertyValue('display');
        const priority = surface.textarea.style.getPropertyPriority('display');
        const restore = () => {
            if (typeof surface.widget.value === 'string' &&
                surface.textarea.value !== surface.widget.value)
                surface.textarea.value = surface.widget.value;
            surface.textarea.hidden = hidden;
            if (display)
                surface.textarea.style.setProperty('display', display, priority);
            else
                surface.textarea.style.removeProperty('display');
        };
        this.#restoreSurfaces.set(surface, restore);
        let session = this.#sessions.get(surface.widget);
        if (!session) {
            const created = {
                node: surface.node,
                binding: new PromptWidgetBinding(surface.node, surface.widget, (source) => {
                    created.editor?.refresh(source, this.#readOnly(created.active));
                }),
                surfaces: new Set(),
                editor: null,
                active: surface,
                disposed: false,
                creating: false,
                creation: Promise.resolve(),
                creationError: null,
            };
            session = created;
            this.#sessions.set(surface.widget, session);
        }
        session.surfaces.add(surface);
        if (!session.active || availablePromptSurface(surface))
            session.active = surface;
        if (!session.editor && !session.creating) {
            session.creating = true;
            session.creation = this.#create(session, surface)
                .catch((error) => {
                session.creationError = error;
                this.#report(error);
            })
                .finally(() => {
                session.creating = false;
            });
        }
        if (session.active)
            this.#present(session, session.active);
        return () => {
            if (session.disposed)
                return;
            session.surfaces.delete(surface);
            this.#restoreSurfaces.get(surface)?.();
            this.#restoreSurfaces.delete(surface);
            if (!session.surfaces.size && !belongsToGraph(surface.node)) {
                this.#retire(surface.widget, session);
                return;
            }
            if (session.active === surface) {
                const remaining = [...session.surfaces];
                session.active =
                    remaining.filter(availablePromptSurface).at(-1) ?? remaining.at(-1) ?? null;
                if (session.active)
                    this.#present(session, session.active);
                else
                    session.editor?.element.remove();
            }
        };
    }
    /** Refresh retained sessions and report workflow retirement to their renderer owner. */
    refresh(surface) {
        const session = this.#sessions.get(surface.widget);
        if (!session || !session.surfaces.has(surface))
            return false;
        session.editor?.refresh(session.binding.read(), this.#readOnly(session.active));
        if (session.active === surface)
            this.#present(session, surface);
        return true;
    }
    /** Complete the active prompt components belonging to one identified presentation. */
    async ready(root) {
        await Promise.all([...this.#sessions.values()]
            .filter((session) => [...session.surfaces].some((surface) => root.contains(surface.textarea)))
            .map(async (session) => {
            await session.creation;
            if (session.disposed)
                return;
            if (!session.editor)
                throw new Error('Prompt component construction failed.', {
                    cause: session.creationError,
                });
            await session.editor.whenPresented();
            if (!session.disposed && root.isConnected && !root.contains(session.editor.element))
                throw new Error('The prompt component is outside its owning presentation.');
        }));
    }
    /** Show a region hovered in another view on the matching separator of these prompts' editors. */
    highlightRegions(prompts, ordinal) {
        for (const session of this.#sessions.values())
            if (prompts.has(session.node))
                session.editor?.highlightRegion(ordinal);
    }
    /** Retire native documents before replacing an entire workflow; the model catalog is app-wide. */
    beforeConfigureGraph() {
        this.#configuring = true;
        for (const [widget, session] of [...this.#sessions]) {
            this.#retired.add(widget);
            this.#retire(widget, session);
        }
    }
    /** End the configuration window; retained prompts can present their editor again. */
    afterConfigureGraph() {
        this.#configuring = false;
    }
    /** Release the document registration and every editor-owned resource. */
    dispose() {
        this.#release?.();
        this.#release = null;
        this.#canvasKeys?.dispose();
        this.#canvasKeys = null;
        this.#releaseRegions?.();
        this.#releaseRegions = null;
        this.#regions = null;
        this.#stopCatalog?.();
        this.#stopCatalog = null;
        this.beforeConfigureGraph();
    }
    /** Release one session's editor, value observation, measured height and borrowed surfaces. */
    #retire(widget, session) {
        session.disposed = true;
        session.editor?.dispose();
        session.editor = null;
        session.active = null;
        session.binding.dispose();
        clearCubePromptWidgetHeight(widget);
        for (const surface of session.surfaces) {
            this.#restoreSurfaces.get(surface)?.();
            this.#restoreSurfaces.delete(surface);
        }
        session.surfaces.clear();
        if (this.#sessions.get(widget) === session)
            this.#sessions.delete(widget);
    }
    /** Load the bundled engine once and adapt the current shared model snapshot outside input. */
    async #create(session, surface) {
        const { SugarPromptEditor } = await import('./runtime/SugarPromptRuntime.js');
        if (session.disposed || !session.active)
            return;
        session.editor = new SugarPromptEditor(this.#document, {
            anchor: session.active.textarea,
            source: session.binding.read(),
            label: surface.node.title ?? 'Cube prompt',
            readOnly: this.#readOnly(session.active),
            commit: (source) => session.binding.commit(source),
            layout: (height) => {
                if (setCubePromptWidgetHeight(surface.widget, height + 20))
                    session.active?.geometryChanged();
            },
            report: this.#report,
            appearance: this.appearance,
            regionAccent: this.#regions?.accent.current() ?? null,
            sceneQueue: this.#sceneQueue,
            regionHover: (ordinal) => {
                try {
                    this.#regions?.hoverPrompt(session.node, ordinal);
                }
                catch (error) {
                    this.#report(error);
                }
            },
        });
        if (session.active)
            this.#present(session, session.active);
        void this.#loadCatalog(session).catch(this.#report);
    }
    /** Load optional model metadata independently of initial component construction. */
    async #loadCatalog(session) {
        const snapshot = await this.#catalog.load();
        if (session.disposed)
            return;
        this.#publishCatalog(snapshot);
        this.#stopCatalog ??= this.#catalog.watch((next) => this.#publishCatalog(next), this.#report);
        if (this.#loraCatalog)
            session.editor?.setLoraCatalog(this.#loraCatalog);
    }
    /** Refresh installed identities and decoded media from the existing model catalog lifecycle. */
    #publishCatalog(snapshot) {
        if (snapshot !== this.#catalogSnapshot) {
            this.#catalogSnapshot = snapshot;
            this.#loraCatalog = preparePromptLoraCatalog(snapshot, this.#document.baseURI, (items) => {
                if (items.length)
                    void this.#catalog.prioritize(items).catch(this.#report);
            });
            for (const current of this.#sessions.values())
                current.editor?.setLoraCatalog(this.#loraCatalog);
        }
    }
    /** Hide only the native editing control while retaining its host-facing value contract. */
    #present(session, surface) {
        if (!session.editor || session.disposed)
            return;
        session.editor.attach(surface.textarea);
        if (!surface.textarea.hidden)
            surface.textarea.hidden = true;
        if (surface.textarea.style.getPropertyValue('display') !== 'none' ||
            surface.textarea.style.getPropertyPriority('display') !== 'important')
            surface.textarea.style.setProperty('display', 'none', 'important');
    }
    /** Native permissions remain authoritative across every presentation. */
    #readOnly(surface) {
        return (!surface ||
            surface.hostReadOnly() ||
            surface.textarea.readOnly ||
            surface.textarea.disabled ||
            surface.widget.computedDisabled === true);
    }
}
/** Report whether a node is still a member of the graph it names, independent of renderer DOM. */
function belongsToGraph(node) {
    const graph = node.graph;
    if (!graph)
        return false;
    const nodes = graph._nodes ?? graph.nodes;
    return !Array.isArray(nodes) || nodes.includes(node);
}
/** Keep late detached or inert graph preparations from taking a connected interactive editor. */
function availablePromptSurface(surface) {
    return (surface !== null &&
        surface.textarea.isConnected &&
        !surface.textarea.parentElement?.closest('[inert],[hidden]'));
}

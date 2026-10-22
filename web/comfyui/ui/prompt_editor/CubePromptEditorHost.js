import { PromptWidgetBinding } from './PromptWidgetBinding.js';
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
    #restoreSurfaces = new Map();
    #release = null;
    #catalogSnapshot = null;
    #loraCatalog = null;
    #stopCatalog = null;
    /** Inject existing model catalog and feedback ownership. */
    constructor(document, catalog, report, appearance) {
        this.appearance = appearance;
        this.#document = document;
        this.#catalog = catalog;
        this.#report = report;
    }
    /** Enable prompt enhancement before the host renders Cube fields. */
    setup() {
        this.#release ??= registerPromptEditorSurfaceOwner(this.#document, this);
    }
    /** Acquire a field presentation without duplicating its native editor or serialized value. */
    mount(surface) {
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
                binding: new PromptWidgetBinding(surface.node, surface.widget, (source) => {
                    created.editor?.refresh(source, this.#readOnly(created.active));
                }),
                surfaces: new Set(),
                editor: null,
                active: surface,
                disposed: false,
                creating: false,
            };
            session = created;
            this.#sessions.set(surface.widget, session);
        }
        session.surfaces.add(surface);
        session.active = surface;
        if (!session.editor && !session.creating) {
            session.creating = true;
            void this.#create(session, surface)
                .catch(this.#report)
                .finally(() => {
                session.creating = false;
            });
        }
        this.#present(session, surface);
        return () => {
            if (session.disposed)
                return;
            session.surfaces.delete(surface);
            this.#restoreSurfaces.get(surface)?.();
            this.#restoreSurfaces.delete(surface);
            if (session.active === surface) {
                session.active = [...session.surfaces].at(-1) ?? null;
                if (session.active)
                    this.#present(session, session.active);
                else
                    session.editor?.element.remove();
            }
        };
    }
    /** Reconcile known host values without DOM discovery or unrelated card refresh. */
    refresh(surface) {
        const session = this.#sessions.get(surface.widget);
        if (!session)
            return;
        session.editor?.refresh(session.binding.read(), this.#readOnly(surface));
        if (session.active === surface)
            this.#present(session, surface);
    }
    /** Retire native documents before replacing an entire workflow. */
    beforeConfigureGraph() {
        this.#stopCatalog?.();
        this.#stopCatalog = null;
        for (const [widget, session] of this.#sessions) {
            session.disposed = true;
            session.editor?.dispose();
            session.binding.dispose();
            clearCubePromptWidgetHeight(widget);
            for (const surface of session.surfaces)
                this.#restoreSurfaces.get(surface)?.();
        }
        this.#sessions.clear();
        this.#restoreSurfaces.clear();
    }
    /** Release the document registration and every editor-owned resource. */
    dispose() {
        this.#release?.();
        this.#release = null;
        this.beforeConfigureGraph();
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
        });
        if (session.active)
            this.#present(session, session.active);
        const snapshot = await this.#catalog.load();
        if (session.disposed)
            return;
        this.#publishCatalog(snapshot);
        this.#stopCatalog ??= this.#catalog.watch((next) => this.#publishCatalog(next), this.#report);
        if (this.#loraCatalog)
            session.editor.setLoraCatalog(this.#loraCatalog);
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

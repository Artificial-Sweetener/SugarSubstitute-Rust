/** File types ComfyUI opens as a workflow of their own. */
const WORKFLOW_FILE = /\.(json|png|webp)$/i;
/** The title ComfyUI gives the tab of a dropped file, or null when the file opens no workflow. */
export function fileTabTitle(name) {
    if (!WORKFLOW_FILE.test(name))
        return null;
    return name.replace(/\.\w+$/, '') || null;
}
/**
 * While a file hangs over the tab bar, show the dashed tab it would open as; when it is dropped
 * there, have the workflow ComfyUI opens take that place.
 *
 * ComfyUI itself opens the dropped file. A page cannot read a dragged file's name before the drop,
 * so without a host that names the drag there is nothing to preview and the drop opens the workflow
 * where ComfyUI puts it.
 */
export class WorkflowFileTabDrop {
    host;
    #pointer = null;
    #names = null;
    #shown = null;
    #stop = null;
    #awaitingOpen = false;
    constructor(host) {
        this.host = host;
    }
    /** Start following file drags over the document. */
    setup() {
        if (this.#stop)
            return;
        const { document, dragged } = this.host;
        const unsubscribe = dragged.subscribe(this.#named);
        document.addEventListener('dragover', this.#over, true);
        document.addEventListener('dragleave', this.#leave, true);
        document.addEventListener('drop', this.#drop, true);
        this.#stop = () => {
            unsubscribe();
            document.removeEventListener('dragover', this.#over, true);
            document.removeEventListener('dragleave', this.#leave, true);
            document.removeEventListener('drop', this.#drop, true);
        };
    }
    /** Stop following drags and withdraw anything shown or requested. */
    dispose() {
        this.#stop?.();
        this.#stop = null;
        this.#pointer = null;
        this.#names = null;
        this.#render();
        this.#forget();
    }
    /**
     * The host named a drag or said it ended. Its word that a drag ended can arrive before the
     * page's own drop, so the names are kept until the page sees the drag end too.
     */
    #named = () => {
        const names = this.host.dragged.names;
        if (names || !this.#pointer)
            this.#names = names;
        this.#render();
    };
    #over = (event) => {
        if (![...(event.dataTransfer?.types ?? [])].includes('Files'))
            return;
        this.#forget();
        this.#pointer = { x: event.clientX, y: event.clientY };
        this.#render();
    };
    /** A drag leaves the window when it leaves an element for nothing. */
    #leave = (event) => {
        if (event.relatedTarget !== null)
            return;
        this.#ended();
    };
    #drop = () => {
        const index = this.#shown;
        this.#ended();
        if (index === null)
            return;
        this.host.placeNextOpened(index);
        this.#awaitingOpen = true;
        // The file may open no workflow; the request ends when the person does anything else.
        this.host.document.addEventListener('pointerdown', this.#forget, true);
        this.host.document.addEventListener('keydown', this.#forget, true);
    };
    #ended() {
        this.#pointer = null;
        this.#names = this.host.dragged.names;
        this.#render();
    }
    /** Withdraw a placement request that no opened workflow took. */
    #forget = () => {
        if (!this.#awaitingOpen)
            return;
        this.#awaitingOpen = false;
        this.host.placeNextOpened(null);
        this.host.document.removeEventListener('pointerdown', this.#forget, true);
        this.host.document.removeEventListener('keydown', this.#forget, true);
    };
    #render() {
        const name = this.#names?.[0];
        const title = name === undefined ? null : fileTabTitle(name);
        const index = title !== null && this.#pointer
            ? this.host.tabBar.hit(this.#pointer.x, this.#pointer.y)
            : null;
        if (index === null && this.#shown === null)
            return;
        this.#shown = index;
        this.host.tabBar.preview(index === null ? null : title, index ?? 0);
    }
}

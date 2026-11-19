/**
 * The generation queue's presentation, laid out as SugarSubstitute's queue always has been: a
 * title, then rows of thumbnail, title, status line and one button. It owns no data and performs
 * no action; its owner supplies the rows and receives the requests.
 */
export class GenerationQueueView {
    document;
    surface;
    actions;
    element;
    #title;
    #summary;
    #empty;
    #rows;
    #mounted = new Map();
    #shown = new Map();
    constructor(document, surface, actions) {
        this.document = document;
        this.surface = surface;
        this.actions = actions;
        this.element = this.#make('section', 'sc-generation-queue');
        this.element.dataset.surface = surface;
        this.element.setAttribute('data-sc-style-surface', '');
        this.element.setAttribute('aria-label', 'Generation queue');
        const header = this.#make('header', 'sc-generation-queue-header');
        this.#title = this.#make('h2', 'sc-generation-queue-title');
        const more = this.#button('sc-generation-queue-more', 'More options', () => actions.more(more));
        this.#summary = this.#make('span', 'sc-generation-queue-summary');
        header.append(this.#title, this.#summary, more);
        if (actions.hide)
            header.append(this.#button('sc-generation-queue-hide', 'Hide full queue panel', () => actions.hide?.()));
        this.element.append(header);
        if (actions.search) {
            const search = this.document.createElement('input');
            search.type = 'search';
            search.className = 'sc-generation-queue-search';
            search.placeholder = 'Search...';
            search.setAttribute('aria-label', 'Search jobs');
            search.addEventListener('input', () => actions.search?.(search.value));
            this.element.append(search);
        }
        this.#empty = this.#make('p', 'sc-generation-queue-empty');
        this.#rows = this.#make('div', 'sc-generation-queue-rows');
        this.#rows.setAttribute('role', 'list');
        this.#rows.setAttribute('data-sc-scrollbar-managed', '');
        this.#rows.addEventListener('click', this.#onClick);
        this.#rows.addEventListener('dblclick', this.#onOpen);
        this.#rows.addEventListener('contextmenu', this.#onMenu);
        this.element.append(this.#empty, this.#rows);
        this.update({ items: [], pending: 0, summary: '', narrowed: false, details: new Map() });
    }
    /** Show the queue as it now stands, keeping each job's row in place across updates. */
    update(state) {
        this.#title.textContent =
            this.surface === 'panel'
                ? `Generation Queue :: ${String(state.pending)} Pending Jobs`
                : 'Generation queue';
        if (this.#summary.textContent !== state.summary)
            this.#summary.textContent = state.summary;
        this.#summary.hidden = state.summary === '';
        const listed = state.items.length > 0;
        this.#empty.hidden = listed;
        this.#rows.hidden = !listed;
        this.#empty.textContent = state.narrowed ? 'No matching jobs' : 'No queued jobs';
        const kept = new Set();
        const order = [];
        this.#shown.clear();
        for (const item of state.items) {
            if (item.kind === 'caption') {
                const caption = this.#make('div', 'sc-generation-queue-caption');
                caption.textContent = item.text;
                order.push(caption);
                continue;
            }
            const row = item.row;
            kept.add(row.jobId);
            this.#shown.set(row.jobId, row);
            const element = this.#mounted.get(row.jobId) ?? this.#mount(row.jobId);
            this.#fill(element, row, state.details.get(row.jobId) ?? '');
            order.push(element);
        }
        for (const [id, element] of this.#mounted)
            if (!kept.has(id)) {
                element.remove();
                this.#mounted.delete(id);
            }
        // Captions are rebuilt; rows are moved, so a row under the pointer keeps its hover.
        for (const stale of this.#rows.querySelectorAll('.sc-generation-queue-caption'))
            stale.remove();
        order.forEach((element, index) => {
            if (this.#rows.children[index] !== element)
                this.#rows.insertBefore(element, this.#rows.children[index] ?? null);
        });
    }
    /** Release the view's listeners and leave the page. */
    dispose() {
        this.#rows.removeEventListener('click', this.#onClick);
        this.#rows.removeEventListener('dblclick', this.#onOpen);
        this.#rows.removeEventListener('contextmenu', this.#onMenu);
        this.element.remove();
    }
    #mount(jobId) {
        const row = this.#make('div', 'sc-generation-queue-row');
        row.dataset.jobId = jobId;
        row.setAttribute('role', 'listitem');
        const thumbnail = this.#make('div', 'sc-generation-queue-thumbnail');
        const text = this.#make('div', 'sc-generation-queue-text');
        text.append(this.#make('div', 'sc-generation-queue-row-title'), this.#make('div', 'sc-generation-queue-row-status'));
        const action = this.document.createElement('button');
        action.type = 'button';
        action.className = 'sc-generation-queue-row-action';
        row.append(thumbnail, text, action);
        this.#mounted.set(jobId, row);
        return row;
    }
    #fill(element, row, detail) {
        element.dataset.role = row.role;
        element.dataset.status = row.status;
        element.toggleAttribute('data-video', row.video);
        element.title = detail;
        const [thumbnail, text, action] = element.children;
        const image = thumbnail.querySelector('img');
        if (!row.previewUrl)
            image?.remove();
        else if (image?.getAttribute('src') !== row.previewUrl) {
            const next = image ?? this.document.createElement('img');
            next.alt = '';
            next.decoding = 'async';
            next.loading = 'lazy';
            next.draggable = false;
            next.src = row.previewUrl;
            if (!image)
                thumbnail.append(next);
        }
        const [title, status] = text.children;
        if (title.textContent !== row.title)
            title.textContent = row.title;
        // ComfyUI's extra word about the job follows the status on the same line.
        const line = row.detail ? `${row.subtitle} · ${row.detail}` : row.subtitle;
        if (status.textContent !== line)
            status.textContent = line;
        const label = row.action === 'cancel' ? 'Cancel job' : 'Remove job';
        action.dataset.action = row.action;
        action.title = label;
        action.setAttribute('aria-label', label);
    }
    #onClick = (event) => {
        const row = this.#row(event);
        if (!row || !(event.target instanceof Element))
            return;
        if (event.target.closest('.sc-generation-queue-row-action'))
            this.actions.act(row);
        else if (event.target.closest('.sc-generation-queue-thumbnail') && row.previewUrl)
            this.actions.open(row);
    };
    #onOpen = (event) => {
        const row = this.#row(event);
        if (!row || !(event.target instanceof Element))
            return;
        if (!event.target.closest('.sc-generation-queue-row-action'))
            this.actions.open(row);
    };
    #onMenu = (event) => {
        const row = this.#row(event);
        if (!row)
            return;
        event.preventDefault();
        this.actions.menu(row, event.clientX, event.clientY);
    };
    #row(event) {
        const element = event.target instanceof Element
            ? event.target.closest('.sc-generation-queue-row')
            : null;
        return (element && this.#shown.get(element.dataset.jobId ?? '')) ?? null;
    }
    #make(tag, className) {
        const element = this.document.createElement(tag);
        element.className = className;
        return element;
    }
    #button(className, label, select) {
        const button = this.document.createElement('button');
        button.type = 'button';
        button.className = className;
        button.title = label;
        button.setAttribute('aria-label', label);
        button.addEventListener('click', select);
        return button;
    }
}

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
/** Draw the generation queue: its title, its rows and the few controls around them. */
import type { QueueItem, QueueRow } from './GenerationQueueRows.js';

/** Where the queue is shown: opened from its button, or pinned beside the canvas. */
export type GenerationQueueSurface = 'flyout' | 'panel';

/** What a person can ask of the queue's view. Each request is carried out by the view's owner. */
export interface GenerationQueueViewActions {
  /** The row's one button: cancel a job that has not finished, remove one that has. */
  act(row: QueueRow): void;
  /** Open a finished job's result. */
  open(row: QueueRow): void;
  /** Offer everything that can be done with one job, at the pointer. */
  menu(row: QueueRow, x: number, y: number): void;
  /** Offer the queue's own choices: which jobs to list, how to sort them, clearing, pinning. */
  more(anchor: HTMLElement): void;
  /** Put the pinned queue away. Absent on the fly-out, which closes with its button. */
  hide?(): void;
  /** Narrow the list to jobs matching the typed text. */
  search?(text: string): void;
}

/** What the view shows at one moment. */
export interface GenerationQueueViewState {
  readonly items: readonly QueueItem[];
  readonly pending: number;
  /** What is running and waiting, in ComfyUI's words; empty when nothing is. */
  readonly summary: string;
  /** Whether a filter, tab or search is hiding jobs, which changes what an empty list means. */
  readonly narrowed: boolean;
  /** One line of detail per job for its tooltip. */
  readonly details: ReadonlyMap<string, string>;
}

/**
 * The generation queue's presentation, laid out as SugarSubstitute's queue always has been: a
 * title, then rows of thumbnail, title, status line and one button. It owns no data and performs
 * no action; its owner supplies the rows and receives the requests.
 */
export class GenerationQueueView {
  readonly element: HTMLElement;
  readonly #title: HTMLElement;
  readonly #summary: HTMLElement;
  readonly #empty: HTMLElement;
  readonly #rows: HTMLElement;
  readonly #mounted = new Map<string, HTMLElement>();
  readonly #shown = new Map<string, QueueRow>();
  #state: GenerationQueueViewState | null = null;
  #waitingPreview: readonly string[] | null = null;

  constructor(
    private readonly document: Document,
    readonly surface: GenerationQueueSurface,
    private readonly actions: GenerationQueueViewActions,
  ) {
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
      header.append(
        this.#button('sc-generation-queue-hide', 'Hide full queue panel', () => actions.hide?.()),
      );
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

  /** The scrolling list, for the owner of dragging within it. */
  get rows(): HTMLElement {
    return this.#rows;
  }

  /** The waiting jobs as shown, top to bottom. */
  waiting(): readonly string[] {
    return [...this.#shown.values()]
      .filter((row) => row.role === 'waiting')
      .map((row) => row.jobId);
  }

  /** Show the waiting jobs in `order` while one is being dragged; null shows the accepted order. */
  previewWaiting(order: readonly string[] | null): void {
    this.#waitingPreview = order;
    if (this.#state) this.update(this.#state);
  }

  /** Show the queue as it now stands, keeping each job's row in place across updates. */
  update(state: GenerationQueueViewState): void {
    this.#state = state;
    this.#title.textContent =
      this.surface === 'panel'
        ? `Generation Queue :: ${String(state.pending)} Pending Jobs`
        : 'Generation queue';
    if (this.#summary.textContent !== state.summary) this.#summary.textContent = state.summary;
    this.#summary.hidden = state.summary === '';
    const listed = state.items.length > 0;
    this.#empty.hidden = listed;
    this.#rows.hidden = !listed;
    this.#empty.textContent = state.narrowed ? 'No matching jobs' : 'No queued jobs';
    const kept = new Set<string>();
    const order: HTMLElement[] = [];
    this.#shown.clear();
    for (const item of this.#ordered(state.items)) {
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
    for (const stale of this.#rows.querySelectorAll('.sc-generation-queue-caption')) stale.remove();
    order.forEach((element, index) => {
      if (this.#rows.children[index] !== element)
        this.#rows.insertBefore(element, this.#rows.children[index] ?? null);
    });
  }

  /** Put the waiting rows in the previewed order; everything else keeps its place. */
  #ordered(items: readonly QueueItem[]): readonly QueueItem[] {
    const preview = this.#waitingPreview;
    if (!preview) return items;
    const waiting = items.filter((item) => item.kind === 'row' && item.row.role === 'waiting');
    const place = (item: QueueItem): number =>
      item.kind === 'row' ? preview.indexOf(item.row.jobId) : -1;
    // A job the preview does not name, queued since the drag began, stays ahead of the others.
    const sorted = [...waiting].sort((left, right) => place(left) - place(right));
    let next = 0;
    return items.map((item) =>
      item.kind === 'row' && item.row.role === 'waiting' ? (sorted[next++] ?? item) : item,
    );
  }

  /** Release the view's listeners and leave the page. */
  dispose(): void {
    this.#rows.removeEventListener('click', this.#onClick);
    this.#rows.removeEventListener('dblclick', this.#onOpen);
    this.#rows.removeEventListener('contextmenu', this.#onMenu);
    this.element.remove();
  }

  #mount(jobId: string): HTMLElement {
    const row = this.#make('div', 'sc-generation-queue-row');
    row.dataset.jobId = jobId;
    row.setAttribute('role', 'listitem');
    const thumbnail = this.#make('div', 'sc-generation-queue-thumbnail');
    const text = this.#make('div', 'sc-generation-queue-text');
    text.append(
      this.#make('div', 'sc-generation-queue-row-title'),
      this.#make('div', 'sc-generation-queue-row-status'),
    );
    const action = this.document.createElement('button');
    action.type = 'button';
    action.className = 'sc-generation-queue-row-action';
    row.append(thumbnail, text, action);
    this.#mounted.set(jobId, row);
    return row;
  }

  #fill(element: HTMLElement, row: QueueRow, detail: string): void {
    element.dataset.role = row.role;
    element.dataset.status = row.status;
    element.toggleAttribute('data-video', row.video);
    element.title = detail;
    const [thumbnail, text, action] = element.children as unknown as [
      HTMLElement,
      HTMLElement,
      HTMLButtonElement,
    ];
    const image = thumbnail.querySelector('img');
    if (!row.previewUrl) image?.remove();
    else if (image?.getAttribute('src') !== row.previewUrl) {
      const next = image ?? this.document.createElement('img');
      next.alt = '';
      next.decoding = 'async';
      next.loading = 'lazy';
      next.draggable = false;
      next.src = row.previewUrl;
      if (!image) thumbnail.append(next);
    }
    const [title, status] = text.children as unknown as [HTMLElement, HTMLElement];
    if (title.textContent !== row.title) title.textContent = row.title;
    // ComfyUI's extra word about the job follows the status on the same line.
    const line = row.detail ? `${row.subtitle} · ${row.detail}` : row.subtitle;
    if (status.textContent !== line) status.textContent = line;
    const label = row.action === 'cancel' ? 'Cancel job' : 'Remove job';
    action.dataset.action = row.action;
    action.title = label;
    action.setAttribute('aria-label', label);
  }

  readonly #onClick = (event: MouseEvent): void => {
    const row = this.#row(event);
    if (!row || !(event.target instanceof Element)) return;
    if (event.target.closest('.sc-generation-queue-row-action')) this.actions.act(row);
    else if (event.target.closest('.sc-generation-queue-thumbnail') && row.previewUrl)
      this.actions.open(row);
  };

  readonly #onOpen = (event: MouseEvent): void => {
    const row = this.#row(event);
    if (!row || !(event.target instanceof Element)) return;
    if (!event.target.closest('.sc-generation-queue-row-action')) this.actions.open(row);
  };

  readonly #onMenu = (event: MouseEvent): void => {
    const row = this.#row(event);
    if (!row) return;
    event.preventDefault();
    this.actions.menu(row, event.clientX, event.clientY);
  };

  #row(event: Event): QueueRow | null {
    const element =
      event.target instanceof Element
        ? event.target.closest<HTMLElement>('.sc-generation-queue-row')
        : null;
    return (element && this.#shown.get(element.dataset.jobId ?? '')) ?? null;
  }

  #make(tag: string, className: string): HTMLElement {
    const element = this.document.createElement(tag);
    element.className = className;
    return element;
  }

  #button(className: string, label: string, select: () => void): HTMLButtonElement {
    const button = this.document.createElement('button');
    button.type = 'button';
    button.className = className;
    button.title = label;
    button.setAttribute('aria-label', label);
    button.addEventListener('click', select);
    return button;
  }
}

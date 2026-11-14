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
/**
 * Decide when the What's new view opens and what it shows for every product on the page.
 *
 * Each product's notes come from one {@link ReleaseNotesSource}: SugarCubes' own, and a host's
 * when the host publishes notes. The first start on a newer version of any product opens the view
 * once, with that product's notes since the version the user last saw; products that updated in
 * the same start share the one view as separate sections. The user can open the same view again
 * from the host or the Updates entry. Which versions the user has seen is kept by a
 * {@link SeenReleaseStore}; a product seen for the first time is recorded without opening anything.
 */
import type { HostReleaseNotesLink } from './HostReleaseNotesLink.js';
import {
  compareReleaseVersions,
  type ReleaseNotesOutcome,
  type ReleaseNotesSource,
} from './ReleaseNotesSource.js';

/** Remember the newest version of each product the user has been shown. */
export interface SeenReleaseStore {
  read(product: string): string | null;
  record(product: string, version: string): void;
}

/** Where one section's notes stand. */
export type WhatsNewSectionStatus = { readonly kind: 'loading' } | ReleaseNotesOutcome;

/** One product's part of the view. */
export interface WhatsNewSection {
  /** Stable within the page; retry names a section by it. */
  readonly key: string;
  readonly product: string;
  readonly currentVersion: string;
  /** The version the user last saw, when this start follows an update of the product. */
  readonly updatedFrom: string | null;
  readonly status: WhatsNewSectionStatus;
}

/** What the view reports back. */
export interface WhatsNewViewActions {
  retry(key: string): void;
  /** The user closed the view. */
  closed(): void;
}

/** The view that shows the sections. */
export interface WhatsNewView {
  /** Show the sections, opening the view when it is closed; a later call replaces them. */
  show(sections: readonly WhatsNewSection[], actions: WhatsNewViewActions): void;
}

/** Hand a host's release notes link to the page's one What's new owner. */
export interface ReleaseNotesHostAttachment {
  /** @returns The release that removes the host's notes from the page. */
  attachHost(link: HostReleaseNotesLink): () => void;
}

interface Entry {
  readonly key: string;
  readonly source: ReleaseNotesSource;
  product: string | null;
  currentVersion: string | null;
  updatedFrom: string | null;
  outcome: ReleaseNotesOutcome | null;
  /** Increases with every read, so a superseded read's answer is ignored. */
  reads: number;
  loading: boolean;
}

/** Own the What's new view for every product on this page. */
export class WhatsNewPresenter implements ReleaseNotesHostAttachment {
  readonly #entries: Entry[] = [];
  readonly #shown = new Set<string>();
  #open = false;
  #everything = false;
  #sources = 0;
  readonly #actions: WhatsNewViewActions = {
    retry: (key) => this.#retry(key),
    closed: () => {
      this.#open = false;
      this.#everything = false;
      this.#shown.clear();
    },
  };

  constructor(
    private readonly view: WhatsNewView,
    private readonly seen: SeenReleaseStore,
    private readonly report: (error: unknown) => void,
  ) {}

  /**
   * Show `source`'s notes on this page.
   *
   * @returns The release that removes them.
   */
  attach(source: ReleaseNotesSource): () => void {
    const entry: Entry = {
      key: `source-${String(++this.#sources)}`,
      source,
      product: null,
      currentVersion: null,
      updatedFrom: null,
      outcome: null,
      reads: 0,
      loading: false,
    };
    this.#entries.push(entry);
    const stop = source.subscribe(() => this.#identify(entry));
    this.#identify(entry);
    return () => {
      stop();
      const index = this.#entries.indexOf(entry);
      if (index < 0) return;
      this.#entries.splice(index, 1);
      if (this.#shown.delete(entry.key)) this.#render();
    };
  }

  attachHost(link: HostReleaseNotesLink): () => void {
    const detach = this.attach(link);
    const stopOpen = link.onOpen(() => this.open());
    return () => {
      stopOpen();
      detach();
    };
  }

  /** Show every product's notes, as the user asked to. */
  open(): void {
    this.#everything = true;
    for (const entry of this.#entries) {
      if (entry.currentVersion === null) continue;
      this.#shown.add(entry.key);
      if (entry.outcome?.kind === 'failed' && !entry.loading) entry.outcome = null;
    }
    this.#render(true);
  }

  /** Adopt a source's product and version once, deciding whether its update opens the view. */
  #identify(entry: Entry): void {
    const identity = entry.source.read();
    if (!identity || entry.currentVersion !== null) return;
    entry.product = identity.product;
    entry.currentVersion = identity.currentVersion;
    let seen: string | null = null;
    try {
      seen = this.seen.read(identity.product);
      if (seen !== identity.currentVersion)
        this.seen.record(identity.product, identity.currentVersion);
    } catch (error: unknown) {
      this.report(error);
    }
    const newer = seen === null ? null : compareReleaseVersions(identity.currentVersion, seen);
    entry.updatedFrom = newer !== null && newer > 0 ? seen : null;
    if (entry.updatedFrom === null && !(this.#open && this.#everything)) return;
    this.#shown.add(entry.key);
    this.#render(true);
  }

  #retry(key: string): void {
    const entry = this.#entries.find((candidate) => candidate.key === key);
    if (!entry || entry.loading) return;
    entry.outcome = null;
    this.#render();
  }

  /** Start every missing read, then show the sections; `opening` opens a closed view. */
  #render(opening = false): void {
    if (!this.#open && !opening) return;
    const sections: WhatsNewSection[] = [];
    for (const entry of this.#entries) {
      if (!this.#shown.has(entry.key) || entry.product === null || entry.currentVersion === null)
        continue;
      if (!entry.outcome && !entry.loading) this.#load(entry, entry.currentVersion);
      sections.push({
        key: entry.key,
        product: entry.product,
        currentVersion: entry.currentVersion,
        updatedFrom: entry.updatedFrom,
        status: entry.outcome ?? { kind: 'loading' },
      });
    }
    if (sections.length === 0) return;
    this.#open = true;
    try {
      this.view.show(sections, this.#actions);
    } catch (error: unknown) {
      this.report(error);
    }
  }

  #load(entry: Entry, currentVersion: string): void {
    entry.loading = true;
    const read = ++entry.reads;
    void entry.source.load(entry.updatedFrom ?? currentVersion).then((outcome) => {
      if (read !== entry.reads) return;
      entry.loading = false;
      entry.outcome = outcome;
      this.#render();
    });
  }
}

import { compareReleaseVersions, } from './ReleaseNotesSource.js';
/** Own the What's new view for every product on this page. */
export class WhatsNewPresenter {
    view;
    seen;
    report;
    #entries = [];
    #shown = new Set();
    #open = false;
    #everything = false;
    #sources = 0;
    #actions = {
        retry: (key) => this.#retry(key),
        closed: () => {
            this.#open = false;
            this.#everything = false;
            this.#shown.clear();
        },
    };
    constructor(view, seen, report) {
        this.view = view;
        this.seen = seen;
        this.report = report;
    }
    /**
     * Show `source`'s notes on this page.
     *
     * @returns The release that removes them.
     */
    attach(source) {
        const entry = {
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
            if (index < 0)
                return;
            this.#entries.splice(index, 1);
            if (this.#shown.delete(entry.key))
                this.#render();
        };
    }
    attachHost(link) {
        const detach = this.attach(link);
        const stopOpen = link.onOpen(() => this.open());
        return () => {
            stopOpen();
            detach();
        };
    }
    /** Show every product's notes, as the user asked to. */
    open() {
        this.#everything = true;
        for (const entry of this.#entries) {
            if (entry.currentVersion === null)
                continue;
            this.#shown.add(entry.key);
            if (entry.outcome?.kind === 'failed' && !entry.loading)
                entry.outcome = null;
        }
        this.#render(true);
    }
    /** Adopt a source's product and version once, deciding whether its update opens the view. */
    #identify(entry) {
        const identity = entry.source.read();
        if (!identity || entry.currentVersion !== null)
            return;
        entry.product = identity.product;
        entry.currentVersion = identity.currentVersion;
        let seen = null;
        try {
            seen = this.seen.read(identity.product);
            if (seen !== identity.currentVersion)
                this.seen.record(identity.product, identity.currentVersion);
        }
        catch (error) {
            this.report(error);
        }
        const newer = seen === null ? null : compareReleaseVersions(identity.currentVersion, seen);
        entry.updatedFrom = newer !== null && newer > 0 ? seen : null;
        if (entry.updatedFrom === null && !(this.#open && this.#everything))
            return;
        this.#shown.add(entry.key);
        this.#render(true);
    }
    #retry(key) {
        const entry = this.#entries.find((candidate) => candidate.key === key);
        if (!entry || entry.loading)
            return;
        entry.outcome = null;
        this.#render();
    }
    /** Start every missing read, then show the sections; `opening` opens a closed view. */
    #render(opening = false) {
        if (!this.#open && !opening)
            return;
        const sections = [];
        for (const entry of this.#entries) {
            if (!this.#shown.has(entry.key) || entry.product === null || entry.currentVersion === null)
                continue;
            if (!entry.outcome && !entry.loading)
                this.#load(entry, entry.currentVersion);
            sections.push({
                key: entry.key,
                product: entry.product,
                currentVersion: entry.currentVersion,
                updatedFrom: entry.updatedFrom,
                status: entry.outcome ?? { kind: 'loading' },
            });
        }
        if (sections.length === 0)
            return;
        this.#open = true;
        try {
            this.view.show(sections, this.#actions);
        }
        catch (error) {
            this.report(error);
        }
    }
    #load(entry, currentVersion) {
        entry.loading = true;
        const read = ++entry.reads;
        void entry.source.load(entry.updatedFrom ?? currentVersion).then((outcome) => {
            if (read !== entry.reads)
                return;
            entry.loading = false;
            entry.outcome = outcome;
            this.#render();
        });
    }
}

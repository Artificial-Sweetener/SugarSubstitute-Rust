import { ContextMenu } from '../../menus/ContextMenu.js';
const GENERATE_GLYPH = 'play.svg';
const CONTINUOUS_GLYPH = 'infinity.svg';
/** Keep controls and keyboard focus stable through native execution and preview updates. */
export class GenerationStripView {
    document;
    actions;
    element;
    queueSlot;
    #input;
    #up;
    #down;
    #generate;
    #cancel;
    #skip;
    #modes;
    #state = {
        batch: 1,
        generateEnabled: false,
        cancelEnabled: false,
        skipEnabled: false,
        mode: 'generate',
        looping: false,
    };
    /** Build accessible segments in the source order: batch, cancel, generate, skip. */
    constructor(document, actions) {
        this.document = document;
        this.actions = actions;
        this.element = document.createElement('div');
        this.element.className = 'sc-generation-strip';
        this.element.setAttribute('role', 'group');
        this.element.setAttribute('aria-label', 'Generation controls');
        const batch = document.createElement('div');
        batch.className = 'sc-generation-batch';
        this.#input = document.createElement('input');
        this.#input.type = 'text';
        this.#input.inputMode = 'numeric';
        this.#input.setAttribute('role', 'spinbutton');
        this.#input.setAttribute('aria-label', 'Generation batch count');
        this.#input.setAttribute('aria-valuemin', '1');
        this.#input.setAttribute('aria-valuemax', '999');
        this.#input.value = '1';
        this.#input.addEventListener('focus', () => this.#input.select());
        this.#input.addEventListener('change', () => this.#commit());
        this.#input.addEventListener('blur', () => this.#commit());
        this.#input.addEventListener('keydown', (event) => {
            if (event.key === 'Escape') {
                this.#input.value = String(this.#state.batch);
                this.#input.blur();
            }
            if (event.key === 'Enter') {
                this.#commit();
                this.#input.blur();
            }
            if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
                event.preventDefault();
                this.#adjust(event.key === 'ArrowUp' ? 1 : -1);
            }
        });
        this.#up = this.#step('up', 1);
        this.#down = this.#step('down', -1);
        batch.append(this.#input, this.#up, this.#down);
        batch.addEventListener('wheel', (event) => {
            if (event.deltaY === 0 || !this.#state.generateEnabled)
                return;
            event.preventDefault();
            this.#adjust(event.deltaY < 0 ? 1 : -1);
        }, { passive: false });
        const cluster = document.createElement('div');
        cluster.className = 'sc-generation-actions';
        this.#cancel = this.#segment('cancel', 'Cancel generation', 'stop.svg', () => actions.cancel());
        this.#generate = this.#segment('generate', 'Generate', GENERATE_GLYPH, () => actions.generate());
        this.#modes = new ContextMenu(document);
        // Right-clicking Generate chooses what it does, as in the original Substitute.
        this.#generate.addEventListener('contextmenu', (event) => {
            event.preventDefault();
            event.stopPropagation();
            this.#modes.open({
                label: 'Generation mode',
                entries: this.#modeEntries(),
                anchor: { kind: 'pointer', x: event.clientX, y: event.clientY },
                trigger: this.#generate,
            });
        });
        this.#skip = this.#segment('skip', 'Skip generation', 'next.svg', () => actions.skip());
        cluster.append(this.#cancel, this.#generate, this.#skip);
        this.queueSlot = document.createElement('span');
        this.queueSlot.className = 'sc-generation-queue-slot';
        this.queueSlot.setAttribute('aria-hidden', 'true');
        cluster.append(this.queueSlot);
        this.element.append(batch, cluster);
    }
    /** Project execution availability without replacing controls or overwriting an in-progress edit. */
    update(state) {
        const batchChanged = state.batch !== this.#state.batch;
        this.#state = state;
        if (batchChanged || this.document.activeElement !== this.#input)
            this.#input.value = String(state.batch);
        this.#input.setAttribute('aria-valuenow', String(state.batch));
        this.#input.disabled = !state.generateEnabled;
        // A running loop can always be stopped from Generate.
        this.#generate.disabled = !state.generateEnabled && !state.looping;
        this.#presentMode(state);
        this.#cancel.disabled = !state.cancelEnabled;
        this.#skip.disabled = !state.skipEnabled;
        this.#up.disabled = !state.generateEnabled || state.batch >= 999;
        this.#down.disabled = !state.generateEnabled || state.batch <= 1;
    }
    /** Release only owned presentation nodes. */
    dispose() {
        this.#modes.close();
        this.element.remove();
    }
    /** Show the chosen mode on Generate: its icon, and the press's meaning while a loop runs. */
    #presentMode(state) {
        const continuous = state.mode === 'continuous';
        const label = state.looping
            ? 'Stop continuous generation'
            : continuous
                ? 'Generate continuously'
                : 'Generate';
        this.#generate.dataset.mode = state.mode;
        this.#generate.toggleAttribute('data-looping', state.looping);
        this.#generate.setAttribute('aria-label', label);
        this.#generate.title = label;
        this.#glyph(this.#generate, continuous ? CONTINUOUS_GLYPH : GENERATE_GLYPH);
    }
    /** The mode menu: Generate and Continuous, each with its icon, the chosen one checked. */
    #modeEntries() {
        const entry = (mode, label, glyph) => ({
            label,
            checked: this.#state.mode === mode,
            selection: 'single',
            icon: (document) => {
                const icon = document.createElement('span');
                icon.className = 'sc-generation-glyph';
                icon.setAttribute('aria-hidden', 'true');
                icon.style.setProperty('--sc-generation-glyph', glyphUrl(glyph));
                return icon;
            },
            select: () => {
                try {
                    this.actions.setMode(mode);
                }
                catch (error) {
                    this.actions.report(error);
                }
            },
        });
        return [
            entry('generate', 'Generate', GENERATE_GLYPH),
            entry('continuous', 'Continuous', CONTINUOUS_GLYPH),
        ];
    }
    /** Point one segment's tinted glyph at a packaged icon. */
    #glyph(button, filename) {
        button
            .querySelector('.sc-generation-glyph')
            ?.style.setProperty('--sc-generation-glyph', glyphUrl(filename));
    }
    /** Commit valid numeric edits and restore the canonical value for empty or invalid text. */
    #commit() {
        const value = this.#input.value.trim();
        if (/^\d+$/.test(value))
            this.#set(Number(value));
        this.#input.value = String(this.#state.batch);
    }
    /** Apply one accessory step against the current host value. */
    #adjust(delta) {
        this.#set(this.#state.batch + delta);
    }
    /** Route synchronous settings failures through the shared feedback boundary. */
    #set(value) {
        if (!this.#state.generateEnabled ||
            Math.max(1, Math.min(999, Math.trunc(value))) === this.#state.batch)
            return;
        try {
            this.actions.setBatch(value);
        }
        catch (error) {
            this.actions.report(error);
        }
    }
    /** Draw the source-matched chevrons with separate accessible hit targets. */
    #step(direction, delta) {
        const button = this.document.createElement('button');
        button.type = 'button';
        button.className = 'sc-generation-step';
        button.dataset.direction = direction;
        button.setAttribute('aria-label', delta > 0 ? 'Increase generation batch' : 'Decrease generation batch');
        const glyph = this.document.createElement('span');
        glyph.setAttribute('aria-hidden', 'true');
        button.append(glyph);
        button.addEventListener('click', () => this.#adjust(delta));
        return button;
    }
    /** Keep trusted packaged SVG artwork theme-tinted and retain asynchronous action errors. */
    #segment(id, label, filename, action) {
        const button = this.document.createElement('button');
        button.type = 'button';
        button.className = 'sc-generation-action';
        button.dataset.action = id;
        button.setAttribute('aria-label', label);
        button.title = label;
        const glyph = this.document.createElement('span');
        glyph.className = 'sc-generation-glyph';
        glyph.setAttribute('aria-hidden', 'true');
        glyph.style.setProperty('--sc-generation-glyph', glyphUrl(filename));
        button.append(glyph);
        button.addEventListener('click', () => {
            void action().catch(this.actions.report);
        });
        return button;
    }
}
/** The packaged strip icon `filename`, as a CSS image. */
function glyphUrl(filename) {
    return `url("${new URL(`../../../../orb/${filename}`, import.meta.url).href}")`;
}

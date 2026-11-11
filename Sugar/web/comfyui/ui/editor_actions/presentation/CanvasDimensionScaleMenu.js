import { CanvasCancellation, canvasSizingProposal } from '../../core/embedded/CanvasRuntime.js';
/** Retain the factor input while discovery changes only menu values and available output paths. */
export class CanvasDimensionScaleMenu {
    action;
    execute;
    run;
    #request = null;
    #scale = '1';
    #output = null;
    #message = 'Reading canvas dimensions…';
    #pending = null;
    #refresh = () => { };
    #close = () => { };
    #submitted = false;
    /** Borrow the application command without owning graph, geometry or probe policy. */
    constructor(action, execute, run) {
        this.action = action;
        this.execute = execute;
        this.run = run;
    }
    /** Build one stable submenu whose dynamic labels never replace its input. */
    entries() {
        const owner = this;
        return [
            {
                label: 'Scale factor',
                get disabled() {
                    return owner.#pending === null;
                },
                input: {
                    get value() {
                        return owner.#scale;
                    },
                    minimum: 0.000001,
                    maximum: 1000,
                    step: 'any',
                    change: (value) => {
                        this.#scale = value;
                    },
                },
            },
            {
                get label() {
                    const output = owner.#request?.outputs.find((item) => item.id === owner.#output);
                    return output ? `Output: ${output.label}` : 'Choose output path';
                },
                get disabled() {
                    return !owner.#pending || !owner.#request?.outputs.length;
                },
                submenuPlacement: 'below',
                get children() {
                    return (owner.#request?.outputs.map((output) => ({
                        label: output.label,
                        get checked() {
                            return owner.#output === output.id;
                        },
                        keepOpen: 'parent',
                        select: () => {
                            owner.#output = output.id;
                            owner.#refresh();
                        },
                    })) ?? []);
                },
            },
            {
                label: 'Discover scale factor',
                keepOpen: true,
                get disabled() {
                    return !owner.#pending || Boolean(owner.#request?.outputs.length && !owner.#output);
                },
                select: () => this.#choose('discover'),
            },
            {
                get label() {
                    const size = owner.#size();
                    return size
                        ? `${owner.#request?.direction === 'to-canvas' ? 'Canvas' : 'Size'}: ${size.width} × ${size.height} px`
                        : owner.#pending
                            ? 'Enter a positive scale factor ≤ 1000'
                            : owner.#message;
                },
                disabled: true,
            },
            {
                label: 'Apply dimensions',
                keepOpen: true,
                get disabled() {
                    return !owner.#pending || !owner.#size();
                },
                select: () => {
                    this.#submitted = true;
                    this.#choose('apply');
                    this.#close();
                },
            },
        ];
    }
    /** Cancel abandoned interactions, while an explicit Apply finishes its captured command. */
    opened = (refresh, close) => {
        const cancellation = new CanvasCancellation();
        this.#request = null;
        this.#scale = '1';
        this.#output = null;
        this.#message = 'Reading canvas dimensions…';
        this.#submitted = false;
        this.#refresh = refresh;
        this.#close = close;
        const configure = (request) => {
            if (cancellation.cancelled())
                return Promise.resolve({ choice: 'cancel', scale: request.scale, output: request.output });
            this.#request = request;
            this.#scale = request.scale;
            this.#output =
                request.output ?? (request.outputs.length === 1 ? (request.outputs[0]?.id ?? null) : null);
            return new Promise((resolve) => {
                const stop = cancellation.subscribe(() => {
                    this.#pending = null;
                    resolve({ choice: 'cancel', scale: request.scale, output: request.output });
                });
                this.#pending = (choice) => {
                    stop();
                    resolve(choice);
                };
                refresh();
            });
        };
        this.run(this.execute(this.action, configure, cancellation).catch((error) => {
            if (!cancellation.cancelled()) {
                this.#pending = null;
                this.#message = error instanceof Error ? error.message : 'Dimensions are unavailable.';
                refresh();
                throw error;
            }
        }));
        return () => {
            if (!this.#submitted)
                cancellation.cancel();
        };
    };
    /** Reuse the same constrained arithmetic that the command applies. */
    #size() {
        if (!this.#request || !this.#pending)
            return null;
        try {
            return canvasSizingProposal(this.#request, this.#scale);
        }
        catch {
            return null;
        }
    }
    /** Resume application discovery or mutation only after the user selects the corresponding command. */
    #choose(choice) {
        const pending = this.#pending;
        if (!pending || !this.#request)
            return;
        const scale = this.#size() ? this.#scale : this.#request.scale;
        this.#pending = null;
        this.#message = choice === 'discover' ? 'Discovering scale factor…' : 'Applying dimensions…';
        pending({ choice, scale, output: this.#output });
        this.#refresh();
    }
}

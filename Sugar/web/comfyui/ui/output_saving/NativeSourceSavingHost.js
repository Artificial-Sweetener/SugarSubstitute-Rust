import { ComfySourceSavingDocument, } from './ComfySourceSavingDocument.js';
import { NativeSavePrefixFields } from './NativeSavePrefixFields.js';
/** Keep source UI independent from mounted image resources and native graph rendering. */
export class NativeSourceSavingHost {
    feature;
    options;
    report;
    surfaces;
    #capabilities = new Map();
    #stop = null;
    #epoch = 0;
    #started = false;
    #fields = null;
    #stops = [];
    /** Bind the presentation feature and existing active-workflow adapters. */
    constructor(feature, options, report, surfaces) {
        this.feature = feature;
        this.options = options;
        this.report = report;
        this.surfaces = surfaces;
    }
    /** Qualify types once and lease a provider that reads cached native source handles. */
    setup() {
        if (this.#started)
            return;
        this.#started = true;
        this.#fields = new NativeSavePrefixFields(this.surfaces.document);
        const refresh = () => {
            try {
                this.#fields?.reconcile(this.options
                    .sources()
                    .filter((source) => this.#capabilities.get(String(source.node.type))?.managed === true), this.feature.configuration.read()?.preferences.manage_workflow === true);
            }
            catch (error) {
                this.report(error);
            }
        };
        this.#stops.push(this.feature.configuration.subscribe(refresh), this.surfaces.subscribe(refresh));
        const document = new ComfySourceSavingDocument({
            ...this.options,
            capabilities: () => this.#capabilities,
        });
        const epoch = ++this.#epoch;
        void this.feature.capabilities
            .load()
            .then((capabilities) => {
            if (epoch === this.#epoch) {
                this.#capabilities = capabilities;
                this.#stop = this.feature.controls?.bind(document) ?? null;
                refresh();
            }
        })
            .catch(this.report);
    }
    /** Release access without retiring workflow results, settings or other windows. */
    dispose() {
        this.#epoch++;
        this.#started = false;
        for (const stop of this.#stops.splice(0))
            stop();
        this.#fields?.dispose();
        this.#fields = null;
        this.#stop?.();
        this.#stop = null;
        this.#capabilities = new Map();
    }
}

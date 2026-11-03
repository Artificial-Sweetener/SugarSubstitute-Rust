/** Resolve the card's endpoint at use time so renumbered or moved nodes stay addressed. */
export class ComfyInputSurfaceCards {
    associations;
    route;
    appearance;
    provisioning;
    imports;
    bindings;
    /** Share the route, appearance and provisioning owners with the canvas tab controls. */
    constructor(associations, route, appearance, provisioning, imports, bindings) {
        this.associations = associations;
        this.route = route;
        this.appearance = appearance;
        this.provisioning = provisioning;
        this.imports = imports;
        this.bindings = bindings;
    }
    /** Bind one loader field's card. */
    port(node, loader) {
        const endpoint = () => this.associations.inventory.workflowKey(this.associations.key(node, loader.field));
        return {
            activate: (entry) => {
                const key = endpoint();
                if (!key)
                    return;
                if (loader.kind === 'image')
                    this.route.activateImage(key);
                else
                    this.route.activateMask(key, loader.ordered ? entry : null);
            },
            hover: (entry) => {
                const key = endpoint();
                if (key && loader.kind === 'mask' && loader.ordered)
                    this.appearance.hover(key, entry);
            },
            opacity: () => {
                const key = endpoint();
                return key && loader.kind === 'mask' ? this.appearance.opacity(key) : null;
            },
            changeOpacity: (opacity) => {
                const key = endpoint();
                if (key && loader.kind === 'mask')
                    this.appearance.changeOpacity(key, opacity);
            },
            canImport: () => {
                const key = endpoint();
                return Boolean(key && loader.kind === 'mask' && loader.ordered && this.imports.available(key));
            },
            importMasks: async (files) => {
                const key = endpoint();
                if (!key || loader.kind !== 'mask' || !loader.ordered)
                    throw new Error('This loader cannot import masks.');
                await this.imports.import(key, files, (staged) => this.bindings.get(node, loader).append(staged));
            },
            subscribe: (listener) => this.provisioning.subscribe(listener),
        };
    }
}

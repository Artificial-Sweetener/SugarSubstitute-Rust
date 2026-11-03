/** Resolve the card's endpoint at use time so renumbered or moved nodes stay addressed. */
export class ComfyInputSurfaceCards {
    associations;
    route;
    appearance;
    provisioning;
    /** Share the route, appearance and provisioning owners with the canvas tab controls. */
    constructor(associations, route, appearance, provisioning) {
        this.associations = associations;
        this.route = route;
        this.appearance = appearance;
        this.provisioning = provisioning;
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
            subscribe: (listener) => this.provisioning.subscribe(listener),
        };
    }
}

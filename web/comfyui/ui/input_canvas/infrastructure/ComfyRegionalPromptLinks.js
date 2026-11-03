/** Address prompt editors by node while the regional hover owner addresses prompts by path. */
export class ComfyRegionalPromptLinks {
    associations;
    hover;
    /** Resolve paths through the association inventory that read the regional topology. */
    constructor(associations, hover) {
        this.associations = associations;
        this.hover = hover;
    }
    /** Report a prompt editor's separator hover, or its end with null. */
    hoverPrompt(node, ordinal) {
        const path = this.associations.inventory.path(node);
        if (path !== null)
            this.hover.hoverPrompt(path, ordinal);
    }
    /** Let prompt editors show the separator of a region hovered in another view. */
    connectPrompts(editors) {
        return this.hover.connect({
            highlight: (prompts, ordinal) => {
                const nodes = new Set();
                for (const path of prompts) {
                    const node = this.associations.inventory.node(path);
                    if (node)
                        nodes.add(node);
                }
                editors.highlightRegions(nodes, ordinal);
            },
        });
    }
}

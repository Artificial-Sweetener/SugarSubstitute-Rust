/** Own contrast selection for DOM icons across graph, masonry, stack, and dialogs. */
export class CubeIconAppearanceHost {
    #style;
    #unsubscribe;
    /** Follow the existing appearance authority while retaining every resolved icon element. */
    constructor(document, source) {
        this.#style = document.createElement('style');
        this.#style.id = 'sugarcubes-cube-icon-appearance';
        document.head.append(this.#style);
        const refresh = () => {
            const tone = source.appearance.dark ? 'dark' : 'bright';
            this.#style.textContent = `
        [data-icon-tone] > .sugarcubes-cube-icon__contrast { display:none; width:100%; height:100%; object-fit:contain; }
        [data-icon-tone="${tone}"][data-icon-tone] > img { display:none; }
        [data-icon-tone="${tone}"][data-icon-tone] > .sugarcubes-cube-icon__contrast { display:block; }
        .sugarcubes-cube-icon__fallback { filter:${source.appearance.dark ? 'none' : 'brightness(0)'}; }
      `;
        };
        this.#unsubscribe = source.subscribeAppearance(refresh);
        refresh();
    }
    /** Release the subscription and exact owned stylesheet with the appearance host. */
    dispose() {
        this.#unsubscribe();
        this.#style.remove();
    }
}

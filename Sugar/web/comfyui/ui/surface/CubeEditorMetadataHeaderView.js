import { presentCubeModelTitle } from './CubeModelPillDomRenderer.js';
import { createCubeUnsavedIndicator } from './CubeUnsavedIndicator.js';
import { createComfyPrimeIconElement } from './ComfyPrimeIcons.js';
/** Keep header presentation separate from metadata draft and save lifecycles. */
export class CubeEditorMetadataHeaderView {
    element;
    #title;
    #save;
    /** Compose literal Cube identity, first-save feedback, and the available editing actions. */
    constructor(document, options) {
        this.element = document.createElement('header');
        this.element.className = 'sugarcubes-cube-editor-metadata__titlebar';
        this.#title = document.createElement('strong');
        presentCubeModelTitle(this.#title, options.title);
        this.element.append(this.#title);
        if (options.unsaved)
            this.element.append(createCubeUnsavedIndicator(document));
        this.#save = options.editable ? this.#createSave(document, options) : null;
        if (this.#save)
            this.element.append(this.#save);
        this.element.append(this.#createCollapse(document, options));
    }
    /** Refresh edited identity and save availability without replacing active body controls. */
    edited(title) {
        presentCubeModelTitle(this.#title, title);
        if (this.#save)
            this.#save.disabled = false;
    }
    /** Disable repeated submissions until the draft owner publishes the next save state. */
    #createSave(document, options) {
        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'p-button p-component sugarcubes-cube-editor-metadata__save';
        button.setAttribute('aria-label', 'Save');
        button.title = 'Save Cube';
        button.append(createComfyPrimeIconElement(document, 'save'));
        button.disabled = !options.saveEnabled;
        button.addEventListener('click', () => {
            button.disabled = true;
            options.save();
        });
        return button;
    }
    /** Expose the draft owner's explicit roll-up action in either disclosure state. */
    #createCollapse(document, options) {
        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'p-button p-component sugarcubes-cube-editor-metadata__collapse';
        const label = options.collapsed ? 'Expand' : 'Minimize';
        button.setAttribute('aria-label', label);
        button.title = label;
        button.append(createComfyPrimeIconElement(document, options.collapsed ? 'chevron-down' : 'chevron-up'));
        button.setAttribute('aria-expanded', String(!options.collapsed));
        button.addEventListener('click', options.toggle);
        return button;
    }
}

import { ContextMenu } from '../../menus/ContextMenu.js';
/** Keep toolbar controls stable while reconciling automatically discovered participant groups. */
export class GlobalOverrideToolbar {
    document;
    relations;
    reportError;
    widgets;
    attach;
    element;
    #controls = new Map();
    #menu;
    #menuButton;
    #labels;
    #signature = '';
    /** Bind toolbar edits directly to the authoritative application owner. */
    constructor(document, relations, reportError, widgets, attach, presentation = 'comfy') {
        this.document = document;
        this.relations = relations;
        this.reportError = reportError;
        this.widgets = widgets;
        this.attach = attach;
        this.element = document.createElement('div');
        this.element.className = 'sc-overrides-toolbar';
        this.element.setAttribute('role', 'toolbar');
        this.element.setAttribute('aria-label', 'Global overrides');
        this.setPresentation(presentation);
        this.#menu = new ContextMenu(document);
        this.#menuButton = document.createElement('button');
        this.#menuButton.type = 'button';
        this.#menuButton.className = 'sc-overrides-menu';
        this.#menuButton.setAttribute('aria-label', 'Set Global Override');
        this.#menuButton.title = 'Set Global Override';
        this.#menuButton.setAttribute('aria-haspopup', 'menu');
        this.#menuButton.setAttribute('aria-expanded', 'false');
        const icon = document.createElement('span');
        icon.className = 'icon-[lucide--pin]';
        icon.setAttribute('aria-hidden', 'true');
        const chevron = document.createElement('span');
        chevron.className = 'icon-[lucide--chevron-down]';
        chevron.setAttribute('aria-hidden', 'true');
        this.#menuButton.append(icon, chevron);
        this.#menuButton.addEventListener('click', () => {
            if (this.#menu.isOpen()) {
                this.#menu.close();
                return;
            }
            this.#menuButton.setAttribute('aria-expanded', 'true');
            this.#menu.open({
                label: 'Set Global Override',
                trigger: this.#menuButton,
                anchor: { kind: 'below', bounds: this.#menuButton.getBoundingClientRect() },
                closed: () => this.#menuButton.setAttribute('aria-expanded', 'false'),
                entries: relations.snapshot().groups.map((group) => ({
                    label: group.label,
                    checked: group.enabled,
                    selection: 'multiple',
                    select: () => this.#run(() => relations.setOverrideEnabled(group.key, !group.enabled)),
                })),
            });
        });
        this.element.append(this.#menuButton);
        this.#labels = new MutationObserver(this.#measureLabels);
        this.#labels.observe(this.element, { childList: true, subtree: true, characterData: true });
    }
    /** Let an embedding host select its chrome while retaining mounted controls and menu identity. */
    setPresentation(presentation) {
        this.element.dataset.presentation = presentation;
    }
    /** Reattach after native shell replacement and update only changed controls. */
    reconcile() {
        if (!this.attach(this.element))
            return;
        const groups = this.relations.snapshot().groups;
        this.element.hidden = groups.length === 0;
        const signature = JSON.stringify(groups);
        if (signature === this.#signature)
            return;
        this.#signature = signature;
        for (const [key, control] of this.#controls)
            if (!groups.some((group) => group.key === key && group.enabled)) {
                control.dispose();
                this.#controls.delete(key);
            }
        for (const group of groups.filter((item) => item.enabled)) {
            let control = this.#controls.get(group.key);
            if (!control) {
                control = this.#createControl(group);
                this.#controls.set(group.key, control);
            }
            control.update(group);
        }
        let preceding = this.#menuButton;
        for (const group of groups.filter((item) => item.enabled)) {
            const root = this.#controls.get(group.key)?.root;
            if (!root)
                continue;
            if (root.previousElementSibling !== preceding)
                this.element.insertBefore(root, preceding.nextSibling);
            preceding = root;
        }
    }
    /** Release menus and owned toolbar DOM without modifying workflow state. */
    dispose() {
        this.#labels.disconnect();
        for (const control of this.#controls.values())
            control.dispose();
        this.#controls.clear();
        this.#menu.close();
        this.element.remove();
    }
    /** Keep label width fixed while each asynchronously mounted native control surrenders its surplus. */
    #measureLabels = () => {
        for (const control of this.#controls.values()) {
            const label = control.root.querySelector('[data-testid="widget-layout-field-label"]');
            if (label)
                control.root.style.setProperty('--sc-override-label-width', `${label.getBoundingClientRect().width}px`);
        }
    };
    /** Mount the same native widget component used on node cards against the shared value owner. */
    #createControl(group) {
        const root = this.document.createElement('div');
        root.className = 'sc-override-control';
        root.dataset.semantic = group.members[0]?.semantic ?? '';
        root.dataset.overrideKey = group.key;
        const mount = this.widgets.mount(root, group, (value) => this.#run(() => this.relations.setOverrideValue(group.key, value)), (mode) => this.#run(() => this.relations.setSeedMode(group.key, mode)));
        return {
            root,
            update: (next) => {
                root.title = `${next.members.length} fields share this value`;
                mount.update(next);
            },
            dispose: () => {
                mount.dispose();
                root.remove();
            },
        };
    }
    /** Report invalid edits through existing SugarCubes feedback and restore validated presentation. */
    #run(operation) {
        try {
            operation();
        }
        catch (error) {
            this.reportError(error);
            this.#signature = '';
            this.reconcile();
        }
    }
}

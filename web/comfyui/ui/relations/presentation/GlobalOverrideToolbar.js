import { ContextMenu } from '../../menus/ContextMenu.js';
import { OverrideToolbarDisclosure, } from './OverrideToolbarDisclosure.js';
import { OverrideToolbarDrag } from './OverrideToolbarDrag.js';
import { OverrideToolbarPlacement } from './OverrideToolbarPlacement.js';
import { OverrideToolbarFieldLayout } from './OverrideToolbarFieldLayout.js';
/** Keep toolbar controls stable while reconciling automatically discovered participant groups. */
export class GlobalOverrideToolbar {
    document;
    relations;
    reportError;
    widgets;
    element;
    #controls = new Map();
    #menu;
    #menuButton;
    #labels;
    #fields;
    #fieldLayout;
    #disclosure;
    #drag;
    #placement;
    #signature = '';
    /** Bind toolbar edits directly to the authoritative application owner. */
    constructor(document, relations, reportError, widgets, host, stateStore, presentation = 'comfy') {
        this.document = document;
        this.relations = relations;
        this.reportError = reportError;
        this.widgets = widgets;
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
        this.#menuButton.addEventListener('click', () => this.openMenu(this.#menuButton));
        const grip = document.createElement('button');
        grip.type = 'button';
        grip.className = 'sc-overrides-grip';
        grip.setAttribute('aria-label', 'Move global overrides');
        grip.title =
            'Drag onto the Run dock target; Enter toggles docking; arrow keys move, Home resets';
        const gripIcon = document.createElement('span');
        gripIcon.className = 'drag-handle h-max w-3 cursor-grab';
        gripIcon.setAttribute('aria-hidden', 'true');
        grip.append(gripIcon);
        this.#fields = document.createElement('div');
        this.#fields.className = 'sc-overrides-fields';
        this.#fields.dataset.scScrollbarHorizontalOutset = '3';
        this.#fieldLayout = new OverrideToolbarFieldLayout(this.#fields);
        this.#fields.id = 'sc-overrides-fields';
        const disclosure = document.createElement('button');
        disclosure.type = 'button';
        disclosure.className = 'sc-overrides-disclosure';
        disclosure.setAttribute('aria-controls', this.#fields.id);
        disclosure.addEventListener('click', () => {
            this.#menu.close();
            queueMicrotask(this.#measureLabels);
        });
        this.element.append(grip, this.#menuButton, this.#fields, disclosure);
        this.#placement = new OverrideToolbarPlacement(this.element, grip, host, (docked) => this.#disclosure.setExpansionDirection(docked ? 'left' : 'right'), () => this.#fieldLayout.reconcile(), stateStore);
        this.#drag = new OverrideToolbarDrag(this.element, grip, this.#placement, () => {
            this.#menu.close();
            this.#disclosure.finish();
        });
        this.#disclosure = new OverrideToolbarDisclosure(this.element, this.#fields, disclosure, (width, height) => {
            this.#fieldLayout.reconcile();
            this.#placement.resize(width, height);
        }, reportError, stateStore);
        this.#labels = new MutationObserver(this.#measureLabels);
        this.#labels.observe(this.element, { childList: true, subtree: true, characterData: true });
    }
    /** Anchor the same override menu to either native toolbar or Substitute frame controls. */
    openMenu(trigger) {
        if (this.#menu.isOpen()) {
            this.#menu.close();
            return;
        }
        trigger.setAttribute('aria-expanded', 'true');
        this.#menu.open({
            label: 'Set Global Override',
            trigger,
            anchor: { kind: 'below', bounds: trigger.getBoundingClientRect() },
            closed: () => trigger.setAttribute('aria-expanded', 'false'),
            entries: this.relations.snapshot().groups.map((group) => ({
                label: group.label,
                checked: group.enabled,
                selection: 'multiple',
                select: () => this.#run(() => this.relations.setOverrideEnabled(group.key, !group.enabled)),
            })),
        });
    }
    /** Let an embedding host select its chrome while retaining mounted controls and menu identity. */
    setPresentation(presentation) {
        this.element.dataset.presentation = presentation;
    }
    /** Reattach after native shell replacement and update only changed controls. */
    reconcile() {
        if (!this.#placement.attached())
            return;
        this.#disclosure.refresh();
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
        let preceding = null;
        for (const group of groups.filter((item) => item.enabled)) {
            const root = this.#controls.get(group.key)?.root;
            if (!root)
                continue;
            if (root.parentElement !== this.#fields || root.previousElementSibling !== preceding)
                this.#fields.insertBefore(root, preceding ? preceding.nextSibling : this.#fields.firstChild);
            preceding = root;
        }
    }
    /** Release menus and owned toolbar DOM without modifying workflow state. */
    dispose() {
        this.#labels.disconnect();
        this.#drag.dispose();
        this.#placement.dispose();
        this.#disclosure.dispose();
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
            const width = label?.getBoundingClientRect().width ?? 0;
            if (width > 0)
                control.root.style.setProperty('--sc-override-label-width', `${width}px`);
        }
        this.#disclosure.refresh();
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

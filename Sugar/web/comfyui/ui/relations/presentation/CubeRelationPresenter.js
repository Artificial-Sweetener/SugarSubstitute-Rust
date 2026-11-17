import { canLinkField, canLinkNode, compatibleNodes, linkedFieldMembers, } from '../domain/RelationResolver.js';
import { choiceRestrictions } from '../domain/FieldCompatibility.js';
import { prepareNativePresentation } from '../../surface/NativePresentationReady.js';
import { setNativeFieldSuppressed } from '../../surface/NativeFieldPresentation.js';
/** Reconcile chrome independently from native card construction and compatibility policy. */
export class CubeRelationPresenter {
    document;
    relations;
    getViews;
    reportError;
    widgets;
    #choices = new Map();
    #hidden = new Set();
    /** Receive identified native view boundaries through an explicit adapter. */
    constructor(document, relations, getViews, reportError, widgets) {
        this.document = document;
        this.relations = relations;
        this.getViews = getViews;
        this.reportError = reportError;
        this.widgets = widgets;
    }
    /** Add selectors only where a compatible peer exists or an existing link needs display. */
    reconcile() {
        const hidden = new Set();
        const snapshot = this.relations.snapshot(), views = this.getViews(), live = new Set(views.map((view) => view.surface.root));
        for (const [root, choices] of this.#choices)
            if (!live.has(root) || !root.isConnected) {
                for (const mounted of choices.values())
                    this.#release(mounted);
                this.#choices.delete(root);
            }
        for (const view of views) {
            const node = snapshot.nodes.find((candidate) => candidate.id === view.nodeId);
            const fields = snapshot.fields.filter((field) => field.nodeId === view.nodeId);
            const mounted = this.#choices.get(view.surface.root) ?? new Map();
            this.#choices.set(view.surface.root, mounted);
            const retain = new Set();
            if (node)
                this.#node(node, view.surface, mounted, retain);
            for (const field of fields)
                this.#field(field, view.surface, mounted, retain, hidden);
            for (const [key, choice] of mounted)
                if (!retain.has(key)) {
                    this.#release(choice);
                    mounted.delete(key);
                }
        }
        for (const element of this.#hidden)
            if (!hidden.has(element))
                setNativeFieldSuppressed(element, false);
        for (const element of hidden)
            if (!this.#hidden.has(element))
                setNativeFieldSuppressed(element, true);
        this.#hidden = hidden;
    }
    /** Restore native presentation and release all owned link controls. */
    dispose() {
        for (const choices of this.#choices.values())
            for (const mounted of choices.values())
                this.#release(mounted);
        this.#choices.clear();
        for (const element of this.#hidden)
            setNativeFieldSuppressed(element, false);
        this.#hidden.clear();
    }
    /** Put compatible clone targets into the native titlebar's trailing flex row. */
    #node(node, surface, mounted, retain) {
        const snapshot = this.relations.snapshot(), peers = snapshot.nodes.filter((peer) => compatibleNodes(node, peer));
        const earlier = peers.filter((peer) => snapshot.nodes.indexOf(peer) < snapshot.nodes.indexOf(node));
        const targets = earlier.filter((peer) => canLinkNode(node, peer, snapshot.nodes, snapshot.state));
        const source = snapshot.state.nodeLinks[node.id];
        if (!earlier.length && !source && !mounted.has('node'))
            return;
        const target = surface.header?.firstElementChild;
        if (!(target instanceof HTMLElement) && !surface.canvasTitle)
            return;
        const choice = this.#choice('node', mounted, `Link ${node.label}`, (value) => this.relations.linkNode(node.id, value || null));
        choice.root.classList.add('sc-node-link');
        const first = snapshot.nodes.find((candidate) => candidate.id === node.id || peers.includes(candidate));
        choice.root.hidden = first?.id === node.id;
        const options = [
            { value: '', label: 'Independent', blockers: [] },
            ...targets.map((peer) => ({
                value: peer.id,
                label: `🔗 ${peer.cubeLabel}`,
                blockers: [],
            })),
        ];
        choice.choice.update(source ?? '', options, false, [
            'Independent',
            ...[node, ...peers].map((candidate) => `🔗 ${candidate.cubeLabel}`),
        ]);
        if (target instanceof HTMLElement) {
            const activation = target.querySelector('.sugarcubes-cube-face__activation');
            if (choice.root.parentElement !== target ||
                (activation && choice.root.nextElementSibling !== activation)) {
                target.insertBefore(choice.root, activation);
                prepareNativePresentation(choice.root);
            }
        }
        else {
            choice.root.classList.add('sc-relation-canvas-header');
            if (surface.canvas)
                surface.root.style.position = 'relative';
            if (choice.root.parentElement !== surface.root) {
                surface.root.append(choice.root);
                prepareNativePresentation(choice.root);
            }
        }
        retain.add('node');
    }
    /** Replace a native combo with its literal and link choices without storing a link label as a value. */
    #field(field, surface, mounted, retain, hidden) {
        const snapshot = this.relations.snapshot(), effective = snapshot.resolved.find((item) => item.field.id === field.id), row = surface.rows.get(field.name), canvasField = surface.canvasFields.get(field.name);
        if (!row && !canvasField)
            return;
        if (effective?.override) {
            if (row && !surface.nativeWidgetVisibility)
                hidden.add(row);
            return;
        }
        if (effective?.source && snapshot.state.nodeLinks[field.nodeId]) {
            if (row && !surface.nativeWidgetVisibility)
                hidden.add(row);
            return;
        }
        const peers = field.cubeId
            ? snapshot.fields.filter((peer) => canLinkField(field, peer, snapshot.fields, snapshot.nodes, snapshot.state))
            : [];
        const members = linkedFieldMembers(field.id, snapshot.fields, snapshot.nodes, snapshot.state);
        if (field.domain.kind !== 'choice' || (!peers.length && members.length < 2))
            return;
        const key = field.id, source = snapshot.state.fieldLinks[field.id];
        const choice = this.#choice(key, mounted, `${field.name} link or value`, (value) => {
            if (value.startsWith('link:'))
                this.relations.linkField(field.id, value.slice(5));
            else {
                if (this.relations.snapshot().state.fieldLinks[field.id])
                    this.relations.linkField(field.id, null);
                this.relations.setFieldValue(field.id, value);
            }
        }, field.name);
        const control = row
            ? (row.querySelector('[data-sc-model-native]') ??
                row.querySelector('[data-sc-style-combo-frame]:not(.sc-model-field)') ??
                row
                    .querySelector('[data-testid="widget-select-default-trigger"]')
                    ?.closest('[data-capture-wheel]') ??
                row.lastElementChild)
            : null;
        if (control instanceof HTMLElement) {
            if (choice.root.parentElement !== control.parentElement) {
                control.parentElement?.append(choice.root);
                prepareNativePresentation(choice.root);
            }
            hidden.add(control);
        }
        else if (canvasField) {
            surface.root.style.position = surface.canvas ? 'relative' : surface.root.style.position;
            choice.root.classList.add('sc-relation-canvas-field');
            Object.assign(choice.root.style, {
                top: `${canvasField.top}px`,
                left: `${canvasField.left}px`,
                width: `${canvasField.width}px`,
                height: `${canvasField.height}px`,
            });
            if (choice.root.parentElement !== surface.root) {
                surface.root.append(choice.root);
                prepareNativePresentation(choice.root);
            }
        }
        else
            return;
        const restrictions = choiceRestrictions(members.length > 1 ? members : [field]);
        choice.choice.update(source ? `link:${source}` : String(effective?.value ?? field.value), [
            ...restrictions.map((item) => ({ ...item, label: item.value })),
            ...peers.map((peer) => ({
                value: `link:${peer.id}`,
                label: `🔗 ${peer.label}`,
                blockers: [],
            })),
        ], Boolean(effective?.source && !source));
        retain.add(key);
    }
    /** Maintain stable choice identity through native redraws. */
    #choice(key, mounted, label, operation, nativeInputName) {
        let existing = mounted.get(key);
        if (!existing) {
            const root = this.document.createElement('div');
            root.className = 'sc-field-relation';
            const choice = this.widgets.mountChoice(root, label, (value) => {
                try {
                    operation(value);
                }
                catch (error) {
                    this.reportError(error);
                }
            }, nativeInputName);
            existing = { root, choice };
            mounted.set(key, existing);
        }
        return existing;
    }
    /** Release the link control while the visibility diff restores its native row. */
    #release(mounted) {
        mounted.choice.dispose();
        mounted.root.remove();
    }
}

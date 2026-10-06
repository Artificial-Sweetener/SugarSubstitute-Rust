import { commonFieldDomain } from '../domain/FieldCompatibility.js';
import { advanceSharedSeeds } from '../domain/SharedSeedAdvancement.js';
import { resolveLinkedNodeModes } from '../domain/LinkedNodeState.js';
import { canLinkField, canLinkNode, discoverOverrides, initializeNodeLinks, linkedFieldMembers, resolveRelations, validateSharedEdit, } from '../domain/RelationResolver.js';
/** Keep user intentions authoritative while projecting their shared editable values. */
export class WorkflowRelations {
    port;
    #snapshot = null;
    #inventory = null;
    #reconciling = false;
    #listeners = new Set();
    /** Bind one typed workflow adapter rather than accessing native graphs from policy. */
    constructor(port) {
        this.port = port;
    }
    /** Discover on topology changes; reuse validated bindings for value-only edits. */
    reconcile(discover = true) {
        if (this.#reconciling && this.#snapshot)
            return this.#snapshot;
        this.#reconciling = true;
        try {
            const bindings = discover || !this.#inventory ? this.port.inventory() : this.#inventory, state = this.port.load();
            this.#inventory = bindings;
            const fields = bindings.fields.map((field) => ({ ...field.contract, value: field.read() }));
            const inventory = {
                ...bindings,
                nodes: bindings.nodes.map((node) => ({
                    ...node,
                    fields: fields.filter((field) => field.nodeId === node.id),
                    mode: bindings.editableNodes?.find((editable) => editable.id === node.id)?.readMode?.() ??
                        node.mode ??
                        0,
                })),
            };
            const previousSnapshot = this.#snapshot;
            this.#repairMembership(inventory, state);
            initializeNodeLinks(inventory.nodes, state);
            const groups = discoverOverrides(fields, state);
            for (const group of groups)
                state.overrides[group.key] = {
                    enabled: group.enabled,
                    value: group.value,
                    seedMode: group.seedMode,
                };
            let resolved = resolveRelations(fields, inventory.nodes, state, groups);
            for (const item of resolved) {
                const controlled = item.source !== null || item.override !== null;
                if (controlled && !Object.hasOwn(state.locals, item.field.id))
                    state.locals[item.field.id] = item.field.value;
                if (!controlled && Object.hasOwn(state.locals, item.field.id)) {
                    const previous = this.#snapshot?.resolved.find((candidate) => candidate.field.id === item.field.id);
                    if (previous &&
                        previous.source === null &&
                        previous.override === null &&
                        item.field.value !== previous.value)
                        state.locals[item.field.id] = item.field.value;
                }
            }
            resolved = resolveRelations(fields, inventory.nodes, state, groups);
            for (const node of inventory.nodes) {
                const previous = this.#snapshot?.nodes.find((candidate) => candidate.id === node.id);
                if (!state.nodeLinks[node.id] &&
                    !this.#snapshot?.state.nodeLinks[node.id] &&
                    previous &&
                    Object.hasOwn(state.localNodeModes, node.id) &&
                    node.mode !== previous.mode)
                    state.localNodeModes[node.id] = node.mode ?? 0;
            }
            const nodeModes = resolveLinkedNodeModes(inventory.nodes, state);
            this.#snapshot = { fields, nodes: inventory.nodes, groups, resolved, state };
            const changedNodes = new Set();
            for (const item of resolved) {
                const previous = previousSnapshot?.resolved.find((field) => field.field.id === item.field.id);
                if (item.value !== item.field.value ||
                    (previous &&
                        (previous.value !== item.value ||
                            previous.source !== item.source ||
                            previous.override !== item.override)))
                    changedNodes.add(item.field.nodeId);
            }
            for (const node of inventory.nodes)
                if (nodeModes.get(node.id) !== (node.mode ?? 0))
                    changedNodes.add(node.id);
            const changed = changedNodes.size > 0;
            if (changed)
                this.port.transaction(() => {
                    this.#apply(inventory, resolved);
                    for (const node of inventory.editableNodes ?? []) {
                        const mode = nodeModes.get(node.id);
                        if (mode !== undefined &&
                            inventory.nodes.find((candidate) => candidate.id === node.id)?.mode !== mode)
                            node.writeMode(mode);
                    }
                    this.port.save(state);
                });
            else
                this.port.save(state);
            if (changed)
                this.port.refresh([...changedNodes]);
            for (const listener of this.#listeners)
                listener();
            return this.#snapshot;
        }
        finally {
            this.#reconciling = false;
        }
    }
    /** Return a validated snapshot without depending on mounted card elements. */
    snapshot() {
        return this.#snapshot ?? this.reconcile();
    }
    /** Subscribe presentation to one relation-state owner. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Enable or disable one discovered override while retaining any underlying field links. */
    setOverrideEnabled(key, enabled) {
        this.#edit((state) => {
            const group = this.snapshot().groups.find((candidate) => candidate.key === key);
            if (!group)
                throw new Error('The override no longer has eligible fields.');
            state.overrides[key] = { enabled, value: group.value, seedMode: group.seedMode };
        });
    }
    /** Validate one toolbar value against every member before making one undoable edit. */
    setOverrideValue(key, value) {
        this.#edit((state) => {
            const group = this.snapshot().groups.find((candidate) => candidate.key === key);
            validateSharedEdit(group?.domain ?? null, value);
            if (group)
                state.overrides[key] = { enabled: true, value, seedMode: group.seedMode };
        });
    }
    /** Keep generation policy on the shared seed rather than independently on each participant. */
    setSeedMode(key, seedMode) {
        this.#edit((state) => {
            const override = state.overrides[key];
            if (override)
                override.seedMode = seedMode;
        });
    }
    /** Link one configurable field to another Cube while rejecting incompatible groups and cycles. */
    linkField(targetId, sourceId) {
        this.#edit((state) => {
            const snapshot = this.snapshot(), target = snapshot.fields.find((field) => field.id === targetId), source = snapshot.fields.find((field) => field.id === sourceId);
            if (sourceId !== null &&
                (!target ||
                    !source ||
                    !canLinkField(target, source, snapshot.fields, snapshot.nodes, state)))
                throw new Error('These fields cannot share a compatible value.');
            state.fieldLinks[targetId] = sourceId;
        });
    }
    /** Clone a compatible node's eligible configuration while retaining the follower's graph links. */
    linkNode(targetId, sourceId) {
        this.#edit((state) => {
            const snapshot = this.snapshot(), target = snapshot.nodes.find((node) => node.id === targetId), source = snapshot.nodes.find((node) => node.id === sourceId);
            if (sourceId !== null &&
                (!target || !source || !canLinkNode(target, source, snapshot.nodes, state)))
                throw new Error('These nodes cannot share a compatible configuration without creating a cycle.');
            state.nodeLinks[targetId] = sourceId;
        });
    }
    /** Edit the locally owned master while honoring the complete group's common value domain. */
    setFieldValue(id, value) {
        this.#edit((state) => {
            const snapshot = this.snapshot(), field = snapshot.fields.find((candidate) => candidate.id === id), effective = snapshot.resolved.find((item) => item.field.id === id);
            if (!field || effective?.source || effective?.override)
                throw new Error('Edit this field through its linked master or global override.');
            validateSharedEdit(commonFieldDomain(linkedFieldMembers(id, snapshot.fields, snapshot.nodes, state)), value);
            state.locals[id] = value;
            this.#inventory?.fields.find((candidate) => candidate.contract.id === id)?.write(value);
        });
    }
    /** Advance each shared seed exactly once before a queue request is projected. */
    advanceSeeds(random) {
        const snapshot = this.reconcile();
        this.#edit((state) => advanceSharedSeeds(snapshot.fields, snapshot.nodes, snapshot.groups, state, random));
    }
    /** Apply one explicit intent and let the same resolver project its effects. */
    #edit(operation) {
        this.reconcile(false);
        this.port.transaction(() => {
            const state = this.port.load();
            operation(state);
            this.port.save(state);
            this.reconcile(false);
        });
    }
    /** Promote surviving masters using their effective value instead of dormant local configuration. */
    #repairMembership(inventory, state) {
        const nodeIds = new Set(inventory.nodes.map((node) => node.id)), fieldIds = new Set(inventory.fields.map((field) => field.contract.id));
        for (const [links, ids] of [
            [state.nodeLinks, nodeIds],
            [state.fieldLinks, fieldIds],
        ]) {
            const missingSources = new Set(Object.values(links).filter((id) => id !== null && !ids.has(id)));
            for (const missing of missingSources) {
                const survivors = Object.keys(links).filter((id) => ids.has(id) && links[id] === missing);
                const replacement = survivors[0];
                if (replacement) {
                    links[replacement] = null;
                    for (const bound of inventory.fields)
                        if (bound.contract.id === replacement || bound.contract.nodeId === replacement)
                            state.locals[bound.contract.id] = bound.read();
                    const promoted = inventory.nodes.find((node) => node.id === replacement);
                    if (promoted)
                        state.localNodeModes[replacement] = promoted.mode ?? 0;
                }
                for (const id of survivors.slice(1))
                    links[id] = replacement ?? null;
            }
            for (const id of Object.keys(links))
                if (!ids.has(id))
                    Reflect.deleteProperty(links, id);
        }
        for (const id of Object.keys(state.locals))
            if (!fieldIds.has(id))
                Reflect.deleteProperty(state.locals, id);
    }
    /** Project only validated editable fields; native connections are never mutated. */
    #apply(inventory, resolved) {
        const values = new Map(resolved.map((item) => [item.field.id, item.value]));
        for (const field of inventory.fields) {
            const value = values.get(field.contract.id);
            if (value !== undefined && value !== field.read())
                field.write(value);
        }
    }
}

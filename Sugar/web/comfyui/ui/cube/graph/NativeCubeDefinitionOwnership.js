import { isNativeCubeSubgraph } from '../ComfyCubeRuntimeGraphBoundary.js';
import { isRecord } from '../../types/common.js';
import { cloneNativeCubeDefinition } from './NativeCubeDefinitionClone.js';
import { transferCubeInstanceState } from '../node/CubeInstanceStateTransfer.js';
import { isCubeNode } from '../node/ComfyCubeNodeFactory.js';
/** Isolate host-shared root and nested definitions before Cube consumers discover editable owners. */
export class NativeCubeDefinitionOwnership {
    graph;
    createId;
    discardDefinition;
    /** Receive native registration and identity generation from the Cube lifecycle boundary. */
    constructor(graph, createId, discardDefinition) {
        this.graph = graph;
        this.createId = createId;
        this.discardDefinition = discardDefinition;
    }
    /** Retain Cube root identities, slots and connections while separating copied mutable contents. */
    reconcile(cubes) {
        const owners = new Map();
        const assignments = [];
        const created = [];
        const visit = (node, owner) => {
            const original = node.subgraph;
            if (!isNativeCubeSubgraph(original))
                return;
            const previous = owners.get(original);
            if (previous === owner)
                return;
            let definition = original;
            if (previous) {
                definition = this.#clone(original, created);
                const originalSurface = node.properties?.sugarcubes_surface;
                assignments.push({ node, original, originalType: node.type, originalSurface });
                const source = isCubeNode(node)
                    ? { ...node, properties: { ...node.properties }, subgraph: original }
                    : null;
                this.#assign(node, definition);
                if (source && isCubeNode(node)) {
                    node.properties.sugarcubes_surface = {};
                    transferCubeInstanceState(source, node);
                }
            }
            owners.set(definition, owner);
            for (const child of definition._nodes)
                visit(child, owner);
        };
        try {
            for (const cube of cubes)
                visit(cube, cube);
        }
        catch (error) {
            const failures = [error];
            for (const assignment of assignments.reverse()) {
                try {
                    this.#assign(assignment.node, assignment.original);
                    if (assignment.originalType === undefined)
                        Reflect.deleteProperty(assignment.node, 'type');
                    else
                        assignment.node.type = assignment.originalType;
                    if (assignment.node.properties) {
                        if (assignment.originalSurface === undefined)
                            Reflect.deleteProperty(assignment.node.properties, 'sugarcubes_surface');
                        else
                            assignment.node.properties.sugarcubes_surface = assignment.originalSurface;
                    }
                }
                catch (restoreError) {
                    failures.push(restoreError);
                }
            }
            for (const id of created) {
                try {
                    this.discardDefinition(id);
                }
                catch (discardError) {
                    failures.push(discardError);
                }
            }
            if (failures.length > 1)
                throw new AggregateError(failures, 'Cube definition isolation could not be fully restored.');
            throw error;
        }
    }
    /** Register a fully serialized native copy through Comfy's public definition-creation API. */
    #clone(original, created) {
        const create = Reflect.get(this.graph, 'createSubgraph');
        const registry = Reflect.get(this.graph, 'subgraphs');
        if (typeof create !== 'function' || !(registry instanceof Map) || !original.asSerialisable)
            throw new TypeError('Comfy native definition isolation is unavailable for copied Cubes.');
        const data = cloneNativeCubeDefinition(original, this.createId);
        const id = this.createId().trim();
        if (!id || registry.has(id))
            throw new Error('Copied Cube definition identity is unavailable.');
        data.id = id;
        created.push(id);
        const definition = create.call(this.graph, data);
        if (!isNativeCubeSubgraph(definition) || definition === original || definition.id !== id)
            throw new TypeError('Comfy did not create an independent native Cube definition.');
        definition.configure(data);
        return definition;
    }
    /** Rebind native promoted slots after changing the definition while preserving external links. */
    #assign(node, definition) {
        node.subgraph = definition;
        node.type = definition.id;
        // Comfy publishes promoted binding refresh on SubgraphNode, but keeps slot rebinding internal.
        const rebind = Reflect.get(node, '_internalConfigureAfterSlots');
        if (typeof rebind === 'function')
            rebind.call(node);
        else if (isRecord(node) && typeof node.rebuildInputWidgetBindings === 'function')
            node.rebuildInputWidgetBindings.call(node);
    }
}

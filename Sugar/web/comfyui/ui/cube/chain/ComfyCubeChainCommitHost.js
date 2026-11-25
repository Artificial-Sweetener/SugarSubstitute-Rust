import { readNativeCubePosition, writeNativeCubePosition, restoreNativeCubePosition, } from '../graph/NativeCubePositions.js';
import { captureNativeCubeConnections, readNativeCubeConnection, restoreNativeCubeConnections, } from '../graph/NativeCubeConnections.js';
import { readInstanceId } from '../node/CubeNodeCatalog.js';
/** Keep construction, removal, geometry, wiring and their compensation at one host boundary. */
export class ComfyCubeChainCommitHost {
    #options;
    /** Bind the root graph, Cube inventory, history and construction owners. */
    constructor(options) {
        this.#options = options;
    }
    /** Apply the whole change and return the Cubes it added, or restore everything and throw. */
    commit(change) {
        const { graph, nodes, construction, definitions, history } = this.#options;
        const original = nodes.list();
        const byInstance = new Map(original.map((node) => [readInstanceId(node), node]));
        const removed = change.removals.map((id) => {
            const node = byInstance.get(id);
            if (!node)
                throw new Error('A Cube to remove is no longer in the workflow.');
            return node;
        });
        const positions = new Map(original.map((node) => [node, readNativeCubePosition(node.pos)]));
        if (isUnchanged(change, byInstance))
            return [];
        const severed = captureNativeCubeConnections(graph, removed);
        const staged = [];
        const inserted = [];
        const unlinked = [];
        const linked = [];
        history.beforeChange();
        try {
            nodes.batch(() => {
                try {
                    for (const creation of change.creations) {
                        const owned = definitions.stage(creation.payload);
                        let constructed;
                        try {
                            constructed = construction.construct(owned.payload, {
                                instanceId: creation.instanceId,
                                instanceAlias: creation.alias,
                                position: [...(change.positions.get(creation.instanceId) ?? [0, 0])],
                                size: [...creation.size],
                            });
                        }
                        catch (error) {
                            try {
                                definitions.discard(owned);
                            }
                            catch (failure) {
                                throw new AggregateError([error, failure], 'Cube construction failed and its definitions could not be cleaned up.');
                            }
                            throw error;
                        }
                        staged.push({ definitions: owned, constructed });
                        if (graph.getNodeById(constructed.node.id))
                            throw new Error('A new Cube’s native node id is already in use.');
                        byInstance.set(creation.instanceId, constructed.node);
                    }
                    for (const node of [
                        ...staged.map(({ constructed }) => constructed.node),
                        ...change.additions.map((addition) => addition.node),
                    ]) {
                        inserted.push(node);
                        graph.add(node);
                        nodes.add(node);
                        byInstance.set(readInstanceId(node), node);
                    }
                    for (const node of removed) {
                        graph.remove(node);
                        nodes.remove(readInstanceId(node));
                    }
                    for (const [instanceId, position] of change.positions) {
                        const node = byInstance.get(instanceId);
                        if (!node)
                            throw new Error('A Cube to place is no longer in the workflow.');
                        writeNativeCubePosition(node, position);
                    }
                    for (const link of change.links.remove) {
                        const target = requireNode(byInstance, link.targetInstance);
                        const id = target.inputs[link.targetSlot]?.link;
                        if (id === null || id === undefined)
                            continue;
                        unlinked.push(readNativeCubeConnection(graph, id));
                        target.disconnectInput(link.targetSlot);
                    }
                    for (const link of change.links.add) {
                        const source = requireNode(byInstance, link.sourceInstance);
                        const target = requireNode(byInstance, link.targetInstance);
                        source.connect(link.sourceSlot, target, link.targetSlot);
                        const id = target.inputs[link.targetSlot]?.link;
                        if (id === null || id === undefined)
                            throw new Error(`${target.title ?? 'A Cube'} could not be wired.`);
                        linked.push(readNativeCubeConnection(graph, id));
                    }
                    this.#options.reconcile();
                    this.#options.announce();
                    const first = inserted[0] ?? original[0];
                    if (first)
                        nodes.changed(first);
                }
                catch (error) {
                    compensate(error, [
                        ...linked.map((connection) => () => connection.target.disconnectInput(connection.targetSlot)),
                        () => restoreNativeCubeConnections(graph, unlinked),
                        ...[...inserted].reverse().map((node) => () => {
                            if (graph.getNodeById(node.id) === node)
                                graph.remove(node);
                            nodes.remove(readInstanceId(node));
                        }),
                        ...removed.map((node) => () => {
                            if (graph.getNodeById(node.id) !== node)
                                graph.add(node);
                            nodes.add(node);
                        }),
                        ...[...positions].map(([node, position]) => () => restoreNativeCubePosition(node, position)),
                        () => restoreNativeCubeConnections(graph, severed),
                        ...[...staged]
                            .reverse()
                            .flatMap((item) => [
                            () => construction.discard(item.constructed),
                            () => definitions.discard(item.definitions),
                        ]),
                    ]);
                }
            });
        }
        finally {
            try {
                history.setDirtyCanvas(true, true);
            }
            finally {
                history.afterChange();
            }
        }
        return inserted;
    }
}
/** A change that adds, removes and rewires nothing and leaves every Cube where it is. */
function isUnchanged(change, nodes) {
    return (!change.creations.length &&
        !change.additions.length &&
        !change.removals.length &&
        !change.links.remove.length &&
        !change.links.add.length &&
        [...change.positions].every(([id, position]) => {
            const node = nodes.get(id);
            const now = node ? readNativeCubePosition(node.pos) : null;
            return now !== null && now[0] === position[0] && now[1] === position[1];
        }));
}
/** Resolve one Cube of the change by instance id. */
function requireNode(nodes, instanceId) {
    const node = nodes.get(instanceId);
    if (!node)
        throw new Error('A Cube to wire is no longer in the workflow.');
    return node;
}
/** Undo every step that ran, then rethrow the failure with any undo that failed beside it. */
function compensate(error, steps) {
    const failures = [];
    for (const step of steps) {
        try {
            step();
        }
        catch (failure) {
            failures.push(failure);
        }
    }
    if (failures.length)
        throw new AggregateError([error, ...failures], 'A Cube change failed and could not be fully undone.');
    throw error;
}

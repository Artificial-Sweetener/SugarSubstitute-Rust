import { requireCubeIdentity } from '../../cube/node/ComfyCubeNodeFactory.js';
import { orderWorkflowNodes } from '../WorkflowNodeOrder.js';
import { readCubeFaceCardState } from '../CubeFaceCardStateService.js';
import { projectNodeCards } from '../NodeCardProjection.js';
import { cubeGraphOrder } from '../../workflow/domain/CubeGraphOrder.js';
/** Keep real Cubes and consecutive loose nodes in distinct runs of the shared grid. */
export function projectWorkflowMasonry(graph, cubes, connections = [], analysis = null) {
    const cubeSet = new Set(cubes);
    const sections = [];
    const looseMembers = new Map();
    let loose = null;
    for (const node of orderWorkflowNodes(graph, connections)) {
        const id = String(node.id);
        const cube = cubeSet.has(node) ? cubes.find((candidate) => candidate === node) : undefined;
        if (cube) {
            loose = null;
            const identity = requireCubeIdentity(cube);
            sections.push({
                id: `cube:${String(identity.instance_id ?? cube.id)}`,
                label: cube.title?.trim() || 'SugarCube',
                cube,
                target: cube,
                cards: orderedCards(cube.subgraph._nodes, cube.subgraph, `cube:${String(cube.id)}`, cube, cube),
            });
            continue;
        }
        if (!loose) {
            loose = { id: `loose:${id}`, label: 'Workflow nodes', cube: null, target: graph, cards: [] };
            sections.push(loose);
            looseMembers.set(loose.id, []);
        }
        looseMembers.get(loose.id)?.push(node);
    }
    for (const section of sections) {
        if (!section.cube)
            section.cards = orderedCards(looseMembers.get(section.id) ?? [], graph, 'root', graph);
    }
    return orderCanonicalCubeSections(sections, analysis);
}
/** Honor accepted native Cube position order without moving loose runs or recomputing graph policy. */
export function orderCanonicalCubeSections(sections, analysis) {
    const ordered = [...sections];
    if (!analysis)
        return ordered;
    const ids = cubeGraphOrder(analysis).map((id) => `cube:${id}`);
    const members = new Map(ordered.filter((section) => ids.includes(section.id)).map((section) => [section.id, section]));
    if (members.size !== ids.length || ids.some((id) => !members.has(id)))
        return ordered;
    let index = 0;
    for (const [slot, section] of ordered.entries()) {
        if (!members.has(section.id))
            continue;
        const id = ids[index++];
        const replacement = id === undefined ? undefined : members.get(id);
        if (replacement)
            ordered[slot] = replacement;
    }
    return ordered;
}
/** Project the exact Cube-face decisions with namespaced placement identities. */
function orderedCards(nodes, graph, prefix, target, cube) {
    return projectNodeCards(nodes, readCubeFaceCardState(target), graph, cube).map((card) => ({
        ...card,
        id: `${prefix}:${card.id}`,
        columnSpan: card.columnSpan ?? 1,
    }));
}

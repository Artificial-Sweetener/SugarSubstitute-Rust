import { modelFamilyScopes } from '../domain/ModelFamilyScopes.js';
import { recognizeModelField } from '../../model_library/ModelFieldRecognition.js';
/** Share the first projection-ordered generative model across all preset consumers of one panel. */
export function editorModelScopes(card, cards, snapshot) {
    if (!snapshot)
        return [];
    const panel = card.root.closest('.sugarcubes-workflow-masonry, .sugarcubes-cube-face');
    const candidates = (panel ? cards.filter((candidate) => panel.contains(candidate.root)) : [card])
        .filter((candidate) => candidate.root.isConnected)
        .sort(projectionOrder);
    for (const candidate of candidates)
        for (const field of [...candidate.modelInputs].sort((a, b) => a.name.localeCompare(b.name))) {
            const kind = recognizeModelField(snapshot, candidate.nodeType, field.name, field.options(), field.origins);
            if (kind !== 'checkpoints' && kind !== 'diffusion_models')
                continue;
            const value = field.read();
            if (typeof value !== 'string' || !value.trim())
                continue;
            const item = matchingModel(value, snapshot.items.filter((item) => item.kind === kind));
            return modelFamilyScopes(item
                ? {
                    baseModel: item.baseModel,
                    architecture: item.architecture?.base ?? '',
                    title: item.title,
                    tags: item.tags ?? [],
                }
                : null);
        }
    return [];
}
/** Follow authored projection order even when retained DOM mounts were created in another order. */
function projectionOrder(left, right) {
    const order = (card) => {
        const value = card.root.closest('[data-sc-projection-order]')?.dataset
            .scProjectionOrder;
        return value !== undefined && /^\d+$/.test(value) ? Number(value) : Number.MAX_SAFE_INTEGER;
    };
    return (order(left) - order(right) ||
        (left.root.compareDocumentPosition(right.root) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1));
}
/** Match Substitute's normalized backend-path and filename-stem lookup without inferring families from paths. */
function matchingModel(value, items) {
    const normalize = (value) => value.trim().replaceAll('\\', '/').toLowerCase();
    const stem = (value) => normalize(value)
        .split('/')
        .at(-1)
        ?.replace(/\.[^.]+$/, '') ?? '';
    return (items.find((item) => [item.value, item.relativePath].some((path) => normalize(path) === normalize(value))) ??
        items.find((item) => [item.value, item.relativePath].some((path) => stem(path) === stem(value))) ??
        null);
}

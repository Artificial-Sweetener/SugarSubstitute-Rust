//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Rebind persisted subgraph widget values to current fields by stable name. */
import { cloneWidgetValue, isSerializedWidget, readCurrentWidgetValue, } from './WidgetValueSerialization.js';
import { indexSubgraphBoundaryWidgetNames } from './SubgraphBoundaryWidgetIdentities.js';
/** Rebuild subgraph widget arrays against the complete current widget layout. */
export function rebindSubgraphWidgetValues(subgraph, createNode, options = { unavailableNode: 'error' }) {
    if (!Array.isArray(subgraph.nodes)) {
        return subgraph;
    }
    if (typeof createNode !== 'function') {
        throw new Error('Current node factory is unavailable for widget rebinding.');
    }
    const boundaryNamesByNode = indexSubgraphBoundaryWidgetNames(subgraph);
    for (const node of subgraph.nodes) {
        if (!Array.isArray(node?.widgets_values)) {
            continue;
        }
        const liveNode = createNode(node.type || node.class_type);
        if (!liveNode) {
            if (options.unavailableNode === 'preserve')
                continue;
            throw new Error(`Node type '${node.type || node.class_type}' is unavailable.`);
        }
        const liveWidgets = Array.isArray(liveNode.widgets) ? liveNode.widgets : [];
        const boundaryNames = boundaryNamesByNode.get(String(node.id ?? '')) ?? new Set();
        const persistedByName = decodeSerializedWidgetValues(node, liveWidgets, boundaryNames, options.historicalWidgetNames?.(node) ?? null, options.unidentifiedValues ?? 'error');
        node.widgets_values = rebuildWidgetValues(liveWidgets, persistedByName);
    }
    return subgraph;
}
/** Decode a canonical saved array using the field identities stored beside it. */
function decodeSerializedWidgetValues(node, liveWidgets, includedLinkedNames, historicalWidgetNames, unidentifiedValues) {
    const persisted = Array.isArray(node?.widgets_values) ? node.widgets_values : [];
    const candidates = serializedWidgetNameCandidates(node, liveWidgets, persisted, includedLinkedNames, historicalWidgetNames);
    if (!candidates.some((names) => names.length > 0) &&
        persisted.length &&
        linkedWidgetNames(node).size) {
        return new Map();
    }
    let firstError = null;
    for (const names of candidates) {
        try {
            return decodeNamedWidgetValues(node, liveWidgets, names, persisted, 'error');
        }
        catch (error) {
            if (!firstError && error instanceof Error)
                firstError = error;
        }
    }
    if (unidentifiedValues === 'discard' && candidates[0]) {
        return decodeNamedWidgetValues(node, liveWidgets, candidates[0], persisted, 'discard');
    }
    throw firstError ?? new Error(`Serialized node '${node?.id ?? ''}' has no widget identities.`);
}
/** Decode one candidate widget order and require complete positional consumption. */
function decodeNamedWidgetValues(node, liveWidgets, names, persisted, unidentifiedValues) {
    const values = new Map();
    let valueIndex = 0;
    let companionValuesRemaining = Math.max(0, persisted.length - names.length);
    for (const name of names) {
        if (valueIndex >= persisted.length) {
            throw new Error(`Serialized node '${node?.id ?? ''}' is missing widget value '${name}'.`);
        }
        values.set(name, persisted[valueIndex]);
        valueIndex += 1;
        const liveIndex = liveWidgets.findIndex((widget) => widget?.name === name);
        const companion = liveIndex >= 0 ? liveWidgets[liveIndex + 1] : null;
        if (companion &&
            !isSerializedWidget(companion) &&
            companionValuesRemaining > 0 &&
            valueIndex < persisted.length) {
            valueIndex += 1;
            companionValuesRemaining -= 1;
        }
    }
    if (valueIndex !== persisted.length && unidentifiedValues === 'error') {
        throw new Error(`Serialized node '${node?.id ?? ''}' has positional widget values without stable names.`);
    }
    return values;
}
/** Return viable same-snapshot and versioned widget orders without conflating them. */
function serializedWidgetNameCandidates(node, liveWidgets, persisted, includedLinkedNames, historicalWidgetNames) {
    const sameSnapshotNames = readSerializedInputWidgetNames(node);
    const currentNames = filterLinkedWidgetNames(node, includedLinkedNames, sameSnapshotNames);
    const historicalNames = uniqueWidgetNames(node, historicalWidgetNames ?? []);
    const historicalLocalNames = filterLinkedWidgetNames(node, includedLinkedNames, historicalNames);
    const liveNames = liveWidgets.filter(isSerializedWidget).map((widget) => widget.name);
    const liveLocalNames = filterLinkedWidgetNames(node, includedLinkedNames, liveNames);
    const evidencedLiveNames = hasCurrentLayoutEvidence(liveWidgets, historicalNames, persisted)
        ? [liveLocalNames, liveNames]
        : [];
    const currentIdentitiesAreComplete = liveWidgets
        .filter(isSerializedWidget)
        .every((widget) => sameSnapshotNames.includes(widget.name));
    const ordered = currentIdentitiesAreComplete
        ? [currentNames, historicalLocalNames, historicalNames, ...evidencedLiveNames]
        : [historicalLocalNames, historicalNames, ...evidencedLiveNames, currentNames];
    return ordered.filter((names, index) => ordered.findIndex((candidate) => arraysEqual(candidate, names)) === index);
}
/** Detect values authored against a newer live layout than the embedded definition. */
function hasCurrentLayoutEvidence(liveWidgets, historicalNames, persisted) {
    const serializedWidgets = liveWidgets.filter(isSerializedWidget);
    if (historicalNames.length === 0 || serializedWidgets.length !== persisted.length)
        return false;
    return serializedWidgets.some((widget, index) => {
        if (historicalNames.includes(widget.name))
            return false;
        const value = persisted[index];
        const pickerValues = widget.options?.values;
        return (Object.is(value, widget.value) ||
            (Array.isArray(pickerValues) && pickerValues.some((option) => Object.is(option, value))));
    });
}
/** Read every widget identity serialized directly beside the positional values. */
function readSerializedInputWidgetNames(node) {
    const names = [];
    for (const input of Array.isArray(node?.inputs) ? node.inputs : []) {
        const name = typeof input?.widget?.name === 'string' && input.widget.name.trim()
            ? input.widget.name.trim()
            : '';
        if (!name) {
            continue;
        }
        if (names.includes(name)) {
            throw new Error(`Serialized node '${node?.id ?? ''}' has duplicate widget name '${name}'.`);
        }
        names.push(name);
    }
    return names;
}
/** Filter one historical widget order through serialized graph-link ownership. */
function filterLinkedWidgetNames(node, includedLinkedNames, historicalNames) {
    const names = uniqueWidgetNames(node, historicalNames);
    const inputs = Array.isArray(node.inputs) ? node.inputs : [];
    return names.filter((name) => {
        const linkedInput = inputs.find((input) => input?.widget?.name?.trim() === name);
        return linkedInput?.link == null || includedLinkedNames.has(name);
    });
}
/** Normalize one widget order and reject ambiguous duplicate identities. */
function uniqueWidgetNames(node, rawNames) {
    const names = [];
    for (const rawName of rawNames) {
        const name = rawName.trim();
        if (!name)
            continue;
        if (names.includes(name)) {
            throw new Error(`Serialized node '${node?.id ?? ''}' has duplicate widget name '${name}'.`);
        }
        names.push(name);
    }
    return names;
}
/** Compare two widget identity sequences without coercing their values. */
function arraysEqual(left, right) {
    return left.length === right.length && left.every((name, index) => name === right[index]);
}
/** Build the complete current array while applying portable values only by name. */
function rebuildWidgetValues(liveWidgets, persistedByName) {
    return liveWidgets.map((widget) => {
        if (!widget) {
            return null;
        }
        const persisted = persistedByName.get(widget.name);
        return isSerializedWidget(widget) && isValidPersistedWidgetValue(widget, persisted)
            ? cloneWidgetValue(persisted)
            : readCurrentWidgetValue(widget);
    });
}
/** Reject absent or stale picker choices so the current host supplies its default. */
function isValidPersistedWidgetValue(widget, value) {
    if (value === undefined || value === null)
        return false;
    const pickerValues = widget.options?.values;
    return !Array.isArray(pickerValues) || pickerValues.some((option) => Object.is(option, value));
}
/** Return widget identities whose authoritative values arrive through graph links. */
function linkedWidgetNames(node) {
    return new Set((Array.isArray(node.inputs) ? node.inputs : [])
        .filter((input) => input?.link != null)
        .map((input) => input.widget?.name?.trim() ?? '')
        .filter(Boolean));
}

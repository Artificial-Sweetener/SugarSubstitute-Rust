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
/**
 * Own managed instance group geometry, metadata, title, and marker projection.
 */
import { readGroupBounds } from './Bounds.js';
import { CUBE_INSTANCE_HEADER_HEIGHT, CUBE_INSTANCE_AUTO_MIN_MARGINS, CUBE_INSTANCE_PADDING, CUBE_INSTANCE_TOP_EXTRA, computeInstanceBounds, computeVisualContentBounds, contentFitsWithinBounds, expandBoundsForContentMargins, inflateInstanceBounds, resolveChromeBoundsFromContent, resolveNewInstanceBounds, } from './CubeBounds.js';
import { hasAuthoredGroupGeometry } from '../geometry/AuthoredGroupGeometry.js';
import { allocateUniqueInstanceAlias, ensureGroupTitleWatcher, syncInstanceAlias, } from './InstanceAliasSync.js';
import { flattenCubeGroupMetadata, getGroupSugarcubes, setGroupSugarcubes, resolveInstanceDisplayName, writeCubeDefinitionMetadata, writeCubeInstanceMetadata, } from './GroupMetadata.js';
import { updateMarkersForIds } from './CubeMarkers.js';
import { buildCubeDefinitionKey, normalizeRevisionRef } from '../core/CubeDefinitionKey.js';
import { isRecord } from '../types/common.js';
const INSTANCE_SCHEMA = 5;
const GROUP_COLOR = '#3f789e';
const GROUP_BACKGROUND = '#3f5159';
/** Apply one discovered Cube instance to its managed LiteGraph group. */
export class InstanceGroupPresenter {
    options;
    constructor(options) {
        this.options = options;
    }
    apply(instance, group, graph, resolvedAlias) {
        const contentBounds = computeInstanceBounds(instance.nodes, instance.markers);
        const visualBounds = computeVisualContentBounds(instance.nodes, instance.markers);
        const existing = group ? getGroupSugarcubes(group) : null;
        const cleaned = existing ? { ...existing } : null;
        const existingId = readMetadataString(cleaned, 'instance_id');
        const canonicalId = existingId || instance.instanceId;
        const previousAlias = readMetadataString(cleaned, 'default_alias');
        const existingInstanceAlias = readMetadataString(cleaned, 'instance_alias');
        const defaultAlias = instance.defaultAlias || readMetadataString(cleaned, 'default_alias');
        const targetModel = instance.targetModel || readMetadataString(cleaned, 'target_model');
        const cubeVersion = instance.cubeVersion || readMetadataString(cleaned, 'cube_version');
        const revisionRef = normalizeRevisionRef(instance.cubeRevisionRef || cleaned?.cube_revision_ref);
        const definitionKey = instance.cubeDefinitionKey ||
            readMetadataString(cleaned, 'cube_definition_key') ||
            buildCubeDefinitionKey(instance.cubeId, cubeVersion);
        const icon = isRecord(instance.icon)
            ? instance.icon
            : isRecord(cleaned?.icon)
                ? cleaned.icon
                : null;
        const instanceAlias = resolvedAlias?.trim() ||
            existingInstanceAlias ||
            instance.instanceAlias ||
            defaultAlias ||
            instance.cubeId ||
            'SugarCube';
        const existingBounds = normalizeBoundsGeometry(cleaned?.bounds);
        const groupBounds = readGroupBoundsGeometry(group);
        const boundsRecord = isRecord(cleaned?.bounds) ? cleaned.bounds : {};
        const padding = normalizePadding(boundsRecord.padding);
        const header = normalizeHeader(boundsRecord.header);
        const contentDerived = resolveChromeBoundsFromContent({
            nodes: instance.nodes,
            markers: instance.markers,
            padding,
            header,
        });
        const preserveAuthored = hasAuthoredGroupGeometry(cleaned);
        let bounds = groupBounds || existingBounds;
        let usedNewResolver = false;
        if (contentDerived && !preserveAuthored) {
            bounds = contentDerived;
            usedNewResolver = true;
        }
        else if (!bounds) {
            if (cleaned) {
                if (!contentBounds)
                    return null;
                bounds = inflateInstanceBounds(contentBounds, { ...padding, header: { ...header } });
            }
            else {
                bounds = resolveNewInstanceBounds({
                    nodes: instance.nodes,
                    markers: instance.markers,
                    padding,
                    header,
                });
                usedNewResolver = true;
            }
            if (!bounds)
                return null;
        }
        const containment = contentBounds || visualBounds;
        if (!usedNewResolver && containment && !contentFitsWithinBounds(bounds, containment)) {
            bounds = inflateInstanceBounds(containment, { ...padding, header: { ...header } });
        }
        if (!bounds)
            return null;
        if (!usedNewResolver && !cleaned && (visualBounds || contentBounds)) {
            bounds =
                expandBoundsForContentMargins(bounds, visualBounds || contentBounds, {
                    left: CUBE_INSTANCE_AUTO_MIN_MARGINS.left,
                    right: CUBE_INSTANCE_AUTO_MIN_MARGINS.right,
                    bottom: CUBE_INSTANCE_AUTO_MIN_MARGINS.bottom,
                    top: padding.y + padding.top_extra + header.height + CUBE_INSTANCE_AUTO_MIN_MARGINS.innerTop,
                }) ?? bounds;
        }
        const seed = { ...(cleaned || {}), schema: INSTANCE_SCHEMA, managed: true };
        const definition = writeCubeDefinitionMetadata(seed, {
            cube_id: instance.cubeId,
            default_alias: defaultAlias,
            target_model: targetModel,
            cube_version: cubeVersion,
            cube_revision_ref: revisionRef,
            cube_definition_key: definitionKey,
            ...(icon ? { icon } : {}),
        });
        const owned = writeCubeInstanceMetadata(definition, {
            instance_id: canonicalId,
            instance_alias: instanceAlias,
            markers: instance.markerLookup,
            nodes: instance.nodeIds,
            bounds: {
                x: bounds.x,
                y: bounds.y,
                w: bounds.w,
                h: bounds.h,
                padding: { ...padding },
                header: { ...header },
            },
        });
        const metadata = flattenCubeGroupMetadata(owned, cleaned);
        const target = group || this.createGroup(graph, metadata);
        if (!target)
            return null;
        target.pos = [bounds.x, bounds.y];
        target.size = [bounds.w, bounds.h];
        setGroupSugarcubes(target, metadata);
        if (target.__sugarcubes_imported)
            delete target.__sugarcubes_imported;
        const metadataId = readMetadataString(metadata, 'instance_id');
        const metadataAlias = readMetadataString(metadata, 'instance_alias');
        const cubeId = readMetadataString(metadata, 'cube_id');
        if (metadataId) {
            updateMarkersForIds(graph, instance.markerIds, {
                instanceId: metadataId,
                instanceAlias: metadataAlias,
                cubeVersion: metadata.cube_version,
                cubeRevisionRef: metadata.cube_revision_ref,
            });
        }
        this.bindTitleProjection(target, graph);
        syncInstanceAlias({
            graph,
            group: target,
            metadata,
            cubeId,
            instanceAlias: metadataAlias,
            requestDirtyRefresh: this.options.requestDirtyRefresh,
        });
        const nextDefaultAlias = readMetadataString(metadata, 'default_alias');
        if (cubeId && previousAlias && previousAlias !== nextDefaultAlias) {
            this.options.events?.emit?.('cube:default-alias:changed', {
                cubeId,
                defaultAlias: nextDefaultAlias,
            });
        }
        instance.instanceId = metadataId;
        return target;
    }
    createGroup(graph, metadata) {
        const Group = this.options.adapter.getLiteGraph?.()?.LGraphGroup;
        if (!Group)
            return null;
        const group = new Group(resolveInstanceDisplayName({ metadata, fallback: 'SugarCube' }) || 'SugarCube');
        graph.add?.(group);
        if (!group.color)
            group.color = GROUP_COLOR;
        if (!group.bgcolor)
            group.bgcolor = GROUP_BACKGROUND;
        return group;
    }
    bindTitleProjection(group, graph) {
        ensureGroupTitleWatcher(group, (groupRef, next) => {
            const metadata = getGroupSugarcubes(groupRef);
            const cubeId = readMetadataString(metadata, 'cube_id');
            if (!cubeId)
                return;
            const alias = allocateUniqueInstanceAlias(graph, next, {
                currentInstanceId: readMetadataString(metadata, 'instance_id'),
                currentGroup: groupRef,
            });
            syncInstanceAlias({
                graph,
                group: groupRef,
                metadata,
                cubeId,
                instanceAlias: alias,
                events: this.options.events,
                requestDirtyRefresh: this.options.requestDirtyRefresh,
            });
        });
    }
}
function readNumber(value, fallback = null) {
    const number = Number(value);
    return Number.isFinite(number) ? number : fallback;
}
function normalizePadding(value) {
    const source = isRecord(value) ? value : {};
    return {
        x: readNumber(source.x, CUBE_INSTANCE_PADDING.x),
        y: readNumber(source.y, CUBE_INSTANCE_PADDING.y),
        top_extra: readNumber(source.top_extra, CUBE_INSTANCE_TOP_EXTRA),
    };
}
function normalizeHeader(value) {
    const source = isRecord(value) ? value : {};
    return { height: readNumber(source.height, CUBE_INSTANCE_HEADER_HEIGHT) };
}
function normalizeBoundsGeometry(value) {
    const source = isRecord(value) ? value : {};
    const x = readNumber(source.x, null);
    const y = readNumber(source.y, null);
    const w = readNumber(source.w, null);
    const h = readNumber(source.h, null);
    return x === null || y === null || w === null || h === null ? null : { x, y, w, h };
}
function readGroupBoundsGeometry(group) {
    const bounds = readGroupBounds(group);
    if (!bounds || bounds.length < 4)
        return null;
    const x = readNumber(bounds[0], null);
    const y = readNumber(bounds[1], null);
    const w = readNumber(bounds[2], null);
    const h = readNumber(bounds[3], null);
    return x === null || y === null || w === null || h === null ? null : { x, y, w, h };
}
function readMetadataString(metadata, key) {
    const value = metadata?.[key];
    return typeof value === 'string' ? value.trim() : '';
}

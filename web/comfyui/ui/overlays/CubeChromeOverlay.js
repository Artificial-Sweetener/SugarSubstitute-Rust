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
 * Own the SugarCubes overlay rendering layer in `frontend/comfyui/ui/overlays/CubeChromeOverlay.js`.
 */
import { getGraphGroups } from '../graph/GraphQuery.js';
import { getGroupSugarcubes, resolveCubeDisplayName, resolveInstanceDisplayName, } from '../graph/GroupMetadata.js';
import { readGroupBounds } from '../graph/Bounds.js';
import { parseCanonicalCubeId } from '../core/CubeId.js';
import { CubeIconResolver } from '../core/CubeIconResolver.js';
import { drawFallbackInitialsCanvas } from '../core/CubeFallbackIconRenderer.js';
const CHROME_BADGE_MAX_WIDTH = 280;
const CHROME_BADGE_MIN_WIDTH = 80;
const CHROME_BUTTONS = Object.freeze([
    {
        key: 'swap-left',
        style: 'action',
        tooltip: 'Swap left',
        requires: 'onSwapLeft',
        label: '⇦',
        labelAlign: 'center',
        labelScale: 1.15,
    },
    {
        key: 'swap-right',
        style: 'action',
        tooltip: 'Swap right',
        requires: 'onSwapRight',
        label: '⇨',
        labelAlign: 'center',
        labelScale: 1.15,
    },
    {
        key: 'menu',
        icon: 'cube',
        style: 'action',
        tooltip: 'Cubes',
    },
]);
function measureTextEllipsis(ctx, text, maxWidth) {
    if (!text) {
        return { text: '', width: 0, truncated: false };
    }
    if (maxWidth <= 0) {
        return { text: '', width: 0, truncated: true };
    }
    const fullWidth = ctx.measureText(text).width;
    if (fullWidth <= maxWidth) {
        return { text, width: fullWidth, truncated: false };
    }
    const ellipsis = '...';
    const ellipsisWidth = ctx.measureText(ellipsis).width;
    if (ellipsisWidth >= maxWidth) {
        return { text: '', width: 0, truncated: true };
    }
    let low = 0;
    let high = text.length;
    while (low < high) {
        const mid = Math.ceil((low + high) / 2);
        const candidate = `${text.slice(0, mid)}${ellipsis}`;
        const width = ctx.measureText(candidate).width;
        if (width <= maxWidth) {
            low = mid;
        }
        else {
            high = mid - 1;
        }
    }
    const trimmed = `${text.slice(0, low)}${ellipsis}`;
    return { text: trimmed, width: ctx.measureText(trimmed).width, truncated: true };
}
function clampNumber(value, min, max) {
    if (!Number.isFinite(value)) {
        return min;
    }
    return Math.min(max, Math.max(min, value));
}
function resolveGroupTitleColor(group) {
    const color = typeof group?.color === 'string' ? group.color.trim() : '';
    if (color) {
        return color;
    }
    const bg = typeof group?.bgcolor === 'string' ? group.bgcolor.trim() : '';
    if (bg) {
        return bg;
    }
    return '#9ab4c7';
}
function resolveGroupFontFamily() {
    const liteGraph = typeof globalThis !== 'undefined' ? globalThis.LiteGraph : null;
    const font = typeof liteGraph?.GROUP_FONT === 'string' ? liteGraph.GROUP_FONT.trim() : '';
    return font || 'sans-serif';
}
function resolveGroupTitlePadding() {
    const liteGraph = typeof globalThis !== 'undefined' ? globalThis.LiteGraph : null;
    const padding = liteGraph?.LGraphGroup?.padding;
    if (Number.isFinite(padding)) {
        return Number(padding);
    }
    return 4;
}
function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
}
function parseHexChannel(value) {
    const parsed = Number.parseInt(value, 16);
    return Number.isFinite(parsed) ? parsed : null;
}
function parseColorToRgb(color) {
    if (typeof color !== 'string') {
        return null;
    }
    const hex = color.trim();
    if (!hex.startsWith('#')) {
        return null;
    }
    if (hex.length === 4) {
        const r = parseHexChannel((hex[1] ?? '') + (hex[1] ?? ''));
        const g = parseHexChannel((hex[2] ?? '') + (hex[2] ?? ''));
        const b = parseHexChannel((hex[3] ?? '') + (hex[3] ?? ''));
        if (r == null || g == null || b == null) {
            return null;
        }
        return { r, g, b };
    }
    if (hex.length === 7) {
        const r = parseHexChannel(hex.slice(1, 3));
        const g = parseHexChannel(hex.slice(3, 5));
        const b = parseHexChannel(hex.slice(5, 7));
        if (r == null || g == null || b == null) {
            return null;
        }
        return { r, g, b };
    }
    return null;
}
function rgbToHsl({ r, g, b }) {
    const rn = r / 255;
    const gn = g / 255;
    const bn = b / 255;
    const max = Math.max(rn, gn, bn);
    const min = Math.min(rn, gn, bn);
    const delta = max - min;
    let h = 0;
    let s = 0;
    const l = (max + min) / 2;
    if (delta !== 0) {
        if (max === rn) {
            h = ((gn - bn) / delta) % 6;
        }
        else if (max === gn) {
            h = (bn - rn) / delta + 2;
        }
        else {
            h = (rn - gn) / delta + 4;
        }
        h = Math.round(h * 60);
        if (h < 0) {
            h += 360;
        }
        s = delta / (1 - Math.abs(2 * l - 1));
    }
    return { h, s, l };
}
function hslToRgb({ h, s, l }) {
    const c = (1 - Math.abs(2 * l - 1)) * s;
    const hh = h / 60;
    const x = c * (1 - Math.abs((hh % 2) - 1));
    let r1 = 0;
    let g1 = 0;
    let b1 = 0;
    if (hh >= 0 && hh < 1) {
        r1 = c;
        g1 = x;
    }
    else if (hh >= 1 && hh < 2) {
        r1 = x;
        g1 = c;
    }
    else if (hh >= 2 && hh < 3) {
        g1 = c;
        b1 = x;
    }
    else if (hh >= 3 && hh < 4) {
        g1 = x;
        b1 = c;
    }
    else if (hh >= 4 && hh < 5) {
        r1 = x;
        b1 = c;
    }
    else {
        r1 = c;
        b1 = x;
    }
    const m = l - c / 2;
    return {
        r: Math.round((r1 + m) * 255),
        g: Math.round((g1 + m) * 255),
        b: Math.round((b1 + m) * 255),
    };
}
function rgbToHex({ r, g, b }) {
    const toHex = (value) => value.toString(16).padStart(2, '0');
    return `#${toHex(clamp(Math.round(r), 0, 255))}${toHex(clamp(Math.round(g), 0, 255))}${toHex(clamp(Math.round(b), 0, 255))}`;
}
function triadicColor(baseColor, offset) {
    const rgb = parseColorToRgb(baseColor);
    if (!rgb) {
        return baseColor;
    }
    const hsl = rgbToHsl(rgb);
    const shifted = (hsl.h + offset + 360) % 360;
    const nextRgb = hslToRgb({ h: shifted, s: hsl.s, l: hsl.l });
    return rgbToHex(nextRgb);
}
function sumPillWidths(entries, gap) {
    if (!entries.length) {
        return 0;
    }
    return entries.reduce((sum, entry) => sum + entry.pillWidth, 0) + gap * (entries.length - 1);
}
function buildStackedBadgeLayout(ctx, nameText, authorText, maxWidth, paddingX, sizes, fontFamily) {
    const trimmedName = typeof nameText === 'string' ? nameText.trim() : '';
    const trimmedAuthor = typeof authorText === 'string' ? authorText.trim() : '';
    if (!trimmedName) {
        return {
            visible: false,
            width: 0,
            lines: [],
            truncated: false,
        };
    }
    const textMaxWidth = Math.max(0, maxWidth - paddingX * 2);
    const lines = [
        { key: 'name', text: trimmedName, size: sizes.name },
        { key: 'author', text: trimmedAuthor, size: sizes.author },
    ];
    const family = fontFamily || 'sans-serif';
    const measured = lines.map((line) => {
        ctx.font = `${line.size}px ${family}`;
        const measurement = measureTextEllipsis(ctx, line.text, textMaxWidth);
        return {
            key: line.key,
            size: line.size,
            text: measurement.text,
            width: measurement.width,
            truncated: measurement.truncated,
            fullText: line.text,
        };
    });
    const maxLineWidth = measured.reduce((max, entry) => Math.max(max, entry.width), 0);
    const width = maxLineWidth + paddingX * 2;
    return {
        visible: width > 0,
        width,
        lines: measured,
        truncated: measured.some((entry) => entry.truncated),
    };
}
/**
 * Resolve the centered badge slot without overlapping titlebar chrome.
 */
function computeCenteredBadgeSlot({ groupX, groupWidth, inset, titlebarLeftWidth, pillStart, gap, }) {
    const left = Math.min(groupX + inset + titlebarLeftWidth, groupX + groupWidth - inset);
    const right = Math.max(left, pillStart - gap);
    return {
        left,
        right,
        center: groupX + groupWidth / 2,
        width: Math.max(0, right - left),
    };
}
/**
 * Resolve cube source identity for the badge from canonical metadata first.
 */
function resolveCubeBadgeSource(metadata, fallbackSource) {
    const cubeId = typeof metadata?.cube_id === 'string' ? metadata.cube_id.trim() : '';
    if (cubeId) {
        try {
            const parsed = parseCanonicalCubeId(cubeId);
            if (parsed.sourceKind === 'github') {
                return {
                    sourceKind: 'github',
                    author: parsed.owner,
                    pack: parsed.repo,
                    namespace: '',
                };
            }
            if (parsed.sourceKind === 'local') {
                return {
                    sourceKind: 'local',
                    author: '',
                    pack: '',
                    namespace: parsed.namespace,
                };
            }
        }
        catch (_error) {
            // Chrome rendering must not fail because persisted metadata is malformed.
        }
    }
    return fallbackSource || { sourceKind: '', author: '', pack: '', namespace: '' };
}
/**
 * Format the badge source line from parsed cube source identity.
 */
function formatSourceBadgeText(source) {
    const pack = typeof source?.pack === 'string' ? source.pack.trim() : '';
    const author = typeof source?.author === 'string' ? source.author.trim() : '';
    const namespace = typeof source?.namespace === 'string' ? source.namespace.trim() : '';
    const sourceKind = typeof source?.sourceKind === 'string' ? source.sourceKind : '';
    if (sourceKind === 'local') {
        return namespace ? `from local ${namespace}` : 'from local';
    }
    if (pack && author) {
        return `from ${pack} by ${author}`;
    }
    if (pack) {
        return `from ${pack}`;
    }
    if (author) {
        return `by ${author}`;
    }
    return 'from Unknown';
}
/**
 * Format the cube version badge with explicit product wording.
 */
function formatVersionBadgeText(metadata) {
    const version = typeof metadata?.cube_version === 'string' ? metadata.cube_version.trim() : '';
    if (!version) {
        return '';
    }
    const normalized = version.replace(/^[vV](?=\d)/, '');
    return `version ${normalized}`;
}
function computePillLayout(ctx, items, maxWidth, fontSize, paddingX, gap, options = {}) {
    const pinnedKey = typeof options?.pinnedKey === 'string' ? options.pinnedKey : '';
    const entries = items.map((item) => {
        const iconSize = item.icon ? Math.max(12, fontSize - 2) : 0;
        const textWidth = item.label ? ctx.measureText(item.label).width : 0;
        const contentWidth = item.icon ? iconSize : textWidth;
        const pillWidth = contentWidth + paddingX * 2;
        return { item, pillWidth };
    });
    const pinIndex = pinnedKey ? entries.findIndex((entry) => entry.item?.key === pinnedKey) : -1;
    while (entries.length) {
        const total = sumPillWidths(entries, gap);
        if (total <= maxWidth) {
            break;
        }
        if (pinIndex >= 0) {
            const candidateIndex = entries.findIndex((entry) => entry.item?.key !== pinnedKey);
            if (candidateIndex === -1) {
                break;
            }
            entries.splice(candidateIndex, 1);
            continue;
        }
        entries.pop();
    }
    return { entries, totalWidth: sumPillWidths(entries, gap) };
}
/**
 * Drop a lower-priority action pill while preserving pinned chrome controls.
 */
function removeUnpinnedPillEntry(entries, pinnedKey) {
    const index = entries.findIndex((entry) => entry.item?.key !== pinnedKey);
    if (index < 0) {
        return false;
    }
    entries.splice(index, 1);
    return true;
}
function hasSwapEligibility(metadata) {
    const markers = metadata?.markers;
    const inputs = Array.isArray(markers?.inputs) ? markers.inputs : [];
    const outputs = Array.isArray(markers?.outputs) ? markers.outputs : [];
    return inputs.length > 0 && outputs.length > 0;
}
/**
 * Coordinate cube chrome overlay behavior for the SugarCubes UI.
 */
export class CubeChromeOverlay {
    adapter;
    actions;
    resolveSource;
    hitRegions;
    badgeRegions;
    hoveredKey;
    hoveredInstance;
    hoveredBadgeInstance;
    hoveredBadgeKey;
    instanceState;
    iconResolver;
    lastTransform = null;
    constructor({ adapter = null, actions = {}, resolveSource = null } = {}) {
        this.adapter = adapter;
        this.actions = actions || {};
        this.resolveSource = typeof resolveSource === 'function' ? resolveSource : null;
        this.hitRegions = [];
        this.badgeRegions = [];
        this.hoveredKey = null;
        this.hoveredInstance = null;
        this.hoveredBadgeInstance = null;
        this.hoveredBadgeKey = null;
        this.instanceState = new Map();
        this.iconResolver = new CubeIconResolver({ onImageLoad: () => this.requestRedraw() });
    }
    buildMenuOptions({ metadata, }) {
        return [
            {
                title: 'Save cube implementation',
                callback: () => this.actions.onSaveImplementation?.(metadata),
            },
            {
                title: 'Save current values as cube defaults',
                callback: () => this.actions.onSaveCubeDefaults?.(metadata),
            },
        ];
    }
    setup() {
        this.installGroupTitleIconRenderer();
    }
    dispose() {
        this.hitRegions = [];
        this.badgeRegions = [];
        this.hoveredKey = null;
        this.hoveredInstance = null;
        this.hoveredBadgeInstance = null;
        this.hoveredBadgeKey = null;
        this.instanceState.clear();
        this.releaseGroupTitleIconRenderer();
    }
    setActions(actions) {
        this.actions = actions || {};
    }
    /** Expose immutable observable overlay state for diagnostics. */
    getDebugState() {
        return {
            actions: this.actions,
            hitRegions: this.hitRegions,
            badgeRegions: this.badgeRegions,
            hoveredKey: this.hoveredKey,
            hoveredBadgeInstance: this.hoveredBadgeInstance,
            hoveredBadgeKey: this.hoveredBadgeKey,
        };
    }
    installGroupTitleIconRenderer() {
        const liteGraph = this.adapter?.getLiteGraph?.() ||
            (typeof globalThis !== 'undefined' ? globalThis.LiteGraph : null);
        const GroupRef = liteGraph?.LGraphGroup;
        if (!GroupRef?.prototype) {
            return;
        }
        const prototype = GroupRef.prototype;
        if (typeof prototype.draw !== 'function') {
            return;
        }
        prototype.__sugarcubes_title_icon_renderer = this;
        if (prototype.__sugarcubes_title_icon_patched) {
            return;
        }
        const originalDraw = prototype.draw;
        prototype.__sugarcubes_title_icon_original_draw = originalDraw;
        prototype.__sugarcubes_title_icon_patched = true;
        prototype.draw = function drawSugarCubesGroupTitleIcon(...args) {
            const renderer = prototype.__sugarcubes_title_icon_renderer;
            if (renderer?.shouldDrawGroupTitleIcon?.(this)) {
                return renderer.drawManagedGroupWithTitleIcon(this, originalDraw, args);
            }
            return originalDraw.apply(this, args);
        };
    }
    releaseGroupTitleIconRenderer() {
        const liteGraph = this.adapter?.getLiteGraph?.() ||
            (typeof globalThis !== 'undefined' ? globalThis.LiteGraph : null);
        const GroupRef = liteGraph?.LGraphGroup;
        const prototype = GroupRef?.prototype;
        if (prototype?.__sugarcubes_title_icon_renderer === this) {
            delete prototype.__sugarcubes_title_icon_renderer;
        }
    }
    shouldDrawGroupTitleIcon(group) {
        const metadata = getGroupSugarcubes(group);
        return Boolean(metadata?.managed && metadata.instance_id);
    }
    drawManagedGroupWithTitleIcon(group, originalDraw, args) {
        const titleDescriptor = Object.getOwnPropertyDescriptor(group, 'title');
        const hadTitleDescriptor = Boolean(titleDescriptor);
        try {
            Object.defineProperty(group, 'title', {
                value: '',
                writable: true,
                enumerable: titleDescriptor?.enumerable ?? true,
                configurable: true,
            });
            originalDraw.apply(group, args);
        }
        finally {
            if (hadTitleDescriptor && titleDescriptor) {
                Object.defineProperty(group, 'title', titleDescriptor);
            }
            else {
                delete group.title;
            }
        }
        const ctx = args[1];
        const canvasInstance = args[0];
        if (!ctx ||
            typeof ctx !== 'object' ||
            typeof ctx.save !== 'function' ||
            !canvasInstance ||
            typeof canvasInstance !== 'object') {
            return undefined;
        }
        this.drawGroupTitleIcon(ctx, group, canvasInstance);
        return undefined;
    }
    drawGroupTitleIcon(ctx, group, canvasInstance) {
        const metadata = getGroupSugarcubes(group);
        const bounds = readGroupBounds(group);
        if (!metadata || !bounds) {
            return;
        }
        const [x, y] = bounds;
        const fontSize = Math.max(14, Number(group?.font_size) || Number(metadata?.font_size) || 18);
        const fontFamily = resolveGroupFontFamily();
        const padding = resolveGroupTitlePadding();
        const headerHeight = fontSize * 1.4;
        const iconSize = clampNumber(Math.floor(headerHeight - 8), 18, 28);
        const iconGap = 7;
        const title = resolveInstanceDisplayName({
            metadata,
            group,
            fallback: resolveCubeDisplayName({ metadata, group, fallback: 'SugarCube' }),
        });
        const iconModel = this.iconResolver.resolve({
            icon: metadata?.icon,
            cube_id: metadata?.cube_id,
            default_alias: metadata?.default_alias,
        });
        const baseAlpha = Number.isFinite(ctx.globalAlpha) ? ctx.globalAlpha : 1;
        const iconX = x + padding;
        const iconY = y + Math.max(2, (headerHeight - iconSize) / 2);
        this.drawDefinitionIcon(ctx, iconModel, iconX, iconY, iconSize, {
            alpha: baseAlpha * 0.96,
        });
        if (!title) {
            return;
        }
        ctx.save();
        ctx.globalAlpha = Math.max(0, Math.min(1, canvasInstance?.editor_alpha ?? baseAlpha));
        ctx.font = `${fontSize}px ${fontFamily}`;
        ctx.fillStyle = resolveGroupTitleColor(group);
        ctx.textAlign = 'left';
        ctx.textBaseline = 'alphabetic';
        ctx.fillText(title, iconX + iconSize + iconGap, y + fontSize);
        ctx.restore();
    }
    render(ctx, canvasInstance) {
        if (!ctx || !canvasInstance) {
            return;
        }
        const graph = canvasInstance.graph;
        if (!graph) {
            return;
        }
        this.hitRegions = [];
        this.badgeRegions = [];
        ctx.save();
        this.lastTransform = typeof ctx.getTransform === 'function' ? ctx.getTransform() : null;
        const groups = getGraphGroups(graph);
        if (!groups.length) {
            ctx.restore();
            return;
        }
        ctx.textBaseline = 'middle';
        for (const group of groups) {
            const metadata = getGroupSugarcubes(group);
            if (!metadata?.managed || !metadata.instance_id) {
                continue;
            }
            const bounds = readGroupBounds(group);
            if (!bounds) {
                continue;
            }
            this.renderHeader(ctx, bounds, metadata, group);
        }
        ctx.restore();
    }
    renderHeader(ctx, bounds, metadata, group = {}) {
        const now = Date.now();
        const [x = 0, y = 0, w = 0] = bounds;
        const fontSize = Math.max(14, Number(metadata?.font_size) || 18);
        const inset = 8;
        const gap = 6;
        const paddingX = 8;
        const paddingY = 4;
        const titleColor = resolveGroupTitleColor(group);
        const swapLeftColor = triadicColor(titleColor, -120);
        const swapRightColor = triadicColor(titleColor, 120);
        const items = [];
        const instanceId = metadata?.instance_id || '';
        const state = this.ensureInstanceState(instanceId);
        const isDirty = Boolean(metadata?.has_saveable_changes);
        const showSaved = Boolean(state.savedAt && now - state.savedAt < 350);
        const chromeContext = {
            isDirty,
            showSaved,
            canSwap: hasSwapEligibility(metadata),
        };
        for (const entry of CHROME_BUTTONS) {
            if (entry.requires && !this.actions[entry.requires]) {
                continue;
            }
            if (entry.key === 'swap-left') {
                const allowed = this.actions.canSwap
                    ? this.actions.canSwap(metadata, 'left')
                    : chromeContext.canSwap;
                if (!allowed) {
                    continue;
                }
            }
            if (entry.key === 'swap-right') {
                const allowed = this.actions.canSwap
                    ? this.actions.canSwap(metadata, 'right')
                    : chromeContext.canSwap;
                if (!allowed) {
                    continue;
                }
            }
            if (typeof entry.visible === 'function' && !entry.visible(chromeContext)) {
                continue;
            }
            const label = typeof entry.buildLabel === 'function' ? entry.buildLabel(chromeContext) : null;
            const extra = typeof entry.buildExtra === 'function' ? entry.buildExtra(chromeContext) : null;
            let itemColor = entry.color;
            if (entry.key === 'swap-left') {
                itemColor = swapLeftColor;
            }
            if (entry.key === 'swap-right') {
                itemColor = swapRightColor;
            }
            items.push({
                key: entry.key,
                icon: entry.icon,
                color: entry.key === 'menu' ? titleColor : itemColor,
                label: label ?? entry.label,
                style: entry.style,
                tooltip: entry.tooltip,
                ...(extra || {}),
            });
        }
        ctx.font = `${fontSize}px sans-serif`;
        const maxWidth = w - inset * 2;
        const headerHeight = Number(metadata?.bounds?.header?.height) || 32;
        const availableHeight = Math.max(0, headerHeight - paddingY * 2);
        const baseFont = Math.max(12, Number(metadata?.font_size) || 18);
        const nameSize = clampNumber(baseFont, 12, Math.max(12, Math.floor(availableHeight * 0.6)));
        const authorSize = clampNumber(nameSize - 2, 10, Math.max(10, Math.floor(availableHeight * 0.4)));
        const lineGap = 2;
        const fontFamily = resolveGroupFontFamily();
        const titlePadding = resolveGroupTitlePadding();
        const groupTitleSize = Number(group?.font_size) || fontSize;
        const resolvedDisplayName = resolveCubeDisplayName({
            metadata,
            group,
            fallback: 'SugarCube',
        });
        const currentInstanceTitle = resolveInstanceDisplayName({
            metadata,
            group,
            fallback: resolvedDisplayName,
        });
        let instanceTitleWidth = 0;
        if (currentInstanceTitle) {
            ctx.save();
            ctx.font = `${groupTitleSize}px ${fontFamily}`;
            instanceTitleWidth = ctx.measureText(currentInstanceTitle).width;
            ctx.restore();
        }
        const versionText = formatVersionBadgeText(metadata);
        const displayName = versionText ? `${resolvedDisplayName} ${versionText}` : resolvedDisplayName;
        const fallbackSource = typeof this.resolveSource === 'function' ? this.resolveSource(metadata) : null;
        const sourceLine = formatSourceBadgeText(resolveCubeBadgeSource(metadata, fallbackSource));
        const badgeSizes = { name: nameSize, author: authorSize };
        const iconSize = clampNumber(Math.floor(availableHeight), 18, 32);
        const iconGap = 7;
        const titlebarLeftWidth = iconSize + iconGap + instanceTitleWidth + titlePadding + 12;
        const pinnedPillKey = 'menu';
        const pillLayout = computePillLayout(ctx, items, maxWidth, fontSize, paddingX, gap, {
            pinnedKey: pinnedPillKey,
        });
        const pillEntries = pillLayout.entries;
        let pillsTotalWidth = pillLayout.totalWidth;
        const computePillStart = () => pillEntries.length ? x + w - inset - (pillsTotalWidth ? pillsTotalWidth : 0) : x + w - inset;
        const computeBadgeSlot = () => computeCenteredBadgeSlot({
            groupX: x,
            groupWidth: w,
            inset,
            titlebarLeftWidth,
            pillStart: computePillStart(),
            gap,
        });
        let centeredBadgeSlot = computeBadgeSlot();
        let badgeMaxWidth = Math.min(CHROME_BADGE_MAX_WIDTH, centeredBadgeSlot.width);
        let stackedBadgeLayout = buildStackedBadgeLayout(ctx, displayName, sourceLine, badgeMaxWidth, paddingX, badgeSizes, fontFamily);
        while (stackedBadgeLayout.visible &&
            centeredBadgeSlot.width < CHROME_BADGE_MIN_WIDTH &&
            pillEntries.length) {
            if (!removeUnpinnedPillEntry(pillEntries, pinnedPillKey)) {
                break;
            }
            pillsTotalWidth = sumPillWidths(pillEntries, gap);
            centeredBadgeSlot = computeBadgeSlot();
            badgeMaxWidth = Math.min(CHROME_BADGE_MAX_WIDTH, centeredBadgeSlot.width);
            stackedBadgeLayout = buildStackedBadgeLayout(ctx, displayName, sourceLine, badgeMaxWidth, paddingX, badgeSizes, fontFamily);
            if (!centeredBadgeSlot.width) {
                break;
            }
        }
        if (stackedBadgeLayout.visible && centeredBadgeSlot.width < CHROME_BADGE_MIN_WIDTH) {
            stackedBadgeLayout = {
                visible: false,
                width: 0,
                lines: [],
                truncated: false,
            };
        }
        let cursorX = x + w - inset - (pillsTotalWidth || 0);
        const centerY = y + fontSize;
        const pillHeight = fontSize + paddingY * 2;
        const badgeHeight = nameSize + authorSize + lineGap + paddingY * 2;
        const baseAlpha = Number.isFinite(ctx.globalAlpha) ? ctx.globalAlpha : 1;
        if (!pillEntries.length && !stackedBadgeLayout.visible) {
            return;
        }
        if (isDirty && !state.dirty) {
            state.dirty = true;
            state.appearAt = now;
        }
        else if (!isDirty && state.dirty) {
            state.dirty = false;
            state.appearAt = 0;
        }
        const isAnimating = this.isAnimating(state, now);
        if (stackedBadgeLayout.visible) {
            centeredBadgeSlot = computeBadgeSlot();
            if (stackedBadgeLayout.width > centeredBadgeSlot.width) {
                stackedBadgeLayout = {
                    visible: false,
                    width: 0,
                    lines: [],
                    truncated: false,
                };
            }
        }
        if (stackedBadgeLayout.visible) {
            const unclampedBadgeX = centeredBadgeSlot.center - stackedBadgeLayout.width / 2;
            const badgeX = clampNumber(unclampedBadgeX, centeredBadgeSlot.left, centeredBadgeSlot.right - stackedBadgeLayout.width);
            const badgeY = centerY - badgeHeight / 2;
            const nameLine = stackedBadgeLayout.lines.find((line) => line.key === 'name');
            const authorLineLayout = stackedBadgeLayout.lines.find((line) => line.key === 'author');
            const titleColor = resolveGroupTitleColor(group);
            const textLeft = badgeX;
            const textWidth = stackedBadgeLayout.width;
            const textCenterX = textLeft + textWidth / 2;
            if (nameLine?.text) {
                ctx.save();
                ctx.globalAlpha = baseAlpha * 0.95;
                ctx.font = `${nameLine.size}px ${fontFamily}`;
                ctx.fillStyle = titleColor;
                ctx.textAlign = 'center';
                const nameY = badgeY + paddingY + nameLine.size / 2;
                ctx.fillText(nameLine.text, textCenterX, nameY);
                ctx.restore();
                this.badgeRegions.push({
                    key: 'name',
                    instanceId,
                    rect: {
                        x: textLeft,
                        y: badgeY + paddingY,
                        w: textWidth,
                        h: nameLine.size + lineGap / 2,
                    },
                    truncated: nameLine.truncated,
                    fullText: nameLine.fullText,
                });
                if (nameLine.truncated &&
                    this.hoveredBadgeInstance === instanceId &&
                    this.hoveredBadgeKey === 'name') {
                    this.drawTooltip(ctx, textCenterX, badgeY, nameLine.fullText, fontSize);
                }
            }
            if (authorLineLayout?.text) {
                ctx.save();
                ctx.globalAlpha = baseAlpha * 0.75;
                ctx.font = `${authorLineLayout.size}px ${fontFamily}`;
                ctx.fillStyle = titleColor;
                ctx.textAlign = 'center';
                const authorY = badgeY + paddingY + nameSize + lineGap + authorLineLayout.size / 2;
                ctx.fillText(authorLineLayout.text, textCenterX, authorY);
                ctx.restore();
                this.badgeRegions.push({
                    key: 'author',
                    instanceId,
                    rect: {
                        x: textLeft,
                        y: badgeY + paddingY + nameSize + lineGap / 2,
                        w: textWidth,
                        h: authorLineLayout.size + lineGap / 2,
                    },
                    truncated: authorLineLayout.truncated,
                    fullText: authorLineLayout.fullText,
                });
                if (authorLineLayout.truncated &&
                    this.hoveredBadgeInstance === instanceId &&
                    this.hoveredBadgeKey === 'author') {
                    this.drawTooltip(ctx, textCenterX, badgeY, authorLineLayout.fullText, fontSize);
                }
            }
        }
        for (const { item, pillWidth } of pillEntries) {
            const isHovered = this.isHovered(instanceId, item.key);
            const lift = isHovered ? 1 : 0;
            const pillX = cursorX;
            const pillY = centerY - pillHeight / 2 - lift;
            const pulse = 0;
            const appearAlpha = 1;
            this.drawPill(ctx, pillX, pillY, pillWidth, pillHeight, item.style, {
                hovered: isHovered,
                pulse,
                alpha: appearAlpha,
            });
            if (item.icon) {
                const iconSize = Math.max(12, fontSize - 2);
                const iconX = pillX + (pillWidth - iconSize) / 2;
                const iconY = centerY - iconSize / 2;
                this.drawIcon(ctx, item.icon, iconX, iconY, iconSize, item.color, {
                    now,
                    alpha: appearAlpha,
                });
            }
            else if (item.label) {
                ctx.fillStyle = item.color || '#f3f7fb';
                const align = item.labelAlign === 'center' ? 'center' : 'left';
                const labelSize = Number(item.labelScale) && Number.isFinite(item.labelScale)
                    ? Math.max(10, Math.round(fontSize * Number(item.labelScale)))
                    : fontSize;
                ctx.save();
                ctx.font = `${labelSize}px sans-serif`;
                ctx.textAlign = align;
                const textX = align === 'center' ? pillX + pillWidth / 2 : pillX + paddingX;
                ctx.fillText(item.label, textX, centerY);
                ctx.restore();
            }
            if (item.style === 'action') {
                this.hitRegions.push({
                    key: item.key,
                    tooltip: item.tooltip || '',
                    instanceId,
                    metadata,
                    flavorOptions: item.flavorOptions || null,
                    rect: { x: pillX, y: pillY, w: pillWidth, h: pillHeight },
                });
            }
            if (isHovered && item.tooltip) {
                this.drawTooltip(ctx, pillX + pillWidth / 2, pillY, item.tooltip, fontSize);
            }
            cursorX += pillWidth + gap;
        }
        if (isAnimating) {
            this.requestRedraw();
        }
    }
    drawPill(ctx, x, y, w, h, style, options = {}) {
        const radius = Math.min(10, h / 2);
        const hovered = Boolean(options.hovered);
        const pulse = Number(options.pulse) || 0;
        const alpha = Number.isFinite(options.alpha) ? Number(options.alpha) : 1;
        const shadowBoost = hovered ? 8 : 0;
        const pulseBoost = pulse ? 10 * pulse : 0;
        ctx.save();
        ctx.shadowColor = 'rgba(80, 180, 255, 0.35)';
        ctx.shadowBlur = shadowBoost + pulseBoost;
        ctx.shadowOffsetY = hovered ? 2 : 0;
        ctx.globalAlpha = Math.max(0, Math.min(1, alpha));
        ctx.beginPath();
        ctx.moveTo(x + radius, y);
        ctx.lineTo(x + w - radius, y);
        ctx.arcTo(x + w, y, x + w, y + radius, radius);
        ctx.lineTo(x + w, y + h - radius);
        ctx.arcTo(x + w, y + h, x + w - radius, y + h, radius);
        ctx.lineTo(x + radius, y + h);
        ctx.arcTo(x, y + h, x, y + h - radius, radius);
        ctx.lineTo(x, y + radius);
        ctx.arcTo(x, y, x + radius, y, radius);
        ctx.closePath();
        if (style === 'danger') {
            ctx.fillStyle = '#d3514a';
            ctx.strokeStyle = 'rgba(0, 0, 0, 0.35)';
        }
        else if (style === 'badge') {
            ctx.fillStyle = 'rgba(18, 32, 40, 0.6)';
            ctx.strokeStyle = 'rgba(255, 255, 255, 0.12)';
        }
        else {
            ctx.fillStyle = hovered ? 'rgba(26, 50, 64, 0.85)' : 'rgba(18, 32, 40, 0.72)';
            ctx.strokeStyle = hovered ? 'rgba(120, 200, 255, 0.4)' : 'rgba(255, 255, 255, 0.15)';
        }
        ctx.lineWidth = 1;
        ctx.fill();
        ctx.stroke();
        ctx.restore();
    }
    drawIcon(ctx, icon, x, y, size, color, options = {}) {
        const alpha = Number.isFinite(options.alpha) ? Number(options.alpha) : 1;
        ctx.save();
        ctx.globalAlpha = Math.max(0, Math.min(1, alpha));
        if (icon === 'pen') {
            this.drawPenIcon(ctx, x, y, size, color || '#f3f7fb');
            ctx.restore();
            return;
        }
        if (icon === 'cube') {
            this.drawCubeIcon(ctx, x, y, size, color || '#f3f7fb');
            ctx.restore();
            return;
        }
        ctx.restore();
    }
    drawPenIcon(ctx, x, y, size, color) {
        const thickness = Math.max(2, size * 0.18);
        ctx.save();
        ctx.translate(x + size * 0.5, y + size * 0.5);
        ctx.rotate(-Math.PI / 4);
        ctx.fillStyle = color;
        ctx.strokeStyle = color;
        ctx.lineWidth = thickness;
        ctx.beginPath();
        ctx.moveTo(-size * 0.35, 0);
        ctx.lineTo(size * 0.25, 0);
        ctx.stroke();
        ctx.beginPath();
        ctx.moveTo(size * 0.25, -size * 0.12);
        ctx.lineTo(size * 0.42, 0);
        ctx.lineTo(size * 0.25, size * 0.12);
        ctx.closePath();
        ctx.fill();
        ctx.restore();
    }
    drawCubeIcon(ctx, x, y, size, color) {
        const stroke = Math.max(1.2, size * 0.1);
        const inset = size * 0.18;
        const offset = size * 0.16;
        const front = {
            x: x + inset,
            y: y + inset + offset,
            w: size - inset * 2 - offset,
            h: size - inset * 2 - offset,
        };
        ctx.save();
        ctx.strokeStyle = color;
        ctx.lineWidth = stroke;
        ctx.beginPath();
        ctx.rect(front.x, front.y, front.w, front.h);
        ctx.stroke();
        ctx.beginPath();
        ctx.moveTo(front.x, front.y);
        ctx.lineTo(front.x + offset, front.y - offset);
        ctx.lineTo(front.x + front.w + offset, front.y - offset);
        ctx.lineTo(front.x + front.w, front.y);
        ctx.stroke();
        ctx.beginPath();
        ctx.moveTo(front.x + front.w, front.y);
        ctx.lineTo(front.x + front.w + offset, front.y - offset);
        ctx.lineTo(front.x + front.w + offset, front.y + front.h - offset);
        ctx.lineTo(front.x + front.w, front.y + front.h);
        ctx.stroke();
        ctx.restore();
    }
    drawDefinitionIcon(ctx, model, x, y, size, options = {}) {
        const alpha = Number.isFinite(options.alpha) ? Number(options.alpha) : 1;
        ctx.save();
        ctx.globalAlpha = Math.max(0, Math.min(1, alpha));
        if (model?.kind === 'asset') {
            const entry = this.iconResolver.getImage(model);
            if (entry.status === 'ready' && entry.image) {
                this.drawContainedImage(ctx, entry.image, x, y, size);
                ctx.restore();
                return;
            }
        }
        if (model?.kind === 'initials' || model?.initials) {
            drawFallbackInitialsCanvas(ctx, model, x, y, size);
            ctx.restore();
            return;
        }
        this.drawCubeIcon(ctx, x + size * 0.16, y + size * 0.14, size * 0.72, '#ffffff');
        ctx.restore();
    }
    drawContainedImage(ctx, image, x, y, size) {
        if (typeof ctx.drawImage !== 'function') {
            return;
        }
        const width = Number(image?.naturalWidth || image?.width) || size;
        const height = Number(image?.naturalHeight || image?.height) || size;
        const scale = Math.min(size / width, size / height);
        const drawWidth = width * scale;
        const drawHeight = height * scale;
        const drawX = x + (size - drawWidth) / 2;
        const drawY = y + (size - drawHeight) / 2;
        ctx.drawImage(image, drawX, drawY, drawWidth, drawHeight);
    }
    handleMouseDown(event, canvasInstance) {
        if (!event || !canvasInstance || !this.hitRegions.length) {
            return false;
        }
        const point = this.convertEventToCanvasPoint(event, canvasInstance);
        if (!point) {
            return false;
        }
        for (const region of this.hitRegions) {
            const { x, y, w, h } = region.rect;
            if (point[0] >= x && point[0] <= x + w && point[1] >= y && point[1] <= y + h) {
                const handlers = {
                    'swap-left': () => this.actions.onSwapLeft?.(region.metadata),
                    'swap-right': () => this.actions.onSwapRight?.(region.metadata),
                    menu: () => {
                        const liteGraph = typeof globalThis !== 'undefined' ? globalThis.LiteGraph : null;
                        if (!liteGraph?.ContextMenu) {
                            return;
                        }
                        const options = this.buildMenuOptions({
                            metadata: region.metadata,
                            isDirty: Boolean(region.metadata?.dirty),
                            flavors: region.flavorOptions || [],
                        });
                        new liteGraph.ContextMenu(options, { event });
                    },
                };
                const handler = handlers[region.key];
                if (handler) {
                    handler();
                    return true;
                }
                return false;
            }
        }
        return false;
    }
    handlePointerMove(event, canvasInstance) {
        if (!event || !canvasInstance) {
            return false;
        }
        const point = this.convertEventToCanvasPoint(event, canvasInstance);
        if (!point) {
            return false;
        }
        let nextKey = null;
        let nextInstance = null;
        let nextBadgeInstance = null;
        let nextBadgeKey = null;
        for (const region of this.badgeRegions) {
            const { x, y, w, h } = region.rect;
            if (point[0] >= x && point[0] <= x + w && point[1] >= y && point[1] <= y + h) {
                nextBadgeInstance = region.instanceId || null;
                nextBadgeKey = region.key || null;
                break;
            }
        }
        for (const region of this.hitRegions) {
            const { x, y, w, h } = region.rect;
            if (point[0] >= x && point[0] <= x + w && point[1] >= y && point[1] <= y + h) {
                nextKey = region.key;
                nextInstance = region.instanceId || null;
                break;
            }
        }
        if (nextKey !== this.hoveredKey ||
            nextInstance !== this.hoveredInstance ||
            nextBadgeInstance !== this.hoveredBadgeInstance ||
            nextBadgeKey !== this.hoveredBadgeKey) {
            this.hoveredKey = nextKey;
            this.hoveredInstance = nextInstance;
            this.hoveredBadgeInstance = nextBadgeInstance;
            this.hoveredBadgeKey = nextBadgeKey;
            this.requestRedraw();
        }
        return Boolean(nextKey);
    }
    drawTooltip(ctx, centerX, topY, text, fontSize) {
        if (!text) {
            return;
        }
        const paddingX = 6;
        const paddingY = 4;
        const tooltipFont = Math.max(10, fontSize - 2);
        ctx.save();
        ctx.font = `${tooltipFont}px sans-serif`;
        const textWidth = ctx.measureText(text).width;
        const width = textWidth + paddingX * 2;
        const height = tooltipFont + paddingY * 2;
        const x = centerX - width / 2;
        const y = topY - height - 6;
        ctx.fillStyle = 'rgba(15, 22, 28, 0.9)';
        ctx.strokeStyle = 'rgba(120, 180, 220, 0.4)';
        ctx.lineWidth = 1;
        const radius = Math.min(6, height / 2);
        ctx.beginPath();
        ctx.moveTo(x + radius, y);
        ctx.lineTo(x + width - radius, y);
        ctx.arcTo(x + width, y, x + width, y + radius, radius);
        ctx.lineTo(x + width, y + height - radius);
        ctx.arcTo(x + width, y + height, x + width - radius, y + height, radius);
        ctx.lineTo(x + radius, y + height);
        ctx.arcTo(x, y + height, x, y + height - radius, radius);
        ctx.lineTo(x, y + radius);
        ctx.arcTo(x, y, x + radius, y, radius);
        ctx.closePath();
        ctx.fill();
        ctx.stroke();
        ctx.fillStyle = '#f3f7fb';
        ctx.textBaseline = 'middle';
        ctx.fillText(text, x + paddingX, y + height / 2);
        ctx.restore();
    }
    isAnimating(state, now) {
        if (!state) {
            return false;
        }
        if (state.appearAt && now - state.appearAt < 1000) {
            return true;
        }
        return false;
    }
    ensureInstanceState(instanceId) {
        if (!instanceId) {
            return { dirty: false, appearAt: 0 };
        }
        const existing = this.instanceState.get(instanceId);
        if (existing) {
            return existing;
        }
        const next = { dirty: false, appearAt: 0 };
        this.instanceState.set(instanceId, next);
        return next;
    }
    isHovered(instanceId, key) {
        if (!key) {
            return false;
        }
        if (!this.hoveredKey || this.hoveredKey !== key) {
            return false;
        }
        if (!instanceId) {
            return true;
        }
        return this.hoveredInstance === instanceId;
    }
    requestRedraw() {
        const appRef = this.adapter?.getApp?.() || null;
        appRef?.canvas?.setDirty?.(true, true);
    }
    convertEventToCanvasPoint(event, canvasInstance) {
        const canvasElement = canvasInstance?.canvas ?? null;
        if (!canvasElement || typeof canvasElement.getBoundingClientRect !== 'function') {
            return null;
        }
        if (typeof canvasInstance.convertEventToCanvasOffset === 'function') {
            try {
                const converted = canvasInstance.convertEventToCanvasOffset(event);
                if (Array.isArray(converted) && converted.length >= 2) {
                    const x = Number(converted[0]);
                    const y = Number(converted[1]);
                    if (Number.isFinite(x) && Number.isFinite(y)) {
                        return [x, y];
                    }
                }
            }
            catch (_error) {
                // ignore event conversion failures
            }
        }
        const rect = canvasElement.getBoundingClientRect();
        const relative = [event.clientX - rect.left, event.clientY - rect.top];
        return this.convertCanvasPoint(canvasInstance, relative);
    }
    convertCanvasPoint(canvasInstance, point) {
        if (!canvasInstance || !Array.isArray(point)) {
            return null;
        }
        try {
            if (typeof canvasInstance.convertCanvasToOffset === 'function') {
                const converted = canvasInstance.convertCanvasToOffset(point);
                if (Array.isArray(converted) && converted.length >= 2) {
                    const x = Number(converted[0]);
                    const y = Number(converted[1]);
                    if (Number.isFinite(x) && Number.isFinite(y)) {
                        return [x, y];
                    }
                }
            }
            const ds = canvasInstance.ds;
            const scale = Number(ds?.scale) || 1;
            const offset = Array.isArray(ds?.offset) ? ds.offset : [0, 0];
            const x = point[0] / scale - (offset[0] ?? 0);
            const y = point[1] / scale - (offset[1] ?? 0);
            if (Number.isFinite(x) && Number.isFinite(y)) {
                return [x, y];
            }
        }
        catch (_error) {
            // ignore conversion failures
        }
        return null;
    }
}

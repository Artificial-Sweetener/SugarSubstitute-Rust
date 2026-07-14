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
 * Own the SugarCubes host integration layer in `web/js/nodes.js`.
 */

import { app } from '/scripts/app.js';
import { wrapMarkerToCube } from './cube_creation.js';
import { getSugarCubesUI } from '../comfyui/ui/index.js';
import {
  findGroupForCubeId,
  findGroupForMarkerId,
  findGroupForNodeId,
  syncInstanceAlias,
} from '../comfyui/ui/graph/InstanceAliasSync.js';
import { getGroupSugarcubes } from '../comfyui/ui/graph/GroupMetadata.js';
import { writeWidgetValue } from '../comfyui/ui/graph/Markers.js';

const ANY = '*';
const NODE_TYPES = new Set(['SugarCubes.CubeInput', 'SugarCubes.CubeOutput']);

const CUBE_MARKER_TYPES = new Set(['SugarCubes.CubeInput', 'SugarCubes.CubeOutput']);

const DEFAULT_TITLES = {
  'SugarCubes.CubeInput': 'Cube Input',
  'SugarCubes.CubeOutput': 'Cube Output',
};

const TYPE_COLORS = (() => {
  const colors = LiteGraph?.LGraphCanvas?.node_colors || {};
  return {
    MODEL: colors.blue,
    LATENT: colors.purple,
    VAE: colors.red,
    CONDITIONING: colors.brown,
    IMAGE: colors.pale_blue,
    CLIP: colors.yellow,
    FLOAT: colors.green,
    MASK: { color: '#1c5715', bgcolor: '#1f401b' },
    INT: { color: '#1b4669', bgcolor: '#29699c' },
    CONTROL_NET: { color: '#156653', bgcolor: '#1c453b' },
    NOISE: { color: '#2e2e2e', bgcolor: '#242121' },
    GUIDER: { color: '#3c7878', bgcolor: '#1c453b' },
    SAMPLER: { color: '#614a4a', bgcolor: '#3b2c2c' },
    SIGMAS: { color: '#485248', bgcolor: '#272e27' },
  };
})();

const state = new WeakMap();
const WIDGET_FLAG = Symbol('sugarcubes:widget');
const CREATE_CUBE_WIDGET_FLAG = Symbol('sugarcubes:create-cube-widget');
const schedule = typeof queueMicrotask === 'function' ? queueMicrotask : (fn) => setTimeout(fn, 0);

function getCurrentUi() {
  return getSugarCubesUI();
}

function scheduleCubeInstanceRefresh(node, reason) {
  const ui = getCurrentUi();
  ui?.scheduleCubeInstanceRefresh?.({ graph: node?.graph, reason });
  ui?.scheduleCubeDirtyRefresh?.({ graph: node?.graph, reason });
}

function baseTitleFor(node) {
  const explicit = DEFAULT_TITLES[node.type];
  if (explicit) return explicit;
  if (typeof node.constructor?.title === 'string' && node.constructor.title) {
    return node.constructor.title;
  }
  if (typeof node.title === 'string' && node.title) {
    const idx = node.title.indexOf('[');
    if (idx > 0) {
      return node.title.slice(0, idx).trim();
    }
    const dash = node.title.indexOf(' - ');
    if (dash > 0) {
      return node.title.slice(0, dash).trim();
    }
    return node.title;
  }
  return node.type.split('.').pop();
}

function ensureState(node) {
  let entry = state.get(node);
  if (!entry) {
    const inputSlot = node.inputs?.[0];
    const outputSlot = node.outputs?.[0];
    entry = {
      baseTitle: baseTitleFor(node),
      inputBaseName: inputSlot?.name ?? 'value',
      outputBaseName: outputSlot?.name ?? 'value',
      defaultColor: node.color,
      defaultBgColor: node.bgcolor,
      type: ANY,
    };
    state.set(node, entry);
  }
  return entry;
}

function normalizeType(raw) {
  if (!raw) return ANY;
  const value = Array.isArray(raw) ? raw[0] : raw;
  if (typeof value !== 'string') return ANY;
  const trimmed = value.trim();
  if (!trimmed || trimmed === ANY) return ANY;
  return trimmed;
}

function resolveLink(graph, linkId) {
  if (!graph || linkId == null) return null;
  if (graph.links) return graph.links[linkId] || null;
  return null;
}

function findNode(graph, nodeId) {
  if (!graph || nodeId == null) return null;
  if (graph.getNodeById) return graph.getNodeById(nodeId);
  return graph._nodes?.find((n) => n.id === nodeId) || null;
}

function getFirstInputLink(node) {
  if (!node.inputs || !node.inputs.length) return null;
  const slot = node.inputs[0];
  if (!slot) return null;
  const linkId =
    slot.link ?? (Array.isArray(slot.links) && slot.links.length ? slot.links[0] : null);
  return resolveLink(node.graph, linkId);
}

function getFirstOutputLink(node) {
  if (!node.outputs || !node.outputs.length) return null;
  const slot = node.outputs[0];
  if (!slot) return null;
  let linkId = null;
  if (Array.isArray(slot.links) && slot.links.length) {
    linkId = slot.links[0];
  } else if (slot.link != null) {
    linkId = slot.link;
  }
  return resolveLink(node.graph, linkId);
}

function detectTypeFromInput(node) {
  const link = getFirstInputLink(node);
  if (!link) return ANY;

  const direct = normalizeType(link.type);
  if (direct !== ANY) return direct;

  const origin = findNode(node.graph, link.origin_id);
  if (origin?.outputs) {
    const sourceSlot = origin.outputs[link.origin_slot];
    const sourceType = normalizeType(sourceSlot?.type);
    if (sourceType !== ANY) return sourceType;
  }

  return ANY;
}

function detectTypeFromOutput(node) {
  const link = getFirstOutputLink(node);
  if (!link) return ANY;

  const direct = normalizeType(link.type);
  if (direct !== ANY) return direct;

  const target = findNode(node.graph, link.target_id);
  if (target?.inputs) {
    const targetSlot = target.inputs[link.target_slot];
    const targetType = normalizeType(targetSlot?.type);
    if (targetType !== ANY) return targetType;
  }

  return ANY;
}

function detectType(node) {
  if (!node) return ANY;

  if (node.type === 'SugarCubes.CubeInput') {
    const outputType = detectTypeFromOutput(node);
    if (outputType !== ANY) return outputType;
    return detectTypeFromInput(node);
  }

  const inputType = detectTypeFromInput(node);
  if (inputType !== ANY) return inputType;
  return detectTypeFromOutput(node);
}

function readDefaultAlias(node) {
  const widget = node.widgets?.find((w) => w.name === 'default_alias');
  return widget && typeof widget.value === 'string' ? widget.value : '';
}

function readCubeId(node) {
  const widget = node.widgets?.find((w) => w.name === 'cube_id');
  return widget && typeof widget.value === 'string' ? widget.value : '';
}

function readInstanceId(node) {
  const widget = node.widgets?.find((w) => w.name === 'instance_id');
  return widget && typeof widget.value === 'string' ? widget.value : '';
}

function readInstanceAlias(node) {
  const widget = node.widgets?.find((w) => w.name === 'instance_alias');
  return widget && typeof widget.value === 'string' ? widget.value : '';
}

function adoptMarkerMetadata(node, linkInfo) {
  if (!node?.graph || !linkInfo) {
    return false;
  }
  if (node.type !== 'SugarCubes.CubeInput' && node.type !== 'SugarCubes.CubeOutput') {
    return false;
  }
  const markerId = readCubeId(node).trim();
  const markerDefaultAlias = readDefaultAlias(node).trim();
  if (markerId && markerDefaultAlias) {
    return false;
  }
  const originId = linkInfo.origin_id ?? linkInfo.origin;
  const targetId = linkInfo.target_id ?? linkInfo.target;
  if (originId == null || targetId == null) {
    return false;
  }
  const otherId = String(originId) === String(node.id) ? targetId : originId;
  const otherNode = findNode(node.graph, otherId);
  if (!otherNode) {
    return false;
  }
  const group = findGroupForNodeId(node.graph, otherNode.id);
  const metadata = group ? getGroupSugarcubes(group) : null;
  const cubeId = typeof metadata?.cube_id === 'string' ? metadata.cube_id.trim() : '';
  const defaultAlias =
    typeof metadata?.default_alias === 'string' ? metadata.default_alias.trim() : '';
  const instanceId = typeof metadata?.instance_id === 'string' ? metadata.instance_id.trim() : '';
  const instanceAlias =
    typeof metadata?.instance_alias === 'string' ? metadata.instance_alias.trim() : '';
  if (!cubeId && !defaultAlias) {
    return false;
  }
  if (!markerId && cubeId) {
    writeWidgetValue(node, 'cube_id', cubeId);
  }
  if (!markerDefaultAlias && defaultAlias) {
    writeWidgetValue(node, 'default_alias', defaultAlias);
  }
  if (!readInstanceAlias(node).trim() && instanceAlias) {
    writeWidgetValue(node, 'instance_alias', instanceAlias);
  }
  if (!readInstanceId(node).trim() && instanceId) {
    writeWidgetValue(node, 'instance_id', instanceId);
  }
  scheduleCubeInstanceRefresh(node, 'cube-marker-adopt');
  return true;
}

function hideCubeIdWidget(node) {
  const widget = node.widgets?.find((w) => w.name === 'cube_id');
  if (!widget) {
    return;
  }
  widget.hidden = true;
  widget.options = { ...widget.options, hidden: true, serialize: true };
  widget.disabled = true;
}

function hideInstanceIdWidget(node) {
  const widget = node.widgets?.find((w) => w.name === 'instance_id');
  if (!widget) {
    return;
  }
  widget.hidden = true;
  widget.options = { ...widget.options, hidden: true, serialize: true };
  widget.disabled = true;
}

function toggleDefaultAliasWidget(node, enabled) {
  const widget = node.widgets?.find((w) => w.name === 'default_alias');
  if (!widget) {
    return;
  }
  widget.hidden = !enabled;
  widget.options = { ...widget.options, hidden: !enabled, serialize: true };
  widget.disabled = !enabled;
}

function toggleInstanceAliasWidget(node, enabled) {
  const widget = node.widgets?.find((w) => w.name === 'instance_alias');
  if (!widget) {
    return;
  }
  widget.hidden = !enabled;
  widget.options = { ...widget.options, hidden: !enabled, serialize: true };
  widget.disabled = !enabled;
}

function getCreateCubeWidget(node) {
  return node.widgets?.find((widget) => widget?.[CREATE_CUBE_WIDGET_FLAG]) || null;
}

function ensureCreateCubeWidget(node) {
  const existing = getCreateCubeWidget(node);
  if (existing || typeof node?.addWidget !== 'function') {
    return existing;
  }
  const widget = node.addWidget(
    'button',
    'Create cube',
    null,
    () => startCreateCubeFromMarker(node),
    { serialize: false },
  );
  if (!widget) {
    return null;
  }
  widget[CREATE_CUBE_WIDGET_FLAG] = true;
  widget.serialize = false;
  widget.options = { ...widget.options, serialize: false };
  return widget;
}

function toggleCreateCubeDecoration(node, enabled) {
  const widget = ensureCreateCubeWidget(node);
  if (!widget) {
    return;
  }
  widget.hidden = !enabled;
  widget.options = { ...widget.options, hidden: !enabled, serialize: false };
  widget.disabled = !enabled;
}

async function startCreateCubeFromMarker(node) {
  const ui = getCurrentUi();
  await ui?.cubeCreation?.startCreateCubeFromMarker?.(node);
  schedule(() => refreshNode(node));
}

function isCubeMarker(node) {
  return Boolean(node && CUBE_MARKER_TYPES.has(node.type));
}

function buildTitle(node, type) {
  const kind = node.type;
  const base = DEFAULT_TITLES[kind] ?? kind.split('.').pop();
  if (type === ANY) {
    return base;
  }
  if (kind === 'SugarCubes.CubeOutput') {
    return `${type} Output`;
  }
  if (kind === 'SugarCubes.CubeInput') {
    return `${type} Input`;
  }
  return base;
}

function applyColors(node, type) {
  const entry = ensureState(node);
  const key = typeof type === 'string' ? type.toUpperCase() : type;
  const colors = TYPE_COLORS[key] || null;
  if (colors && colors.color && colors.bgcolor) {
    node.color = colors.color;
    node.bgcolor = colors.bgcolor;
  } else {
    node.color = entry.defaultColor;
    node.bgcolor = entry.defaultBgColor;
  }
}

function updatePorts(node, type) {
  const entry = ensureState(node);
  const label = type === ANY ? 'value' : type;
  if (node.inputs && node.inputs[0]) {
    const slot = node.inputs[0];
    slot.type = type;
    slot.name = entry.inputBaseName;
    slot.label = label;
  }
  if (node.outputs && node.outputs[0]) {
    const slot = node.outputs[0];
    slot.type = type;
    slot.name = entry.outputBaseName;
    slot.label = label;
  }
}

function touchWidget(node) {
  const widget = node.widgets?.find((w) => w.name === 'default_alias');
  if (widget && !widget[WIDGET_FLAG]) {
    const original = widget.callback;
    widget.callback = function (...args) {
      const previous =
        typeof widget.__sugarcubes_last_value === 'string'
          ? widget.__sugarcubes_last_value
          : typeof widget.last_value === 'string'
            ? widget.last_value
            : typeof widget.value === 'string'
              ? widget.value
              : '';
      const result = original?.apply(this, args);
      const entry = ensureState(node);
      node.title = buildTitle(node, entry.type);
      node.setDirtyCanvas?.(true, true);
      const next =
        typeof args[0] === 'string'
          ? args[0]
          : typeof widget.value === 'string'
            ? widget.value
            : '';
      widget.__sugarcubes_last_value = next;
      if (previous !== next) {
        scheduleCubeInstanceRefresh(node, 'default-alias-widget');
      }
      return result;
    };
    widget[WIDGET_FLAG] = true;
  }
}

function applyType(node, type) {
  const entry = ensureState(node);
  entry.type = type;
  updatePorts(node, type);
  applyColors(node, type);
  node.title = buildTitle(node, type);
  node.setDirtyCanvas?.(true, true);
  node.update?.();
}

function refreshNode(node) {
  if (!node) return;
  touchWidget(node);
  hideCubeIdWidget(node);
  hideInstanceIdWidget(node);
  const hasCubeId = Boolean(readCubeId(node).trim());
  toggleDefaultAliasWidget(node, !hasCubeId);
  toggleInstanceAliasWidget(node, hasCubeId);
  toggleCreateCubeDecoration(node, !hasCubeId);
  const type = detectType(node);
  applyType(node, type);
}

function cleanSlotList(list) {
  if (!Array.isArray(list)) return [];
  return list.filter((value) => value != null && Number(value) >= 0);
}

function syncSlotLinks(node, slotType) {
  if (!node?.graph) return;
  if (slotType === LiteGraph.INPUT || slotType === 1) {
    const slot = node.inputs?.[0];
    if (!slot) return;
    const link = getFirstInputLink(node);
    if (link) {
      slot.link = link.id ?? slot.link;
      if (Array.isArray(slot.links)) {
        slot.links = cleanSlotList(slot.links);
        if (!slot.links.includes(link.id)) {
          slot.links.push(link.id);
        }
      } else {
        slot.links = [link.id];
      }
    } else {
      slot.link = null;
      if (Array.isArray(slot.links)) {
        slot.links.length = 0;
      } else {
        slot.links = [];
      }
    }
  }
  if (slotType === LiteGraph.OUTPUT || slotType === 2) {
    const slot = node.outputs?.[0];
    if (!slot) return;
    if (Array.isArray(slot.links)) {
      slot.links = cleanSlotList(slot.links);
    } else if (slot.link != null) {
      slot.links = cleanSlotList([slot.link]);
    } else {
      slot.links = [];
    }
  }
}

function wrapLifecycle(nodeType) {
  const proto = nodeType.prototype;

  if (proto.__sugarcubes_wrapped) {
    return;
  }
  proto.__sugarcubes_wrapped = true;

  const originalAdded = proto.onAdded;
  proto.onAdded = function (...args) {
    const result = originalAdded?.apply(this, args);
    ensureState(this);
    touchWidget(this);
    schedule(() => refreshNode(this));
    return result;
  };

  const originalConfigure = proto.onConfigure;
  proto.onConfigure = function (...args) {
    const result = originalConfigure?.apply(this, args);
    ensureState(this);
    schedule(() => refreshNode(this));
    return result;
  };

  const originalConnectionsChange = proto.onConnectionsChange;
  proto.onConnectionsChange = function (slotType, _slot, _connect, _linkInfo, _output) {
    const res = originalConnectionsChange?.apply(this, arguments);
    if (slotType === 1 || slotType === 2) {
      refreshNode(this);
      if (_connect && _linkInfo) {
        adoptMarkerMetadata(this, _linkInfo);
      }
    }
    if (typeof LiteGraph !== 'undefined' && this?.graph) {
      schedule(() => syncSlotLinks(this, slotType));
    }
    return res;
  };

  const originalWidgetChanged = proto.onWidgetChanged;
  proto.onWidgetChanged = function (name, value, oldValue, _widget) {
    const res = originalWidgetChanged?.apply(this, arguments);
    if (name === 'default_alias') {
      const previous = typeof oldValue === 'string' ? oldValue : '';
      const next = typeof value === 'string' ? value : '';
      const entry = ensureState(this);
      this.title = buildTitle(this, entry.type);
      this.setDirtyCanvas?.(true, true);
      const cubeId = readCubeId(this).trim();
      if (cubeId) {
        const widget = this.widgets?.find((w) => w.name === 'default_alias');
        if (widget) {
          widget.value = previous;
        }
        return res;
      }
      if (previous !== next) {
        scheduleCubeInstanceRefresh(this, 'default-alias-change');
      }
    }
    if (name === 'instance_alias') {
      const previous = typeof oldValue === 'string' ? oldValue : '';
      const next = typeof value === 'string' ? value : '';
      if (previous !== next) {
        scheduleCubeInstanceRefresh(this, 'instance-alias-change');
        const cubeId = readCubeId(this).trim();
        const instanceAlias = next.trim();
        if (!instanceAlias) {
          return res;
        }
        const group = cubeId
          ? findGroupForCubeId(this.graph, cubeId)
          : findGroupForMarkerId(this.graph, this.id);
        syncInstanceAlias({
          graph: this.graph,
          group,
          metadata: group ? getGroupSugarcubes(group) : null,
          cubeId: cubeId || '',
          instanceAlias,
          events: getCurrentUi()?.events,
        });
      }
    }
    return res;
  };

  const originalExtraMenu = proto.getExtraMenuOptions;
  proto.getExtraMenuOptions = function () {
    const options = originalExtraMenu?.apply(this, arguments) ?? [];
    if (!isCubeMarker(this)) {
      return options;
    }
    if (!readCubeId(this).trim()) {
      options.push({
        content: 'Create SugarCube',
        callback: () => startCreateCubeFromMarker(this),
      });
    }
    options.push({
      content: 'Wrap Into SugarCube',
      callback: async () => {
        const ui = getCurrentUi();
        const currentDefaultAlias = readDefaultAlias(this).trim();
        const defaultAlias =
          currentDefaultAlias ||
          (await ui?.dialogs?.promptText?.({
            title: 'Wrap Into SugarCube',
            message: ['Name the SugarCube that will be created for this marker group.'],
            label: 'Default alias',
            confirmLabel: 'Wrap',
            normalizeValue: (value) => value.trim(),
          })) ||
          '';
        const result = wrapMarkerToCube(this, { defaultAlias });
        if (!result.ok) {
          ui?.toast?.push?.('warn', 'SugarCube wrap failed', result.message);
          return;
        }
        ui?.toast?.push?.('success', 'SugarCube updated', result.message);
      },
    });
    return options;
  };
}

app.registerExtension({
  name: 'SugarCubes.Nodes',
  beforeRegisterNodeDef(nodeType, nodeData) {
    if (!NODE_TYPES.has(nodeData?.name)) return;
    wrapLifecycle(nodeType);
  },
  nodeCreated(node) {
    if (!NODE_TYPES.has(node?.type)) return;
    schedule(() => refreshNode(node));
  },
  loadedGraphNode(node) {
    if (!NODE_TYPES.has(node?.type)) return;
    schedule(() => refreshNode(node));
  },
});

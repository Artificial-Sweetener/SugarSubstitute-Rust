// node_modules/@microsoft/focusgroup-polyfill/build/constants.mjs
var DatasetName = {
  INFERRED_ROLE: "data-fg-ir",
  ITEM: "data-fg-item",
  AUTHOR_TABINDEX: "data-fg-ati",
  SEGMENT: "data-fg-seg",
  SEGMENT_START: "data-fg-segs"
};
var BehaviorToken = {
  TOOLBAR: "toolbar",
  TABLIST: "tablist",
  RADIOGROUP: "radiogroup",
  LISTBOX: "listbox",
  MENU: "menu",
  MENUBAR: "menubar",
  NONE: "none"
};
var BEHAVIOR_TOKENS = [
  BehaviorToken.TOOLBAR,
  BehaviorToken.TABLIST,
  BehaviorToken.RADIOGROUP,
  BehaviorToken.LISTBOX,
  BehaviorToken.MENU,
  BehaviorToken.MENUBAR,
  BehaviorToken.NONE
];

// node_modules/@microsoft/focusgroup-polyfill/build/shadow-utils/index-shadowless.mjs
function nodeContains(node, otherNode) {
  return node.contains(otherNode);
}

// node_modules/@microsoft/focusgroup-polyfill/build/utils-shadowless.mjs
function supportsFocusGroup() {
  return "focusgroup" in (globalThis?.HTMLElement?.prototype ?? {}) || "focusGroup" in (globalThis?.HTMLElement?.prototype ?? {});
}
function getNavigationDirection(event, owner, axis) {
  const FORWARD = "forward";
  const BACKWARD = "backward";
  const BLOCK = "block";
  const INLINE = "inline";
  if (isKeyConflictElement(event.composedPath()[0])) return event.key === "Tab" ? event.shiftKey ? BACKWARD : FORWARD : null;
  if (event.shiftKey || event.ctrlKey || event.metaKey) return null;
  const { writingMode, direction } = window.getComputedStyle(owner);
  const isVertical = !writingMode.startsWith("horizontal-");
  const isRtl = direction === "rtl";
  const horizontal = isVertical ? BLOCK : INLINE;
  const vertical = isVertical ? INLINE : BLOCK;
  const isHorizontalReversed = isVertical ? writingMode.endsWith("-rl") !== isRtl : isRtl;
  const isVerticalReversed = isVertical && isRtl;
  const action = {
    ArrowUp: {
      axis: vertical,
      dir: isVerticalReversed ? FORWARD : BACKWARD
    },
    ArrowDown: {
      axis: vertical,
      dir: isVerticalReversed ? BACKWARD : FORWARD
    },
    ArrowLeft: {
      axis: horizontal,
      dir: isHorizontalReversed ? FORWARD : BACKWARD
    },
    ArrowRight: {
      axis: horizontal,
      dir: isHorizontalReversed ? BACKWARD : FORWARD
    },
    Home: { dir: "start" },
    End: { dir: "end" }
  }[event.key];
  if (!action || axis && action.axis && action.axis !== axis) return null;
  return action.dir;
}
function isKeyConflictElement(el) {
  return el?.nodeType === Node.ELEMENT_NODE && ([
    "INPUT",
    "TEXTAREA",
    "SELECT"
  ].includes(el.nodeName) && !["checkbox", "radio"].includes(el.getAttribute("type")) || el.isContentEditable || ["AUDIO", "VIDEO"].includes(el.nodeName) && el.hasAttribute("controls") || ["IFRAME", "OBJECT"].includes(el.nodeName));
}

// node_modules/@microsoft/focusgroup-polyfill/build/global-state.mjs
globalThis.__FOCUSGROUP_POLYFILL__ ??= {
  o: /* @__PURE__ */ new Set(),
  b: false
};
var state = globalThis.__FOCUSGROUP_POLYFILL__;

// node_modules/@microsoft/focusgroup-polyfill/build/observer-registry.mjs
var observers = state.o;
function flushAllObservers() {
  for (const observer of observers) observer.takeRecords();
}

// node_modules/@microsoft/focusgroup-polyfill/build/focusgroup-shadowless.mjs
var FocusGroup = class {
  /**
  * The focus group owner element.
  * @type {HTMLElement!}
  */
  #owner;
  /**
  * The items collection — exposes the focus group's items and answers
  * queries about them. Reconciliation is triggered externally via
  * `FocusGroup#update()`.
  * @type {FocusGroupItemCollection}
  */
  #items;
  /**
  * The focus group behavior.
  * @type {BehaviorToken | null}
  */
  #behavior = null;
  /**
  * The focus group navigation axis limitation.
  * @type {("inline" | "block" | undefined)}
  */
  #axis = void 0;
  /**
  * Whether the focus group wraps. Defaults to `false`.
  * @type {boolean}
  */
  #wrap = false;
  /**
  * Whether the focus group remembers the previously focused element.
  * Defaults to `true`.
  * @type {boolean}
  */
  #memory = true;
  /**
  * The focus group start element (initial tab stop after decoration).
  * @type {HTMLElement}
  */
  #start;
  /**
  * The current item — the most recently focused item within the group.
  * Serves as the keyboard-navigation cursor while focus is inside, and (in
  * memory mode) as the tab stop to restore on re-entry. Updated by
  * `#handleFocusin`, plus directly by `#handleKeydown` for shadow-internal
  * navigation (where focus events don't cross the shadow boundary).
  * Cleared in nomemory mode on focusout and when validation fails after
  * re-decoration.
  * @type {HTMLElement|null}
  */
  #current = null;
  /**
  * Whether the owner currently has `tabindex=0` set as a Tab-entry proxy so
  * sequential focus navigation can reach a tab stop inside a shadow root.
  * @type {boolean}
  */
  #ownerIsProxy = false;
  /**
  * The owner's original `tabindex` attribute value (or `null` if it had no
  * `tabindex`), saved before the polyfill sets `tabindex=0` for proxy duty.
  * @type {string|null}
  */
  #ownerTabindexBeforeProxy = null;
  /**
  * The abort controller for when `disconnect()` is called.
  * @type {AbortController}
  */
  #abort = new AbortController();
  /**
  * Optional owner-decoration hook injected via `options.decorateOwner`.
  * Called with `(owner, behavior)` on decoration and `(owner, null)` on
  * undecoration. When absent, no owner decoration happens.
  * @type {((element: HTMLElement, behavior: BehaviorToken|null) => void) | undefined}
  */
  #decorateOwner;
  /**
  * Optional item-decoration hook injected via `options.decorateItem`.
  * Called with `(item, behavior)` on decoration and `(item, null)` on
  * undecoration. When absent, no item decoration happens.
  * @type {((element: HTMLElement, behavior: BehaviorToken|null) => void) | undefined}
  */
  #decorateItem;
  /**
  * @param {HTMLElement!} owner - The focus group owner element.
  * @param {FocusGroupItemCollection} items - The items collection providing
  *     item discovery and queries.
  * @param {FocusGroupOptions} [options]
  */
  constructor(owner, items, options = {}) {
    if (supportsFocusGroup() || !owner) return;
    this.#owner = owner;
    this.#items = items;
    this.#decorateOwner = options.decorateOwner;
    this.#decorateItem = options.decorateItem;
    this.#updateDefinition(options.definition);
    this.#decorateOwner?.(this.#owner, this.#behavior);
    this.#decorateItems();
    const opts = { signal: this.#abort.signal };
    this.#owner.addEventListener("keydown", this.#handleKeydown.bind(this), opts);
    this.#owner.addEventListener("focusin", this.#handleFocusin.bind(this), opts);
    this.#owner.addEventListener("focusout", this.#handleFocusout.bind(this), opts);
  }
  /**
  * Tears down the focus group: disables the owner proxy, removes all event
  * listeners (via the abort signal), then disconnects the items collection
  * if it supports it.
  *
  * Ordering matters: owner-proxy teardown can trigger `flushAllObservers()`,
  * which expects the items' observer to still be in the global registry.
  * The items' own `disconnect()` is therefore called last.
  *
  * NOTE: This method does not undecorate the elements. Call it only after
  * the focusgroup owner has been removed from the DOM.
  */
  disconnect() {
    this.#disableFocusabilityProxy();
    this.#abort.abort();
    this.#items?.disconnect?.();
    this.#owner = null;
  }
  /**
  * Reconciles decoration state in response to relevant changes. Call this
  * whenever the focus group should refresh — e.g. items were added or
  * removed, the owner's `focusgroup` attribute changed, or an author set
  * `tabindex` on a decorated item.
  *
  * The polyfill's default `TreeWalkerItemCollection` calls this from a
  * `MutationObserver`. App-supplied collections (or app code that knows
  * when its model changed) can call it directly.
  *
  * @param {FocusGroupUpdateInfo} [info]
  */
  update(info = {}) {
    if (!this.#owner) return;
    if (info.definition !== void 0) {
      this.#updateDefinition(info.definition);
      this.#decorateOwner?.(this.#owner, this.#behavior);
    }
    if (info.authorTabindexChanges) for (const el of info.authorTabindexChanges) el.setAttribute(DatasetName.AUTHOR_TABINDEX, el.getAttribute("tabindex") ?? "none");
    this.#undecorateItems();
    this.#decorateItems();
  }
  /** @param {FocusGroupDefinition} [def] */
  #updateDefinition(def) {
    this.#behavior = def?.behavior ?? null;
    this.#wrap = def?.wrap ?? false;
    this.#axis = def?.axis;
    this.#memory = def?.memory ?? true;
    if (!this.#memory) this.#current = null;
  }
  #decorateItems() {
    if (!this.#behavior || this.#behavior === BehaviorToken.NONE) {
      this.#undecorateItems();
      return;
    }
    this.#items.decorate?.();
    for (const { element, segmentBoundary } of this.#items.items()) {
      this.#decorateItem?.(element, this.#behavior);
      element.setAttribute(DatasetName.AUTHOR_TABINDEX, element.getAttribute("tabindex") ?? "none");
      element.tabIndex = segmentBoundary ? 0 : -1;
    }
    if (!this.#current?.isConnected || !(this.#items.isItem?.(this.#current) ?? this.#items.contains(this.#current))) this.#current = null;
    const startItem = this.#current ?? this.#items.start ?? this.#items.first?.() ?? null;
    if (startItem) {
      startItem.tabIndex = 0;
      this.#start = startItem;
      this.#disableFocusabilityProxy();
      this.#enableFocusabilityProxy(startItem);
    }
    this.#items.flush?.();
  }
  #undecorateItems() {
    this.#disableFocusabilityProxy();
    let undecorated = false;
    for (const { element } of this.#items.items()) {
      undecorated = true;
      this.#decorateItem?.(element, null);
      const authorTabIndex = element.getAttribute(DatasetName.AUTHOR_TABINDEX);
      if (authorTabIndex) {
        if (authorTabIndex === "none") element.removeAttribute("tabindex");
        else element.setAttribute("tabindex", authorTabIndex);
        element.removeAttribute(DatasetName.AUTHOR_TABINDEX);
      }
    }
    this.#items.undecorate?.();
    if (undecorated) this.#items.flush?.();
  }
  /** @param {KeyboardEvent} evt */
  #handleKeydown(evt) {
    const current = evt.composedPath()[0];
    if (evt.defaultPrevented || current === this.#owner) return;
    if (!this.#items.contains(current)) return;
    let target;
    switch (getNavigationDirection(evt, current, this.#axis)) {
      case "start":
        target = this.#items.first();
        break;
      case "end":
        target = this.#items.last();
        break;
      case "forward":
        target = this.#items.next(current);
        if (!target && this.#wrap) target = this.#items.first();
        break;
      case "backward":
        target = this.#items.previous(current);
        if (!target && this.#wrap) target = this.#items.last();
        break;
    }
    if (target && target !== current) {
      this.#advanceFocus(current, target, true);
      this.#current = target;
      evt.preventDefault();
    }
  }
  /** @param {FocusEvent} evt */
  #handleFocusin(evt) {
    const target = evt.composedPath()[0];
    if (target === this.#owner && this.#ownerIsProxy && (!evt.relatedTarget || !nodeContains(this.#owner, evt.relatedTarget))) {
      const tabStop = this.#current || this.#start;
      this.#disableFocusabilityProxy();
      if (tabStop) tabStop.focus();
      evt.stopPropagation();
      return;
    }
    if (!this.#items.contains(target)) return;
    if (this.#ownerIsProxy) this.#disableFocusabilityProxy();
    const prev = this.#current;
    this.#current = target;
    if (prev === target) return;
    if (target.tabIndex < 0) {
      const transferFrom = prev ?? this.#start;
      if (transferFrom) this.#advanceFocus(transferFrom, target);
    }
  }
  /** @param {FocusEvent} evt */
  #handleFocusout(evt) {
    if (!evt.relatedTarget || !nodeContains(this.#owner, evt.relatedTarget)) {
      const tabStop = this.#memory ? this.#current || this.#start : this.#start;
      if (tabStop) this.#enableFocusabilityProxy(tabStop);
    }
    if (evt.relatedTarget && nodeContains(this.#owner, evt.relatedTarget) || this.#memory || !this.#start) return;
    const prev = this.#current;
    this.#current = null;
    const nextStart = this.#items.start ?? this.#items.first?.() ?? null;
    if (prev !== this.#start || nextStart !== this.#start) {
      for (const { element, segmentBoundary } of this.#items.items()) element.tabIndex = segmentBoundary ? 0 : -1;
      if (nextStart) {
        nextStart.tabIndex = 0;
        this.#start = nextStart;
      }
      this.#items.flush?.();
    }
  }
  /**
  * If the tab stop is inside a shadow DOM, sets `tabindex=0` on the
  * focusgroup owner so the browser's Tab navigation can land on it, at
  * which point `#handleFocusin` will redirect focus to the real tab stop.
  * @param {HTMLElement} tabStop - The actual focusable tab stop element.
  */
  #enableFocusabilityProxy(tabStop) {
    const rootNode = (tabStop.assignedSlot ?? tabStop).getRootNode();
    const hasFocusableHost = rootNode instanceof ShadowRoot && rootNode.host.hasAttribute(DatasetName.AUTHOR_TABINDEX);
    if (this.#ownerIsProxy || !hasFocusableHost) return;
    this.#ownerTabindexBeforeProxy = this.#owner.getAttribute("tabindex");
    this.#owner.tabIndex = 0;
    this.#ownerIsProxy = true;
    flushAllObservers();
  }
  /** Undoes `#enableFocusabilityProxy`. */
  #disableFocusabilityProxy() {
    if (!this.#ownerIsProxy) return;
    if (this.#ownerTabindexBeforeProxy !== null) this.#owner.setAttribute("tabindex", this.#ownerTabindexBeforeProxy);
    else this.#owner.removeAttribute("tabindex");
    this.#ownerIsProxy = false;
    this.#ownerTabindexBeforeProxy = null;
    this.#items.flush?.();
    flushAllObservers();
  }
  /**
  * Advances the focusgroup's active tab stop from one item to another. Sets
  * the target's `tabindex` to `0` and optionally calls `focus()` on it. The
  * previous item's `tabindex` is set to `-1` unless it belongs to a different
  * segment (in which case it remains `0` as a segment tab stop). Also disables
  * the owner proxy.
  *
  * @param {HTMLElement} prev - The currently focused item.
  * @param {HTMLElement} next - The item to receive focus.
  * @param {boolean} [shouldCallFocus=false] - Whether to programmatically
  *     call `focus()` on the target element.
  */
  #advanceFocus(prev, next2, shouldCallFocus = false) {
    next2.tabIndex = 0;
    if (shouldCallFocus) next2.focus();
    prev.tabIndex = this.#items.sameSegment?.(prev, next2) ?? true ? -1 : 0;
    this.#disableFocusabilityProxy();
    flushAllObservers();
  }
};

// node_modules/@fluentui/web-components/dist/esm/utils/focusgroup.js
var ArrayItemCollection = class {
  constructor(getItems, getStart) {
    this.getItems = getItems;
    this.getStart = getStart;
  }
  get start() {
    return this.getStart?.() ?? null;
  }
  first() {
    return this.getItems()[0] ?? null;
  }
  last() {
    const items = this.getItems();
    return items[items.length - 1] ?? null;
  }
  next(current) {
    const items = this.getItems();
    const i = items.indexOf(current);
    return i === -1 ? null : items[i + 1] ?? null;
  }
  previous(current) {
    const items = this.getItems();
    const i = items.indexOf(current);
    return i <= 0 ? null : items[i - 1] ?? null;
  }
  *items() {
    for (const element of this.getItems()) {
      yield { element };
    }
  }
  contains(element) {
    return this.getItems().includes(element);
  }
};

// node_modules/tslib/tslib.es6.mjs
function __decorate(decorators, target, key, desc) {
  var c = arguments.length, r = c < 3 ? target : desc === null ? desc = Object.getOwnPropertyDescriptor(target, key) : desc, d;
  if (typeof Reflect === "object" && typeof Reflect.decorate === "function") r = Reflect.decorate(decorators, target, key, desc);
  else for (var i = decorators.length - 1; i >= 0; i--) if (d = decorators[i]) r = (c < 3 ? d(r) : c > 3 ? d(target, key, r) : d(target, key)) || r;
  return c > 3 && r && Object.defineProperty(target, key, r), r;
}

// node_modules/@microsoft/fast-element/dist/esm/binding/binding.js
var Binding = class {
  /**
   * Creates a binding.
   * @param evaluate - Evaluates the binding.
   * @param policy - The security policy to associate with this binding.
   * @param isVolatile - Indicates whether the binding is volatile.
   */
  constructor(evaluate, policy, isVolatile = false) {
    this.evaluate = evaluate;
    this.policy = policy;
    this.isVolatile = isVolatile;
  }
};

// node_modules/@microsoft/fast-element/dist/esm/interfaces.js
var Message;
(function(Message2) {
  Message2[Message2["needsArrayObservation"] = 1101] = "needsArrayObservation";
  Message2[Message2["onlySetDOMPolicyOnce"] = 1201] = "onlySetDOMPolicyOnce";
  Message2[Message2["bindingInnerHTMLRequiresTrustedTypes"] = 1202] = "bindingInnerHTMLRequiresTrustedTypes";
  Message2[Message2["twoWayBindingRequiresObservables"] = 1203] = "twoWayBindingRequiresObservables";
  Message2[Message2["hostBindingWithoutHost"] = 1204] = "hostBindingWithoutHost";
  Message2[Message2["unsupportedBindingBehavior"] = 1205] = "unsupportedBindingBehavior";
  Message2[Message2["directCallToHTMLTagNotAllowed"] = 1206] = "directCallToHTMLTagNotAllowed";
  Message2[Message2["onlySetTemplatePolicyOnce"] = 1207] = "onlySetTemplatePolicyOnce";
  Message2[Message2["cannotSetTemplatePolicyAfterCompilation"] = 1208] = "cannotSetTemplatePolicyAfterCompilation";
  Message2[Message2["blockedByDOMPolicy"] = 1209] = "blockedByDOMPolicy";
  Message2[Message2["invalidHydrationAttributeMarker"] = 1210] = "invalidHydrationAttributeMarker";
  Message2[Message2["duplicateRenderInstruction"] = 1211] = "duplicateRenderInstruction";
  Message2[Message2["missingElementDefinition"] = 1401] = "missingElementDefinition";
  Message2[Message2["noRegistrationForContext"] = 1501] = "noRegistrationForContext";
  Message2[Message2["noFactoryForResolver"] = 1502] = "noFactoryForResolver";
  Message2[Message2["invalidResolverStrategy"] = 1503] = "invalidResolverStrategy";
  Message2[Message2["cannotAutoregisterDependency"] = 1504] = "cannotAutoregisterDependency";
  Message2[Message2["cannotResolveKey"] = 1505] = "cannotResolveKey";
  Message2[Message2["cannotConstructNativeFunction"] = 1506] = "cannotConstructNativeFunction";
  Message2[Message2["cannotJITRegisterNonConstructor"] = 1507] = "cannotJITRegisterNonConstructor";
  Message2[Message2["cannotJITRegisterIntrinsic"] = 1508] = "cannotJITRegisterIntrinsic";
  Message2[Message2["cannotJITRegisterInterface"] = 1509] = "cannotJITRegisterInterface";
  Message2[Message2["invalidResolver"] = 1510] = "invalidResolver";
  Message2[Message2["invalidKey"] = 1511] = "invalidKey";
  Message2[Message2["noDefaultResolver"] = 1512] = "noDefaultResolver";
  Message2[Message2["cyclicDependency"] = 1513] = "cyclicDependency";
  Message2[Message2["connectUpdateRequiresController"] = 1514] = "connectUpdateRequiresController";
})(Message || (Message = {}));
var isFunction = (object) => typeof object === "function";
var isString = (object) => typeof object === "string";
var noop = () => void 0;

// node_modules/@microsoft/fast-element/dist/esm/platform.js
var debugMessages = /* @__PURE__ */ Object.create(null);
var FAST = {
  /**
   * Sends a warning to the developer.
   * @param code - The warning code to send.
   * @param values - Values relevant for the warning message.
   */
  warn(_code, _values) {
  },
  /**
   * Creates an error from a code.
   * @param code - The error code.
   * @param values - Values relevant for the error message.
   */
  error(code, _values) {
    return new Error(`Error ${code}`);
  },
  /**
   * Adds debug messages for errors and warnings.
   * @param messages - The message dictionary to add.
   */
  addMessages(messages) {
    Object.assign(debugMessages, messages);
  }
};
var emptyArray = Object.freeze([]);
function createTypeRegistry() {
  const typeToDefinition = /* @__PURE__ */ new Map();
  return Object.freeze({
    register(definition3) {
      if (typeToDefinition.has(definition3.type)) {
        return false;
      }
      typeToDefinition.set(definition3.type, definition3);
      return true;
    },
    getByType(key) {
      return typeToDefinition.get(key);
    },
    getForInstance(object) {
      if (object === null || object === void 0) {
        return void 0;
      }
      return typeToDefinition.get(object.constructor);
    }
  });
}
function createMetadataLocator() {
  const metadataLookup = /* @__PURE__ */ new WeakMap();
  return function(target) {
    let metadata = metadataLookup.get(target);
    if (metadata === void 0) {
      let currentTarget = Reflect.getPrototypeOf(target);
      while (metadata === void 0 && currentTarget !== null) {
        metadata = metadataLookup.get(currentTarget);
        currentTarget = Reflect.getPrototypeOf(currentTarget);
      }
      metadata = metadata === void 0 ? [] : metadata.slice(0);
      metadataLookup.set(target, metadata);
    }
    return metadata;
  };
}
function makeSerializationNoop(type) {
  type.prototype.toJSON = noop;
}

// node_modules/@microsoft/fast-element/dist/esm/binding/one-time.js
var OneTimeBinding = class extends Binding {
  createObserver() {
    return this;
  }
  bind(controller) {
    return this.evaluate(controller.source, controller.context);
  }
};
makeSerializationNoop(OneTimeBinding);
function oneTime(expression, policy) {
  return new OneTimeBinding(expression, policy);
}

// node_modules/@microsoft/fast-element/dist/esm/observation/notifier.js
var SubscriberSet = class {
  /**
   * Creates an instance of SubscriberSet for the specified subject.
   * @param subject - The subject that subscribers will receive notifications from.
   * @param initialSubscriber - An initial subscriber to changes.
   */
  constructor(subject, initialSubscriber) {
    this.sub1 = void 0;
    this.sub2 = void 0;
    this.spillover = void 0;
    this.subject = subject;
    this.sub1 = initialSubscriber;
  }
  /**
   * Checks whether the provided subscriber has been added to this set.
   * @param subscriber - The subscriber to test for inclusion in this set.
   */
  has(subscriber) {
    return this.spillover === void 0 ? this.sub1 === subscriber || this.sub2 === subscriber : this.spillover.indexOf(subscriber) !== -1;
  }
  /**
   * Subscribes to notification of changes in an object's state.
   * @param subscriber - The object that is subscribing for change notification.
   */
  subscribe(subscriber) {
    const spillover = this.spillover;
    if (spillover === void 0) {
      if (this.has(subscriber)) {
        return;
      }
      if (this.sub1 === void 0) {
        this.sub1 = subscriber;
        return;
      }
      if (this.sub2 === void 0) {
        this.sub2 = subscriber;
        return;
      }
      this.spillover = [this.sub1, this.sub2, subscriber];
      this.sub1 = void 0;
      this.sub2 = void 0;
    } else {
      const index = spillover.indexOf(subscriber);
      if (index === -1) {
        spillover.push(subscriber);
      }
    }
  }
  /**
   * Unsubscribes from notification of changes in an object's state.
   * @param subscriber - The object that is unsubscribing from change notification.
   */
  unsubscribe(subscriber) {
    const spillover = this.spillover;
    if (spillover === void 0) {
      if (this.sub1 === subscriber) {
        this.sub1 = void 0;
      } else if (this.sub2 === subscriber) {
        this.sub2 = void 0;
      }
    } else {
      const index = spillover.indexOf(subscriber);
      if (index !== -1) {
        spillover.splice(index, 1);
      }
    }
  }
  /**
   * Notifies all subscribers.
   * @param args - Data passed along to subscribers during notification.
   */
  notify(args) {
    const spillover = this.spillover;
    const subject = this.subject;
    if (spillover === void 0) {
      const sub1 = this.sub1;
      const sub2 = this.sub2;
      if (sub1 !== void 0) {
        sub1.handleChange(subject, args);
      }
      if (sub2 !== void 0) {
        sub2.handleChange(subject, args);
      }
    } else {
      for (let i = 0, ii = spillover.length; i < ii; ++i) {
        spillover[i].handleChange(subject, args);
      }
    }
  }
};
var PropertyChangeNotifier = class {
  /**
   * Creates an instance of PropertyChangeNotifier for the specified subject.
   * @param subject - The object that subscribers will receive notifications for.
   */
  constructor(subject) {
    this.subscribers = {};
    this.subjectSubscribers = null;
    this.subject = subject;
  }
  /**
   * Notifies all subscribers, based on the specified property.
   * @param propertyName - The property name, passed along to subscribers during notification.
   */
  notify(propertyName) {
    var _a2, _b;
    (_a2 = this.subscribers[propertyName]) === null || _a2 === void 0 ? void 0 : _a2.notify(propertyName);
    (_b = this.subjectSubscribers) === null || _b === void 0 ? void 0 : _b.notify(propertyName);
  }
  /**
   * Subscribes to notification of changes in an object's state.
   * @param subscriber - The object that is subscribing for change notification.
   * @param propertyToWatch - The name of the property that the subscriber is interested in watching for changes.
   */
  subscribe(subscriber, propertyToWatch) {
    var _a2, _b;
    let subscribers;
    if (propertyToWatch) {
      subscribers = (_a2 = this.subscribers[propertyToWatch]) !== null && _a2 !== void 0 ? _a2 : this.subscribers[propertyToWatch] = new SubscriberSet(this.subject);
    } else {
      subscribers = (_b = this.subjectSubscribers) !== null && _b !== void 0 ? _b : this.subjectSubscribers = new SubscriberSet(this.subject);
    }
    subscribers.subscribe(subscriber);
  }
  /**
   * Unsubscribes from notification of changes in an object's state.
   * @param subscriber - The object that is unsubscribing from change notification.
   * @param propertyToUnwatch - The name of the property that the subscriber is no longer interested in watching.
   */
  unsubscribe(subscriber, propertyToUnwatch) {
    var _a2, _b;
    if (propertyToUnwatch) {
      (_a2 = this.subscribers[propertyToUnwatch]) === null || _a2 === void 0 ? void 0 : _a2.unsubscribe(subscriber);
    } else {
      (_b = this.subjectSubscribers) === null || _b === void 0 ? void 0 : _b.unsubscribe(subscriber);
    }
  }
};

// node_modules/@microsoft/fast-element/dist/esm/observation/update-queue.js
var tasks = [];
var pendingErrors = [];
var rAF = globalThis.requestAnimationFrame;
var updateAsync = true;
function throwFirstError() {
  if (pendingErrors.length) {
    throw pendingErrors.shift();
  }
}
function tryRunTask(task) {
  try {
    task.call();
  } catch (error) {
    if (updateAsync) {
      pendingErrors.push(error);
      setTimeout(throwFirstError, 0);
    } else {
      tasks.length = 0;
      throw error;
    }
  }
}
function process() {
  const capacity = 1024;
  let index = 0;
  while (index < tasks.length) {
    tryRunTask(tasks[index]);
    index++;
    if (index > capacity) {
      for (let scan = 0, newLength = tasks.length - index; scan < newLength; scan++) {
        tasks[scan] = tasks[scan + index];
      }
      tasks.length -= index;
      index = 0;
    }
  }
  tasks.length = 0;
}
function enqueue(callable) {
  tasks.push(callable);
  if (tasks.length < 2) {
    updateAsync ? rAF(process) : process();
  }
}
var Updates = Object.freeze({
  enqueue,
  next: () => new Promise(enqueue),
  process,
  setMode: (isAsync) => updateAsync = isAsync
});

// node_modules/@microsoft/fast-element/dist/esm/observation/observable.js
var SourceLifetime = Object.freeze({
  /**
   * The source to controller lifetime relationship is unknown.
   */
  unknown: void 0,
  /**
   * The source and controller lifetimes are coupled to one another.
   * They can/will be GC'd together.
   */
  coupled: 1
});
var Observable = (() => {
  const queueUpdate = Updates.enqueue;
  const volatileRegex = /(:|&&|\|\||if|\?\.)/;
  const notifierLookup = /* @__PURE__ */ new WeakMap();
  let watcher = void 0;
  let createArrayObserver = (array) => {
    throw FAST.error(Message.needsArrayObservation);
  };
  function getNotifier(source) {
    var _a2;
    let found = (_a2 = source.$fastController) !== null && _a2 !== void 0 ? _a2 : notifierLookup.get(source);
    if (found === void 0) {
      Array.isArray(source) ? found = createArrayObserver(source) : notifierLookup.set(source, found = new PropertyChangeNotifier(source));
    }
    return found;
  }
  const getAccessors = createMetadataLocator();
  class DefaultObservableAccessor {
    constructor(name) {
      this.name = name;
      this.field = `_${name}`;
      this.callback = `${name}Changed`;
    }
    getValue(source) {
      if (watcher !== void 0) {
        watcher.watch(source, this.name);
      }
      return source[this.field];
    }
    setValue(source, newValue) {
      const field = this.field;
      const oldValue = source[field];
      if (oldValue !== newValue) {
        source[field] = newValue;
        const callback = source[this.callback];
        if (isFunction(callback)) {
          callback.call(source, oldValue, newValue);
        }
        getNotifier(source).notify(this.name);
      }
    }
  }
  class ExpressionNotifierImplementation extends SubscriberSet {
    constructor(expression, initialSubscriber, isVolatileBinding = false) {
      super(expression, initialSubscriber);
      this.expression = expression;
      this.isVolatileBinding = isVolatileBinding;
      this.needsRefresh = true;
      this.needsQueue = true;
      this.isAsync = true;
      this.first = this;
      this.last = null;
      this.propertySource = void 0;
      this.propertyName = void 0;
      this.notifier = void 0;
      this.next = void 0;
    }
    setMode(isAsync) {
      this.isAsync = this.needsQueue = isAsync;
    }
    bind(controller) {
      const value = this.observe(controller.source, controller.context);
      if (!controller.isBound && this.requiresUnbind(controller)) {
        controller.onUnbind(this);
      }
      return value;
    }
    requiresUnbind(controller) {
      return controller.sourceLifetime !== SourceLifetime.coupled || this.first !== this.last || this.first.propertySource !== controller.source;
    }
    unbind(controller) {
      this.dispose();
    }
    observe(source, context) {
      if (this.needsRefresh && this.last !== null) {
        this.dispose();
      }
      const previousWatcher = watcher;
      watcher = this.needsRefresh ? this : void 0;
      this.needsRefresh = this.isVolatileBinding;
      let result;
      try {
        result = this.expression(source, context);
      } finally {
        watcher = previousWatcher;
      }
      return result;
    }
    // backwards compat with v1 kernel
    disconnect() {
      this.dispose();
    }
    dispose() {
      if (this.last !== null) {
        let current = this.first;
        while (current !== void 0) {
          current.notifier.unsubscribe(this, current.propertyName);
          current = current.next;
        }
        this.last = null;
        this.needsRefresh = this.needsQueue = this.isAsync;
      }
    }
    watch(propertySource, propertyName) {
      const prev = this.last;
      const notifier = getNotifier(propertySource);
      const current = prev === null ? this.first : {};
      current.propertySource = propertySource;
      current.propertyName = propertyName;
      current.notifier = notifier;
      notifier.subscribe(this, propertyName);
      if (prev !== null) {
        if (!this.needsRefresh) {
          let prevValue;
          watcher = void 0;
          prevValue = prev.propertySource[prev.propertyName];
          watcher = this;
          if (propertySource === prevValue) {
            this.needsRefresh = true;
          }
        }
        prev.next = current;
      }
      this.last = current;
    }
    handleChange() {
      if (this.needsQueue) {
        this.needsQueue = false;
        queueUpdate(this);
      } else if (!this.isAsync) {
        this.call();
      }
    }
    call() {
      if (this.last !== null) {
        this.needsQueue = this.isAsync;
        this.notify(this);
      }
    }
    *records() {
      let next2 = this.first;
      while (next2 !== void 0) {
        yield next2;
        next2 = next2.next;
      }
    }
  }
  makeSerializationNoop(ExpressionNotifierImplementation);
  return Object.freeze({
    /**
     * @internal
     * @param factory - The factory used to create array observers.
     */
    setArrayObserverFactory(factory) {
      createArrayObserver = factory;
    },
    /**
     * Gets a notifier for an object or Array.
     * @param source - The object or Array to get the notifier for.
     */
    getNotifier,
    /**
     * Records a property change for a source object.
     * @param source - The object to record the change against.
     * @param propertyName - The property to track as changed.
     */
    track(source, propertyName) {
      watcher && watcher.watch(source, propertyName);
    },
    /**
     * Notifies watchers that the currently executing property getter or function is volatile
     * with respect to its observable dependencies.
     */
    trackVolatile() {
      watcher && (watcher.needsRefresh = true);
    },
    /**
     * Notifies subscribers of a source object of changes.
     * @param source - the object to notify of changes.
     * @param args - The change args to pass to subscribers.
     */
    notify(source, args) {
      getNotifier(source).notify(args);
    },
    /**
     * Defines an observable property on an object or prototype.
     * @param target - The target object to define the observable on.
     * @param nameOrAccessor - The name of the property to define as observable;
     * or a custom accessor that specifies the property name and accessor implementation.
     */
    defineProperty(target, nameOrAccessor) {
      if (isString(nameOrAccessor)) {
        nameOrAccessor = new DefaultObservableAccessor(nameOrAccessor);
      }
      getAccessors(target).push(nameOrAccessor);
      Reflect.defineProperty(target, nameOrAccessor.name, {
        enumerable: true,
        get() {
          return nameOrAccessor.getValue(this);
        },
        set(newValue) {
          nameOrAccessor.setValue(this, newValue);
        }
      });
    },
    /**
     * Finds all the observable accessors defined on the target,
     * including its prototype chain.
     * @param target - The target object to search for accessor on.
     */
    getAccessors,
    /**
     * Creates a {@link ExpressionNotifier} that can watch the
     * provided {@link Expression} for changes.
     * @param expression - The binding to observe.
     * @param initialSubscriber - An initial subscriber to changes in the binding value.
     * @param isVolatileBinding - Indicates whether the binding's dependency list must be re-evaluated on every value evaluation.
     */
    binding(expression, initialSubscriber, isVolatileBinding = this.isVolatileBinding(expression)) {
      return new ExpressionNotifierImplementation(expression, initialSubscriber, isVolatileBinding);
    },
    /**
     * Determines whether a binding expression is volatile and needs to have its dependency list re-evaluated
     * on every evaluation of the value.
     * @param expression - The binding to inspect.
     */
    isVolatileBinding(expression) {
      return volatileRegex.test(expression.toString());
    }
  });
})();
function observable(target, nameOrAccessor) {
  Observable.defineProperty(target, nameOrAccessor);
}
var contextEvent = /* @__PURE__ */ (() => {
  let current = null;
  return {
    get() {
      return current;
    },
    set(event) {
      current = event;
    }
  };
})();
var ExecutionContext = Object.freeze({
  /**
   * A default execution context.
   */
  default: {
    index: 0,
    length: 0,
    get event() {
      return ExecutionContext.getEvent();
    },
    eventDetail() {
      return this.event.detail;
    },
    eventTarget() {
      return this.event.target;
    }
  },
  /**
   * Gets the current event.
   * @returns An event object.
   */
  getEvent() {
    return contextEvent.get();
  },
  /**
   * Sets the current event.
   * @param event - An event object.
   */
  setEvent(event) {
    contextEvent.set(event);
  }
});

// node_modules/@microsoft/fast-element/dist/esm/binding/one-way.js
var OneWayBinding = class extends Binding {
  createObserver(subscriber) {
    return Observable.binding(this.evaluate, subscriber, this.isVolatile);
  }
};
function oneWay(expression, policy, isVolatile = Observable.isVolatileBinding(expression)) {
  return new OneWayBinding(expression, policy, isVolatile);
}

// node_modules/@microsoft/fast-element/dist/esm/dom.js
var DOMAspect = Object.freeze({
  /**
   * Not aspected.
   */
  none: 0,
  /**
   * An attribute.
   */
  attribute: 1,
  /**
   * A boolean attribute.
   */
  booleanAttribute: 2,
  /**
   * A property.
   */
  property: 3,
  /**
   * Content
   */
  content: 4,
  /**
   * A token list.
   */
  tokenList: 5,
  /**
   * An event.
   */
  event: 6
});
var createHTML = (html2) => html2;
var fastTrustedType = globalThis.trustedTypes ? globalThis.trustedTypes.createPolicy("fast-html", { createHTML }) : { createHTML };
var defaultPolicy = Object.freeze({
  createHTML(value) {
    return fastTrustedType.createHTML(value);
  },
  protect(tagName3, aspect, aspectName, sink) {
    return sink;
  }
});
var fastPolicy = defaultPolicy;
var DOM = Object.freeze({
  /**
   * Gets the dom policy used by the templating system.
   */
  get policy() {
    return defaultPolicy;
  },
  /**
   * Sets the dom policy used by the templating system.
   * @param policy - The policy to set.
   * @remarks
   * This API can only be called once, for security reasons. It should be
   * called by the application developer at the start of their program.
   */
  setPolicy(value) {
    if (defaultPolicy !== fastPolicy) {
      throw FAST.error(Message.onlySetDOMPolicyOnce);
    }
    defaultPolicy = value;
  },
  /**
   * Sets an attribute value on an element.
   * @param element - The element to set the attribute value on.
   * @param attributeName - The attribute name to set.
   * @param value - The value of the attribute to set.
   * @remarks
   * If the value is `null` or `undefined`, the attribute is removed, otherwise
   * it is set to the provided value using the standard `setAttribute` API.
   */
  setAttribute(element, attributeName, value) {
    value === null || value === void 0 ? element.removeAttribute(attributeName) : element.setAttribute(attributeName, value);
  },
  /**
   * Sets a boolean attribute value.
   * @param element - The element to set the boolean attribute value on.
   * @param attributeName - The attribute name to set.
   * @param value - The value of the attribute to set.
   * @remarks
   * If the value is true, the attribute is added; otherwise it is removed.
   */
  setBooleanAttribute(element, attributeName, value) {
    value ? element.setAttribute(attributeName, "") : element.removeAttribute(attributeName);
  }
});

// node_modules/@microsoft/fast-element/dist/esm/components/attributes.js
var booleanMode = "boolean";
var reflectMode = "reflect";
var AttributeConfiguration = Object.freeze({
  /**
   * Locates all attribute configurations associated with a type.
   */
  locate: createMetadataLocator()
});
var booleanConverter = {
  toView(value) {
    return value ? "" : null;
  },
  fromView(value) {
    return !!value;
  }
};
var AttributeDefinition = class _AttributeDefinition {
  /**
   * Creates an instance of AttributeDefinition.
   * @param Owner - The class constructor that owns this attribute.
   * @param name - The name of the property associated with the attribute.
   * @param attribute - The name of the attribute in HTML.
   * @param mode - The {@link AttributeMode} that describes the behavior of this attribute.
   * @param converter - A {@link ValueConverter} that integrates with the property getter/setter
   * to convert values to and from a DOM string.
   */
  constructor(Owner, name, attribute = name.toLowerCase(), mode = reflectMode, converter) {
    this.guards = /* @__PURE__ */ new Set();
    this.Owner = Owner;
    this.name = name;
    this.attribute = attribute;
    this.mode = mode;
    this.converter = converter;
    this.fieldName = `_${name}`;
    this.callbackName = `${name}Changed`;
    this.hasCallback = this.callbackName in Owner.prototype;
    if (mode === booleanMode && converter === void 0) {
      this.converter = booleanConverter;
    }
  }
  /**
   * Sets the value of the attribute/property on the source element.
   * @param source - The source element to access.
   * @param newValue - The value to set the attribute/property to.
   */
  setValue(source, newValue) {
    const oldValue = source[this.fieldName];
    const converter = this.converter;
    if (converter !== void 0) {
      newValue = converter.fromView(newValue);
    }
    if (oldValue !== newValue) {
      source[this.fieldName] = newValue;
      this.tryReflectToAttribute(source);
      if (this.hasCallback) {
        source[this.callbackName](oldValue, newValue);
      }
      source.$fastController.notify(this.name);
    }
  }
  /**
   * Gets the value of the attribute/property on the source element.
   * @param source - The source element to access.
   */
  getValue(source) {
    Observable.track(source, this.name);
    return source[this.fieldName];
  }
  /** @internal */
  onAttributeChangedCallback(element, value) {
    if (this.guards.has(element)) {
      return;
    }
    this.guards.add(element);
    if (this.mode === booleanMode) {
      this.setValue(element, value !== null);
    } else {
      this.setValue(element, value);
    }
    this.guards.delete(element);
  }
  tryReflectToAttribute(element) {
    const mode = this.mode;
    const guards = this.guards;
    if (guards.has(element) || mode === "fromView") {
      return;
    }
    Updates.enqueue(() => {
      guards.add(element);
      const latestValue = element[this.fieldName];
      switch (mode) {
        case reflectMode: {
          const converter = this.converter;
          DOM.setAttribute(element, this.attribute, converter !== void 0 ? converter.toView(latestValue) : latestValue);
          break;
        }
        case booleanMode:
          DOM.setBooleanAttribute(element, this.attribute, latestValue);
          break;
      }
      guards.delete(element);
    });
  }
  /**
   * Collects all attribute definitions associated with the owner.
   * @param Owner - The class constructor to collect attribute for.
   * @param attributeLists - Any existing attributes to collect and merge with those associated with the owner.
   * @internal
   */
  static collect(Owner, ...attributeLists) {
    const attributes = [];
    attributeLists.push(AttributeConfiguration.locate(Owner));
    for (let i = 0, ii = attributeLists.length; i < ii; ++i) {
      const list = attributeLists[i];
      if (list === void 0) {
        continue;
      }
      for (let j = 0, jj = list.length; j < jj; ++j) {
        const config = list[j];
        if (isString(config)) {
          attributes.push(new _AttributeDefinition(Owner, config));
        } else {
          attributes.push(new _AttributeDefinition(Owner, config.property, config.attribute, config.mode, config.converter));
        }
      }
    }
    return attributes;
  }
};
function attr(configOrTarget, prop) {
  let config;
  function decorator($target, $prop) {
    if (arguments.length > 1) {
      config.property = $prop;
    }
    AttributeConfiguration.locate($target.constructor).push(config);
  }
  if (arguments.length > 1) {
    config = {};
    decorator(configOrTarget, prop);
    return;
  }
  config = configOrTarget === void 0 ? {} : configOrTarget;
  return decorator;
}

// node_modules/@microsoft/fast-element/dist/esm/styles/element-styles.js
var DefaultStyleStrategy;
function reduceStyles(styles3) {
  return styles3.reduce((reduced, current) => {
    if (current instanceof ElementStyles) {
      reduced.push(...reduceStyles(current.styles));
    } else {
      reduced.push(current);
    }
    return reduced;
  }, []);
}
var ElementStyles = class _ElementStyles {
  /**
   * Gets the StyleStrategy associated with these element styles.
   */
  get strategy() {
    if (this._strategy === null) {
      if (!DefaultStyleStrategy) {
        _ElementStyles.setDefaultStrategy(_ElementStyles.supportsAdoptedStyleSheets ? createAdoptedSheetsStrategy() : createStyleElementStrategy());
      }
      this.withStrategy(DefaultStyleStrategy);
    }
    return this._strategy;
  }
  /**
   * Creates an instance of ElementStyles.
   * @param styles - The styles that will be associated with elements.
   */
  constructor(styles3) {
    this.styles = styles3;
    this.targets = /* @__PURE__ */ new WeakSet();
    this._strategy = null;
  }
  /** @internal */
  addStylesTo(target) {
    this.strategy.addStylesTo(target);
    this.targets.add(target);
  }
  /** @internal */
  removeStylesFrom(target) {
    this.strategy.removeStylesFrom(target);
    this.targets.delete(target);
  }
  /** @internal */
  isAttachedTo(target) {
    return this.targets.has(target);
  }
  /**
   * Sets the strategy that handles adding/removing these styles for an element.
   * @param strategy - The strategy to use.
   */
  withStrategy(Strategy) {
    this._strategy = new Strategy(reduceStyles(this.styles));
    return this;
  }
  /**
   * Sets the default strategy type to use when creating style strategies.
   * @param Strategy - The strategy type to construct.
   */
  static setDefaultStrategy(Strategy) {
    DefaultStyleStrategy = Strategy;
  }
  /**
   * Normalizes a set of composable style options.
   * @param styles - The style options to normalize.
   * @returns A singular ElementStyles instance or undefined.
   */
  static normalize(styles3) {
    return styles3 === void 0 ? void 0 : Array.isArray(styles3) ? new _ElementStyles(styles3) : styles3 instanceof _ElementStyles ? styles3 : new _ElementStyles([styles3]);
  }
};
ElementStyles.supportsAdoptedStyleSheets = Array.isArray(document.adoptedStyleSheets) && "replace" in CSSStyleSheet.prototype;
function createAdoptedSheetsStrategy() {
  const cache = /* @__PURE__ */ new Map();
  return class FallbackAdoptedSheetsStrategy {
    constructor(styles3) {
      this.sheets = styles3.map((x) => {
        if (x instanceof CSSStyleSheet) {
          return x;
        }
        let sheet = cache.get(x);
        if (sheet === void 0) {
          sheet = new CSSStyleSheet();
          sheet.replaceSync(x);
          cache.set(x, sheet);
        }
        return sheet;
      });
    }
    addStylesTo(target) {
      const t = target;
      t.adoptedStyleSheets = [...t.adoptedStyleSheets, ...this.sheets];
    }
    removeStylesFrom(target) {
      const t = target;
      t.adoptedStyleSheets = t.adoptedStyleSheets.filter((x) => this.sheets.indexOf(x) === -1);
    }
  };
}
var fallbackStyleId = 0;
function createStyleElementStrategy() {
  return class FallbackStyleElementStrategy {
    constructor(styles3) {
      this.styles = styles3;
      this.styleClass = `fast-${++fallbackStyleId}`;
    }
    addStylesTo(target) {
      const t = target === document ? document.body : target;
      for (let i = 0; i < this.styles.length; i++) {
        const element = document.createElement("style");
        element.innerHTML = this.styles[i];
        element.className = this.styleClass;
        t.append(element);
      }
    }
    removeStylesFrom(target) {
      const t = target === document ? document.body : target;
      const styles3 = t.querySelectorAll(`.${this.styleClass}`);
      for (let i = 0, ii = styles3.length; i < ii; ++i) {
        t.removeChild(styles3[i]);
      }
    }
  };
}

// node_modules/@microsoft/fast-element/dist/esm/components/fast-element-registry.js
var globalRegisteredTypes = {};
var registeredTypesByRegistry = /* @__PURE__ */ new WeakMap();
var typeRegistry = createTypeRegistry();
var definitionInstances = /* @__PURE__ */ new WeakMap();
var instanceRefs = /* @__PURE__ */ new WeakMap();
var fastElementRegistry = Object.freeze({
  register(definition3) {
    if (!typeRegistry.register(definition3)) {
      return false;
    }
    const registeredTypes = getRegisteredTypes(definition3.registry);
    if (!Object.prototype.hasOwnProperty.call(registeredTypes, definition3.name)) {
      Observable.defineProperty(registeredTypes, definition3.name);
    }
    registeredTypes[definition3.name] = definition3.type;
    return true;
  },
  getByType: typeRegistry.getByType,
  getForInstance: typeRegistry.getForInstance,
  whenRegistered
});
function getRegisteredTypes(registry3 = customElements) {
  if (registry3 === customElements) {
    return globalRegisteredTypes;
  }
  let registeredTypes = registeredTypesByRegistry.get(registry3);
  if (!registeredTypes) {
    registeredTypes = {};
    registeredTypesByRegistry.set(registry3, registeredTypes);
  }
  return registeredTypes;
}
function getDefinitionForType(type) {
  return type === void 0 ? void 0 : fastElementRegistry.getByType(type);
}
function whenRegistered(name, registry3 = customElements) {
  const registeredTypes = getRegisteredTypes(registry3);
  if (!Object.prototype.hasOwnProperty.call(registeredTypes, name)) {
    Observable.defineProperty(registeredTypes, name);
  }
  const definition3 = getDefinitionForType(registeredTypes[name]);
  if (definition3 !== void 0) {
    return Promise.resolve(definition3);
  }
  return new Promise((resolve) => {
    const notifier = Observable.getNotifier(registeredTypes);
    const subscriber = {
      handleChange: () => {
        const definition4 = getDefinitionForType(registeredTypes[name]);
        if (definition4 === void 0) {
          return;
        }
        notifier.unsubscribe(subscriber, name);
        resolve(definition4);
      }
    };
    notifier.subscribe(subscriber, name);
  });
}
function trackFASTElementInstance(definition3, instance) {
  if (instanceRefs.has(instance)) {
    return false;
  }
  let instances = definitionInstances.get(definition3);
  const isFirstInstance = instances === void 0;
  if (instances === void 0) {
    instances = /* @__PURE__ */ new Set();
    definitionInstances.set(definition3, instances);
  }
  const ref2 = new WeakRef(instance);
  instances.add(ref2);
  instanceRefs.set(instance, ref2);
  return isFirstInstance;
}
function trackedFASTElementInstances(definition3, callback) {
  const instances = definitionInstances.get(definition3);
  if (instances === void 0) {
    return;
  }
  definitionInstances.delete(definition3);
  for (const ref2 of instances) {
    const instance = ref2.deref();
    if (instance !== void 0) {
      callback(instance);
    }
  }
}

// node_modules/@microsoft/fast-element/dist/esm/components/fast-definitions.js
var defaultShadowOptions = { mode: "open" };
var defaultElementOptions = {};
var fastElementBaseTypes = /* @__PURE__ */ new Set();
var templateResolvers = /* @__PURE__ */ new WeakMap();
var pendingTemplateResolutions = /* @__PURE__ */ new WeakMap();
var templateResolutionErrors = /* @__PURE__ */ new WeakMap();
var extensionRegistries = /* @__PURE__ */ new WeakMap();
var lateAttributeLookups = /* @__PURE__ */ new WeakMap();
function isFASTElementTemplateResolver(value) {
  return isFunction(value);
}
function isPromiseLike(value) {
  return typeof (value === null || value === void 0 ? void 0 : value.then) === "function";
}
function finalizeResolvedTemplate(definition3, template3) {
  pendingTemplateResolutions.delete(definition3);
  if (definition3.template === void 0 && template3 !== void 0) {
    definition3.template = template3;
  }
  if (definition3.template !== void 0) {
    templateResolutionErrors.delete(definition3);
    templateResolvers.delete(definition3);
    return definition3.template;
  }
  return void 0;
}
function applyFASTElementExtensions(definition3, registry3 = definition3.registry, extensions) {
  if (!(extensions === null || extensions === void 0 ? void 0 : extensions.length)) {
    return;
  }
  const typedDefinition = definition3;
  let registries = extensionRegistries.get(typedDefinition);
  if (registries === null || registries === void 0 ? void 0 : registries.has(registry3)) {
    return;
  }
  if (registries === void 0) {
    registries = /* @__PURE__ */ new WeakSet();
    extensionRegistries.set(typedDefinition, registries);
  }
  registries.add(registry3);
  for (const extension of extensions) {
    extension(definition3);
  }
}
function getLateAttributeLookup(definition3) {
  var _a2;
  return (_a2 = lateAttributeLookups.get(definition3)) !== null && _a2 !== void 0 ? _a2 : null;
}
function resolveFASTElementTemplate(definition3) {
  if (definition3.template !== void 0) {
    templateResolutionErrors.delete(definition3);
    return definition3.template;
  }
  const pendingResolution = pendingTemplateResolutions.get(definition3);
  if (pendingResolution) {
    return pendingResolution;
  }
  const templateResolver = templateResolvers.get(definition3);
  if (!templateResolver) {
    return void 0;
  }
  templateResolutionErrors.delete(definition3);
  let template3;
  try {
    template3 = templateResolver(definition3);
  } catch (error) {
    templateResolutionErrors.set(definition3, error);
    throw error;
  }
  if (isPromiseLike(template3)) {
    const resolution = Promise.resolve(template3).then((resolvedTemplate) => finalizeResolvedTemplate(definition3, resolvedTemplate)).catch((error) => {
      pendingTemplateResolutions.delete(definition3);
      templateResolutionErrors.set(definition3, error);
      throw error;
    });
    pendingTemplateResolutions.set(definition3, resolution);
    return resolution;
  }
  return finalizeResolvedTemplate(definition3, template3);
}
function setFASTElementTemplateError(definition3, error) {
  const typedDefinition = definition3;
  if (error === void 0) {
    templateResolutionErrors.delete(typedDefinition);
    return;
  }
  templateResolutionErrors.set(typedDefinition, error);
}
var FASTElementDefinition = class _FASTElementDefinition {
  /**
   * Indicates if this element has been defined in at least one registry.
   */
  get isDefined() {
    return this.platformDefined;
  }
  constructor(type, nameOrConfig = type.definition) {
    var _a2;
    this.platformDefined = false;
    if (isString(nameOrConfig)) {
      nameOrConfig = { name: nameOrConfig };
    }
    this.type = type;
    this.name = nameOrConfig.name;
    this.registry = (_a2 = nameOrConfig.registry) !== null && _a2 !== void 0 ? _a2 : customElements;
    if (isFASTElementTemplateResolver(nameOrConfig.template)) {
      templateResolvers.set(this, nameOrConfig.template);
    } else {
      this.template = nameOrConfig.template;
    }
    const proto = type.prototype;
    const attributes = AttributeDefinition.collect(type, nameOrConfig.attributes);
    const observedAttributes = new Array(attributes.length);
    const propertyLookup = {};
    const attributeLookup = {};
    for (let i = 0, ii = attributes.length; i < ii; ++i) {
      const current = attributes[i];
      observedAttributes[i] = current.attribute;
      propertyLookup[current.name] = current;
      attributeLookup[current.attribute] = current;
      Observable.defineProperty(proto, current);
    }
    Reflect.defineProperty(type, "observedAttributes", {
      value: observedAttributes,
      enumerable: true
    });
    this.attributes = attributes;
    this.propertyLookup = propertyLookup;
    this.attributeLookup = attributeLookup;
    this.shadowOptions = nameOrConfig.shadowOptions === void 0 ? defaultShadowOptions : nameOrConfig.shadowOptions === null ? void 0 : Object.assign(Object.assign({}, defaultShadowOptions), nameOrConfig.shadowOptions);
    this.elementOptions = nameOrConfig.elementOptions === void 0 ? defaultElementOptions : Object.assign(Object.assign({}, defaultElementOptions), nameOrConfig.elementOptions);
    this.styles = ElementStyles.normalize(nameOrConfig.styles);
    this.schema = nameOrConfig.schema;
    fastElementRegistry.register(this);
  }
  /**
   * Defines a custom element based on this definition.
   * @param registry - The element registry to define the element in.
   * @param extensions - An optional array of extension callbacks to invoke
   * with this definition before platform registration.
   * @remarks
   * This operation is idempotent per registry.
   */
  define(registry3 = this.registry, extensions) {
    const type = this.type;
    if (!registry3.get(this.name)) {
      applyFASTElementExtensions(this, registry3, extensions);
      if (this.template === void 0 && templateResolvers.has(this)) {
        void Promise.resolve().then(() => resolveFASTElementTemplate(this)).then((template3) => {
          if (template3 !== void 0 && !registry3.get(this.name)) {
            this.platformDefined = true;
            registry3.define(this.name, type, this.elementOptions);
          }
        }).catch((error) => {
          setFASTElementTemplateError(this, error);
          Observable.notify(this, "template");
        });
        return this;
      }
      this.platformDefined = true;
      registry3.define(this.name, type, this.elementOptions);
    }
    return this;
  }
  /**
   * Creates an instance of FASTElementDefinition.
   * @param type - The type this definition is being created for.
   * @param nameOrDef - The name of the element to define or a config object
   * that describes the element to define.
   */
  static compose(type, nameOrDef) {
    const definition3 = fastElementBaseTypes.has(type) || fastElementRegistry.getByType(type) ? new _FASTElementDefinition(class extends type {
    }, nameOrDef) : new _FASTElementDefinition(type, nameOrDef);
    return Promise.resolve(definition3);
  }
  /**
   * Registers a FASTElement base type.
   * @param type - The type to register as a base type.
   * @internal
   */
  static registerBaseType(type) {
    fastElementBaseTypes.add(type);
  }
};
FASTElementDefinition.getByType = fastElementRegistry.getByType;
FASTElementDefinition.getForInstance = fastElementRegistry.getForInstance;
Observable.defineProperty(FASTElementDefinition.prototype, "template");

// node_modules/@microsoft/fast-element/dist/esm/components/element-controller.js
var defaultEventOptions = {
  bubbles: true,
  composed: true,
  cancelable: true
};
var isConnectedPropertyName = "isConnected";
var shadowRoots = /* @__PURE__ */ new WeakMap();
var lateAttributeObserver = Symbol("fast-late-attribute-observer");
function getShadowRoot(element) {
  var _a2, _b;
  return (_b = (_a2 = element.shadowRoot) !== null && _a2 !== void 0 ? _a2 : shadowRoots.get(element)) !== null && _b !== void 0 ? _b : null;
}
var elementControllerStrategy;
var Stages;
(function(Stages2) {
  Stages2[Stages2["connecting"] = 0] = "connecting";
  Stages2[Stages2["connected"] = 1] = "connected";
  Stages2[Stages2["disconnecting"] = 2] = "disconnecting";
  Stages2[Stages2["disconnected"] = 3] = "disconnected";
})(Stages || (Stages = {}));
var ElementController = class _ElementController {
  /**
   * Indicates whether or not the custom element has been
   * connected to the document.
   */
  get isConnected() {
    Observable.track(this, isConnectedPropertyName);
    return this.stage === Stages.connected;
  }
  /**
   * The context the expression is evaluated against.
   */
  get context() {
    var _a2;
    var _b;
    return (_b = (_a2 = this.view) === null || _a2 === void 0 ? void 0 : _a2.context) !== null && _b !== void 0 ? _b : ExecutionContext.default;
  }
  /**
   * Indicates whether the controller is bound.
   */
  get isBound() {
    var _a2;
    var _b;
    return (_b = (_a2 = this.view) === null || _a2 === void 0 ? void 0 : _a2.isBound) !== null && _b !== void 0 ? _b : false;
  }
  /**
   * Indicates how the source's lifetime relates to the controller's lifetime.
   */
  get sourceLifetime() {
    var _a2;
    return (_a2 = this.view) === null || _a2 === void 0 ? void 0 : _a2.sourceLifetime;
  }
  /**
   * Gets/sets the template used to render the component.
   * @remarks
   * This value can only be accurately read after connect but can be set at any time.
   */
  get template() {
    var _a2;
    if (this._template === null) {
      const definition3 = this.definition;
      if (this.source.resolveTemplate) {
        this._template = this.source.resolveTemplate();
      } else if (definition3.template) {
        this._template = (_a2 = definition3.template) !== null && _a2 !== void 0 ? _a2 : null;
      }
    }
    return this._template;
  }
  set template(value) {
    if (this._template === value) {
      return;
    }
    this._template = value;
    if (!this.needsInitialization) {
      this.renderTemplate(value);
    }
  }
  /**
   * The shadow root options for the component.
   */
  get shadowOptions() {
    return this._shadowRootOptions;
  }
  set shadowOptions(value) {
    if (this._shadowRootOptions === void 0 && value !== void 0) {
      this._shadowRootOptions = value;
      let shadowRoot = this.source.shadowRoot;
      if (shadowRoot) {
        this.hasExistingShadowRoot = true;
      } else {
        shadowRoot = this.source.attachShadow(value);
        if (value.mode === "closed") {
          shadowRoots.set(this.source, shadowRoot);
        }
      }
    }
  }
  /**
   * The main set of styles used for the component, independent
   * of any dynamically added styles.
   */
  get mainStyles() {
    var _a2;
    if (this._mainStyles === null) {
      const definition3 = this.definition;
      if (this.source.resolveStyles) {
        this._mainStyles = this.source.resolveStyles();
      } else if (definition3.styles) {
        this._mainStyles = (_a2 = definition3.styles) !== null && _a2 !== void 0 ? _a2 : null;
      }
    }
    return this._mainStyles;
  }
  set mainStyles(value) {
    if (this._mainStyles === value) {
      return;
    }
    if (this._mainStyles !== null) {
      this.removeStyles(this._mainStyles);
    }
    this._mainStyles = value;
    if (!this.needsInitialization) {
      this.addStyles(value);
    }
  }
  /**
   * Creates a Controller to control the specified element.
   * @param element - The element to be controlled by this controller.
   * @param definition - The element definition metadata that instructs this
   * controller in how to handle rendering and other platform integrations.
   * @internal
   */
  constructor(element, definition3) {
    this.boundObservables = null;
    this.needsInitialization = true;
    this.hasExistingShadowRoot = false;
    this.isPrerendered = new Promise((resolve) => {
      this._resolvePrerendered = resolve;
    });
    this.isHydrated = new Promise((resolve) => {
      this._resolveHydrated = resolve;
    });
    this._template = null;
    this.stage = Stages.disconnected;
    this.guardBehaviorConnection = false;
    this.behaviors = null;
    this.behaviorsConnected = false;
    this._mainStyles = null;
    this.$fastController = this;
    this.view = null;
    this._notifier = new PropertyChangeNotifier(element);
    this.source = element;
    this.definition = definition3;
    this.shadowOptions = definition3.shadowOptions;
    const prototype = Reflect.getPrototypeOf(element);
    const accessors = prototype === null ? [] : Observable.getAccessors(prototype);
    if (accessors.length > 0) {
      const boundObservables = this.boundObservables = /* @__PURE__ */ Object.create(null);
      for (let i = 0, ii = accessors.length; i < ii; ++i) {
        const propertyName = accessors[i].name;
        const value = element[propertyName];
        if (value !== void 0) {
          delete element[propertyName];
          boundObservables[propertyName] = value;
        }
      }
      if (Object.keys(boundObservables).length === 0) {
        this.boundObservables = null;
      }
    }
    this.captureBoundObservables();
  }
  /**
   * The subject that subscribers will receive notifications for.
   */
  get subject() {
    return this._notifier.subject;
  }
  /**
   * Notifies all subscribers of a property change.
   * @param args - The property name that changed.
   */
  notify(args) {
    this._notifier.notify(args);
  }
  /**
   * Subscribes to notification of changes in the element's state.
   * @param subscriber - The object that is subscribing for change notification.
   * @param propertyToWatch - The name of the property to watch for changes.
   */
  subscribe(subscriber, propertyToWatch) {
    this._notifier.subscribe(subscriber, propertyToWatch);
  }
  /**
   * Unsubscribes from notification of changes in the element's state.
   * @param subscriber - The object that is unsubscribing from change notification.
   * @param propertyToUnwatch - The name of the property to unsubscribe from.
   */
  unsubscribe(subscriber, propertyToUnwatch) {
    this._notifier.unsubscribe(subscriber, propertyToUnwatch);
  }
  /**
   * Registers an unbind handler with the controller.
   * @param behavior - An object to call when the controller unbinds.
   */
  onUnbind(behavior) {
    var _a2;
    (_a2 = this.view) === null || _a2 === void 0 ? void 0 : _a2.onUnbind(behavior);
  }
  /**
   * Adds the behavior to the component.
   * @param behavior - The behavior to add.
   */
  addBehavior(behavior) {
    var _a2, _b;
    const targetBehaviors = (_a2 = this.behaviors) !== null && _a2 !== void 0 ? _a2 : this.behaviors = /* @__PURE__ */ new Map();
    const count = (_b = targetBehaviors.get(behavior)) !== null && _b !== void 0 ? _b : 0;
    if (count === 0) {
      targetBehaviors.set(behavior, 1);
      behavior.addedCallback && behavior.addedCallback(this);
      if (behavior.connectedCallback && !this.guardBehaviorConnection && (this.stage === Stages.connected || this.stage === Stages.connecting)) {
        behavior.connectedCallback(this);
      }
    } else {
      targetBehaviors.set(behavior, count + 1);
    }
  }
  /**
   * Removes the behavior from the component.
   * @param behavior - The behavior to remove.
   * @param force - Forces removal even if this behavior was added more than once.
   */
  removeBehavior(behavior, force = false) {
    const targetBehaviors = this.behaviors;
    if (targetBehaviors === null) {
      return;
    }
    const count = targetBehaviors.get(behavior);
    if (count === void 0) {
      return;
    }
    if (count === 1 || force) {
      targetBehaviors.delete(behavior);
      if (behavior.disconnectedCallback && this.stage !== Stages.disconnected) {
        behavior.disconnectedCallback(this);
      }
      behavior.removedCallback && behavior.removedCallback(this);
    } else {
      targetBehaviors.set(behavior, count - 1);
    }
  }
  /**
   * Adds styles to this element. Providing an HTMLStyleElement will attach the element instance to the shadowRoot.
   * @param styles - The styles to add.
   */
  addStyles(styles3) {
    var _a2;
    if (!styles3) {
      return;
    }
    const source = this.source;
    if (styles3 instanceof HTMLElement) {
      const target = (_a2 = getShadowRoot(source)) !== null && _a2 !== void 0 ? _a2 : this.source;
      target.append(styles3);
    } else if (!styles3.isAttachedTo(source)) {
      styles3.addStylesTo(source);
    }
  }
  /**
   * Removes styles from this element. Providing an HTMLStyleElement will detach the element instance from the shadowRoot.
   * @param styles - the styles to remove.
   */
  removeStyles(styles3) {
    var _a2;
    if (!styles3) {
      return;
    }
    const source = this.source;
    if (styles3 instanceof HTMLElement) {
      const target = (_a2 = getShadowRoot(source)) !== null && _a2 !== void 0 ? _a2 : source;
      target.removeChild(styles3);
    } else if (styles3.isAttachedTo(source)) {
      styles3.removeStylesFrom(source);
    }
  }
  /**
   * Runs connected lifecycle behavior on the associated element.
   */
  connect() {
    if (this.stage !== Stages.disconnected) {
      return;
    }
    this.stage = Stages.connecting;
    this.captureBoundObservables();
    this.syncLateAttributes();
    this.observeLateAttributes();
    this.bindObservables();
    this.connectBehaviors();
    if (this.needsInitialization) {
      this.renderTemplate(this.template);
      this.addStyles(this.mainStyles);
      this.needsInitialization = false;
    } else if (this.view !== null) {
      this.view.bind(this.source);
    }
    this.stage = Stages.connected;
    Observable.notify(this, isConnectedPropertyName);
  }
  /**
   * Binds any observables that were set before upgrade.
   */
  bindObservables() {
    if (this.boundObservables !== null) {
      const element = this.source;
      const boundObservables = this.boundObservables;
      const propertyNames = Object.keys(boundObservables);
      for (let i = 0, ii = propertyNames.length; i < ii; ++i) {
        const propertyName = propertyNames[i];
        element[propertyName] = boundObservables[propertyName];
      }
      this.boundObservables = null;
    }
  }
  /**
   * Captures public own-properties that shadow observable accessors on the
   * prototype so they can be rebound through the accessor before rendering.
   */
  captureBoundObservables() {
    const element = this.source;
    const propertyNames = Object.getOwnPropertyNames(element);
    const hasPrototypeAccessor = (propertyName) => {
      let currentTarget = Reflect.getPrototypeOf(element);
      while (currentTarget !== null) {
        const descriptor = Reflect.getOwnPropertyDescriptor(currentTarget, propertyName);
        if ((descriptor === null || descriptor === void 0 ? void 0 : descriptor.get) || (descriptor === null || descriptor === void 0 ? void 0 : descriptor.set)) {
          return true;
        }
        currentTarget = Reflect.getPrototypeOf(currentTarget);
      }
      return false;
    };
    let boundObservables = this.boundObservables;
    for (let i = 0, ii = propertyNames.length; i < ii; ++i) {
      const ownPropertyName = propertyNames[i];
      const propertyName = ownPropertyName;
      if (!hasPrototypeAccessor(propertyName)) {
        continue;
      }
      const value = element[propertyName];
      if (value === void 0) {
        delete element[propertyName];
        continue;
      }
      delete element[propertyName];
      (boundObservables !== null && boundObservables !== void 0 ? boundObservables : boundObservables = this.boundObservables = /* @__PURE__ */ Object.create(null))[propertyName] = value;
    }
  }
  /**
   * Synchronizes late-defined attribute-map attributes from the live DOM to the
   * associated property values before the initial render occurs.
   */
  syncLateAttributes() {
    const lateAttributes = getLateAttributeLookup(this.definition);
    if (lateAttributes === null) {
      return;
    }
    for (const attributeName of Object.keys(lateAttributes)) {
      if (!this.source.hasAttribute(attributeName)) {
        continue;
      }
      this.onAttributeChangedCallback(attributeName, null, this.source.getAttribute(attributeName));
    }
  }
  /**
   * Observes late-defined attribute-map attributes that the platform does not
   * surface through attributeChangedCallback because they were added after
   * customElements.define() completed.
   */
  observeLateAttributes() {
    const lateAttributes = getLateAttributeLookup(this.definition);
    if (lateAttributes === null) {
      return;
    }
    const element = this.source;
    if (element[lateAttributeObserver] !== void 0) {
      return;
    }
    element[lateAttributeObserver] = new MutationObserver((records) => {
      const controller = element.$fastController;
      const lateAttributes2 = getLateAttributeLookup(controller.definition);
      if (lateAttributes2 === null) {
        return;
      }
      for (let i = 0, ii = records.length; i < ii; ++i) {
        const attributeName = records[i].attributeName;
        if (attributeName === null || lateAttributes2[attributeName] === void 0) {
          continue;
        }
        controller.onAttributeChangedCallback(attributeName, null, element.getAttribute(attributeName));
      }
    });
    element[lateAttributeObserver].observe(element, {
      attributes: true,
      attributeFilter: Object.keys(lateAttributes)
    });
  }
  /**
   * Connects any existing behaviors on the associated element.
   */
  connectBehaviors() {
    if (this.behaviorsConnected === false) {
      const behaviors = this.behaviors;
      if (behaviors !== null) {
        this.guardBehaviorConnection = true;
        for (const key of behaviors.keys()) {
          key.connectedCallback && key.connectedCallback(this);
        }
        this.guardBehaviorConnection = false;
      }
      this.behaviorsConnected = true;
    }
  }
  /**
   * Disconnects any behaviors on the associated element.
   */
  disconnectBehaviors() {
    if (this.behaviorsConnected === true) {
      const behaviors = this.behaviors;
      if (behaviors !== null) {
        for (const key of behaviors.keys()) {
          key.disconnectedCallback && key.disconnectedCallback(this);
        }
      }
      this.behaviorsConnected = false;
    }
  }
  /**
   * Runs disconnected lifecycle behavior on the associated element.
   */
  disconnect() {
    if (this.stage !== Stages.connected) {
      return;
    }
    this.stage = Stages.disconnecting;
    Observable.notify(this, isConnectedPropertyName);
    if (this.view !== null) {
      this.view.unbind();
    }
    this.disconnectBehaviors();
    this.stage = Stages.disconnected;
  }
  /**
   * Runs the attribute changed callback for the associated element.
   * @param name - The name of the attribute that changed.
   * @param oldValue - The previous value of the attribute.
   * @param newValue - The new value of the attribute.
   */
  onAttributeChangedCallback(name, oldValue, newValue) {
    const attrDef = this.definition.attributeLookup[name];
    if (attrDef !== void 0) {
      attrDef.onAttributeChangedCallback(this.source, newValue);
    }
  }
  /**
   * Emits a custom HTML event.
   * @param type - The type name of the event.
   * @param detail - The event detail object to send with the event.
   * @param options - The event options. By default bubbles and composed.
   * @remarks
   * Only emits events if connected.
   */
  emit(type, detail, options) {
    if (this.stage === Stages.connected) {
      return this.source.dispatchEvent(new CustomEvent(type, Object.assign(Object.assign({ detail }, defaultEventOptions), options)));
    }
    return false;
  }
  /**
   * Renders the provided template to the element.
   *
   * @param template - The template to render.
   * @remarks
   * If `null` is provided, any existing view will be removed.
   */
  renderTemplate(template3) {
    var _a2;
    const element = this.source;
    const host = (_a2 = getShadowRoot(element)) !== null && _a2 !== void 0 ? _a2 : element;
    if (this.view !== null) {
      this.view.dispose();
      this.view = null;
    } else if (!this.needsInitialization || this.hasExistingShadowRoot) {
      if (!this.hasExistingShadowRoot || !this.needsInitialization) {
        for (let child = host.firstChild; child !== null; child = host.firstChild) {
          host.removeChild(child);
        }
      }
    }
    if (template3) {
      const hasPrerenderedContent = this.hasExistingShadowRoot && this.needsInitialization;
      let didHydrate = false;
      if (hasPrerenderedContent && _ElementController.hydrationHook) {
        didHydrate = _ElementController.hydrationHook(this, template3, element, host);
      }
      if (!didHydrate) {
        this.renderClientSide(template3, element, host);
      }
      this._resolvePrerendered(hasPrerenderedContent);
      this._resolveHydrated(didHydrate);
    } else if (this.needsInitialization) {
      this._resolvePrerendered(false);
      this._resolveHydrated(false);
    }
  }
  /**
   * Standard client-side render: clears any stale content, clones the
   * compiled fragment, binds, and appends to the host.
   */
  renderClientSide(template3, element, host) {
    if (this.hasExistingShadowRoot) {
      for (let child = host.firstChild; child !== null; child = host.firstChild) {
        host.removeChild(child);
      }
      this.hasExistingShadowRoot = false;
    }
    this.view = template3.render(element, host, element);
    this.view.sourceLifetime = SourceLifetime.coupled;
  }
  /**
   * Locates or creates a controller for the specified element.
   * @param element - The element to return the controller for.
   * @param override - Reset the controller even if one has been defined.
   * @remarks
   * The specified element must have a {@link FASTElementDefinition}
   * registered either through the use of the {@link customElement}
   * decorator or a call to `FASTElement.define`.
   */
  static forCustomElement(element, override = false) {
    const controller = element.$fastController;
    if (controller !== void 0 && !override) {
      return controller;
    }
    const definition3 = FASTElementDefinition.getForInstance(element);
    if (definition3 === void 0) {
      throw FAST.error(Message.missingElementDefinition);
    }
    if (definition3.template === void 0 && trackFASTElementInstance(definition3, element)) {
      const notifier = Observable.getNotifier(definition3);
      const subscriber = {
        handleChange: () => {
          notifier.unsubscribe(subscriber, "template");
          trackedFASTElementInstances(definition3, (tracked) => {
            const trackedElement = tracked;
            _ElementController.forCustomElement(trackedElement, true);
            if (trackedElement.isConnected) {
              trackedElement.$fastController.connect();
            }
          });
        }
      };
      notifier.subscribe(subscriber, "template");
    }
    return element.$fastController = new elementControllerStrategy(element, definition3);
  }
  /**
   * Sets the strategy that ElementController.forCustomElement uses to construct
   * ElementController instances for an element.
   * @param strategy - The strategy to use.
   */
  static setStrategy(strategy) {
    elementControllerStrategy = strategy;
  }
  /**
   * Installs the hydration hook. Called by enableHydration().
   * @internal
   */
  static installHydrationHook(hook) {
    _ElementController.hydrationHook = hook;
  }
};
ElementController.hydrationHook = null;
makeSerializationNoop(ElementController);
ElementController.setStrategy(ElementController);
function normalizeStyleTarget(target) {
  var _a2;
  if ("adoptedStyleSheets" in target) {
    return target;
  } else {
    return (_a2 = getShadowRoot(target)) !== null && _a2 !== void 0 ? _a2 : target.getRootNode();
  }
}
var AdoptedStyleSheetsStrategy = class _AdoptedStyleSheetsStrategy {
  constructor(styles3) {
    const styleSheetCache = _AdoptedStyleSheetsStrategy.styleSheetCache;
    this.sheets = styles3.map((x) => {
      if (x instanceof CSSStyleSheet) {
        return x;
      }
      let sheet = styleSheetCache.get(x);
      if (sheet === void 0) {
        sheet = new CSSStyleSheet();
        sheet.replaceSync(x);
        styleSheetCache.set(x, sheet);
      }
      return sheet;
    });
  }
  addStylesTo(target) {
    addAdoptedStyleSheets(normalizeStyleTarget(target), this.sheets);
  }
  removeStylesFrom(target) {
    removeAdoptedStyleSheets(normalizeStyleTarget(target), this.sheets);
  }
};
AdoptedStyleSheetsStrategy.styleSheetCache = /* @__PURE__ */ new Map();
var id = 0;
var nextStyleId = () => `fast-${++id}`;
function usableStyleTarget(target) {
  return target === document ? document.body : target;
}
var StyleElementStrategy = class {
  constructor(styles3) {
    this.styles = styles3;
    this.styleClass = nextStyleId();
  }
  addStylesTo(target) {
    target = usableStyleTarget(normalizeStyleTarget(target));
    const styles3 = this.styles;
    const styleClass = this.styleClass;
    for (let i = 0; i < styles3.length; i++) {
      const element = document.createElement("style");
      element.innerHTML = styles3[i];
      element.className = styleClass;
      target.append(element);
    }
  }
  removeStylesFrom(target) {
    target = usableStyleTarget(normalizeStyleTarget(target));
    const styles3 = target.querySelectorAll(`.${this.styleClass}`);
    for (let i = 0, ii = styles3.length; i < ii; ++i) {
      target.removeChild(styles3[i]);
    }
  }
};
var addAdoptedStyleSheets = (target, sheets) => {
  target.adoptedStyleSheets = [...target.adoptedStyleSheets, ...sheets];
};
var removeAdoptedStyleSheets = (target, sheets) => {
  target.adoptedStyleSheets = target.adoptedStyleSheets.filter((x) => sheets.indexOf(x) === -1);
};
if (ElementStyles.supportsAdoptedStyleSheets) {
  try {
    document.adoptedStyleSheets.push();
    document.adoptedStyleSheets.splice();
    addAdoptedStyleSheets = (target, sheets) => {
      target.adoptedStyleSheets.push(...sheets);
    };
    removeAdoptedStyleSheets = (target, sheets) => {
      for (const sheet of sheets) {
        const index = target.adoptedStyleSheets.indexOf(sheet);
        if (index !== -1) {
          target.adoptedStyleSheets.splice(index, 1);
        }
      }
    };
  } catch (_e) {
  }
  ElementStyles.setDefaultStrategy(AdoptedStyleSheetsStrategy);
} else {
  ElementStyles.setDefaultStrategy(StyleElementStrategy);
}

// node_modules/@microsoft/fast-element/dist/esm/components/fast-element.js
function createFASTElement(BaseType) {
  const type = class extends BaseType {
    constructor() {
      super();
      ElementController.forCustomElement(this);
    }
    $emit(type2, detail, options) {
      return this.$fastController.emit(type2, detail, options);
    }
    connectedCallback() {
      this.$fastController.connect();
    }
    disconnectedCallback() {
      this.$fastController.disconnect();
    }
    attributeChangedCallback(name, oldValue, newValue) {
      this.$fastController.onAttributeChangedCallback(name, oldValue, newValue);
    }
  };
  FASTElementDefinition.registerBaseType(type);
  return type;
}
function isPromiseLike2(value) {
  return typeof (value === null || value === void 0 ? void 0 : value.then) === "function";
}
function define(type, nameOrDef, extensions) {
  if (Array.isArray(nameOrDef)) {
    extensions = nameOrDef;
    nameOrDef = void 0;
  }
  const composePromise = isFunction(type) ? FASTElementDefinition.compose(type, nameOrDef) : FASTElementDefinition.compose(this, type);
  return composePromise.then((def) => {
    applyFASTElementExtensions(def, def.registry, extensions);
    const template3 = resolveFASTElementTemplate(def);
    if (isPromiseLike2(template3)) {
      return template3.then(() => def.define().type);
    }
    return def.define().type;
  });
}
function from(BaseType) {
  return createFASTElement(BaseType);
}
var FASTElement = Object.assign(createFASTElement(HTMLElement), {
  /**
   * Creates a new FASTElement base class inherited from the
   * provided base type.
   * @param BaseType - The base element type to inherit from.
   */
  from,
  /**
   * Defines a platform custom element based on the provided type and definition.
   * @param type - The custom element type to define.
   * @param nameOrDef - The name of the element to define or a definition object
   * that describes the element to define.
   */
  define
});

// node_modules/@microsoft/fast-element/dist/esm/styles/css-directive.js
var registry = createTypeRegistry();
var CSSDirective = Object.freeze({
  /**
   * Gets the directive definition associated with the instance.
   * @param instance - The directive instance to retrieve the definition for.
   */
  getForInstance: registry.getForInstance,
  /**
   * Gets the directive definition associated with the specified type.
   * @param type - The directive type to retrieve the definition for.
   */
  getByType: registry.getByType,
  /**
   * Defines a CSSDirective.
   * @param type - The type to define as a directive.
   */
  define(type) {
    registry.register({ type });
    return type;
  }
});

// node_modules/@microsoft/fast-element/dist/esm/styles/css.js
function collectStyles(strings, values) {
  const styles3 = [];
  let cssString = "";
  for (let i = 0, ii = strings.length - 1; i < ii; ++i) {
    cssString += strings[i];
    let value = values[i];
    if (CSSDirective.getForInstance(value) !== void 0) {
      value = value.createCSS();
    }
    if (value instanceof ElementStyles || value instanceof CSSStyleSheet) {
      if (cssString.trim() !== "") {
        styles3.push(cssString);
        cssString = "";
      }
      styles3.push(value);
    } else {
      cssString += value;
    }
  }
  cssString += strings[strings.length - 1];
  if (cssString.trim() !== "") {
    styles3.push(cssString);
  }
  return styles3;
}
var css = ((strings, ...values) => {
  return new ElementStyles(collectStyles(strings, values));
});
var CSSPartial = class {
  constructor(styles3) {
    this.value = styles3.length === 0 ? "" : styles3.length === 1 ? styles3[0] : new ElementStyles(styles3);
  }
  createCSS() {
    return this.value;
  }
};
CSSDirective.define(CSSPartial);
css.partial = (strings, ...values) => {
  return new CSSPartial(collectStyles(strings, values));
};

// node_modules/@microsoft/fast-element/dist/esm/templating/markup.js
var marker = `fast-${Math.random().toString(36).substring(2, 8)}`;
var interpolationStart = `${marker}{`;
var interpolationEnd = `}${marker}`;
var interpolationEndLength = interpolationEnd.length;
var id2 = 0;
var nextId = () => `${marker}-${++id2}`;
var Markup = Object.freeze({
  /**
   * Creates a placeholder string suitable for marking out a location *within*
   * an attribute value or HTML content.
   * @param index - The directive index to create the placeholder for.
   * @remarks
   * Used internally by binding directives.
   */
  interpolation: (id3) => `${interpolationStart}${id3}${interpolationEnd}`,
  /**
   * Creates a placeholder that manifests itself as an attribute on an
   * element.
   * @param attributeName - The name of the custom attribute.
   * @param index - The directive index to create the placeholder for.
   * @remarks
   * Used internally by attribute directives such as `ref`, `slotted`, and `children`.
   */
  attribute: (id3) => `${nextId()}="${interpolationStart}${id3}${interpolationEnd}"`,
  /**
   * Creates a placeholder that manifests itself as a marker within the DOM structure.
   * @param index - The directive index to create the placeholder for.
   * @remarks
   * Used internally by structural directives such as `repeat`.
   */
  comment: (id3) => `<!--${interpolationStart}${id3}${interpolationEnd}-->`
});
var Parser = Object.freeze({
  /**
   * Parses text content or HTML attribute content, separating out the static strings
   * from the directives.
   * @param value - The content or attribute string to parse.
   * @param factories - A list of directives to search for in the string.
   * @returns A heterogeneous array of static strings interspersed with
   * directives or null if no directives are found in the string.
   */
  parse(value, factories) {
    const parts = value.split(interpolationStart);
    if (parts.length === 1) {
      return null;
    }
    const result = [];
    for (let i = 0, ii = parts.length; i < ii; ++i) {
      const current = parts[i];
      const index = current.indexOf(interpolationEnd);
      let literal;
      if (index === -1) {
        literal = current;
      } else {
        const factoryId = current.substring(0, index);
        result.push(factories[factoryId]);
        literal = current.substring(index + interpolationEndLength);
      }
      if (literal !== "") {
        result.push(literal);
      }
    }
    return result;
  }
});

// node_modules/@microsoft/fast-element/dist/esm/templating/html-directive.js
var registry2 = createTypeRegistry();
var HTMLDirective = Object.freeze({
  /**
   * Gets the directive definition associated with the instance.
   * @param instance - The directive instance to retrieve the definition for.
   */
  getForInstance: registry2.getForInstance,
  /**
   * Gets the directive definition associated with the specified type.
   * @param type - The directive type to retrieve the definition for.
   */
  getByType: registry2.getByType,
  /**
   * Defines an HTMLDirective based on the options.
   * @param type - The type to define as a directive.
   * @param options - Options that specify the directive's application.
   */
  define(type, options) {
    options = options || {};
    options.type = type;
    registry2.register(options);
    return type;
  },
  /**
   * Determines the DOM aspect type for a directive based on attribute name prefix.
   * The prefix convention maps to aspect types as follows:
   *   - No prefix (e.g. "class")  → DOMAspect.attribute
   *   - ":" prefix (e.g. ":value") → DOMAspect.property (":classList" → DOMAspect.tokenList)
   *   - "?" prefix (e.g. "?disabled") → DOMAspect.booleanAttribute
   *   - `@` prefix (e.g. `@click`) → DOMAspect.event
   *   - Falsy or absent value → DOMAspect.content (see remarks)
   * @param directive - The directive to assign the aspect to.
   * @param value - The value to base the aspect determination on.
   * @remarks
   * If a falsy value is provided, then the content aspect will be assigned.
   */
  assignAspect(directive, value) {
    if (!value) {
      directive.aspectType = DOMAspect.content;
      return;
    }
    directive.sourceAspect = value;
    switch (value[0]) {
      case ":":
        directive.targetAspect = value.substring(1);
        directive.aspectType = directive.targetAspect === "classList" ? DOMAspect.tokenList : DOMAspect.property;
        break;
      case "?":
        directive.targetAspect = value.substring(1);
        directive.aspectType = DOMAspect.booleanAttribute;
        break;
      case "@":
        directive.targetAspect = value.substring(1);
        directive.aspectType = DOMAspect.event;
        break;
      default:
        directive.targetAspect = value;
        directive.aspectType = DOMAspect.attribute;
        break;
    }
  }
});
var StatelessAttachedAttributeDirective = class {
  /**
   * Creates an instance of RefDirective.
   * @param options - The options to use in configuring the directive.
   */
  constructor(options) {
    this.options = options;
  }
  /**
   * Creates a placeholder string based on the directive's index within the template.
   * @param index - The index of the directive within the template.
   * @remarks
   * Creates a custom attribute placeholder.
   */
  createHTML(add) {
    return Markup.attribute(add(this));
  }
  /**
   * Creates a behavior.
   * @param targets - The targets available for behaviors to be attached to.
   */
  createBehavior() {
    return this;
  }
};
makeSerializationNoop(StatelessAttachedAttributeDirective);

// node_modules/@microsoft/fast-element/dist/esm/templating/node-observation.js
var NodeObservationDirective = class extends StatelessAttachedAttributeDirective {
  /**
   * The unique id of the factory.
   */
  get id() {
    return this._id;
  }
  set id(value) {
    this._id = value;
    this._controllerProperty = `${value}-c`;
  }
  /**
   * Bind this behavior to the source.
   * @param source - The source to bind to.
   * @param context - The execution context that the binding is operating within.
   * @param targets - The targets that behaviors in a view can attach to.
   */
  bind(controller) {
    const target = controller.targets[this.targetNodeId];
    target[this._controllerProperty] = controller;
    this.updateTarget(controller.source, this.computeNodes(target));
    this.observe(target);
    controller.onUnbind(this);
  }
  /**
   * Unbinds this behavior from the source.
   * @param source - The source to unbind from.
   * @param context - The execution context that the binding is operating within.
   * @param targets - The targets that behaviors in a view can attach to.
   */
  unbind(controller) {
    const target = controller.targets[this.targetNodeId];
    this.updateTarget(controller.source, emptyArray);
    this.disconnect(target);
    target[this._controllerProperty] = null;
  }
  /**
   * Gets the data source for the target.
   * @param target - The target to get the source for.
   * @returns The source.
   */
  getSource(target) {
    return target[this._controllerProperty].source;
  }
  /**
   * Updates the source property with the computed nodes.
   * @param source - The source object to assign the nodes property to.
   * @param value - The nodes to assign to the source object property.
   */
  updateTarget(source, value) {
    source[this.options.property] = value;
  }
  /**
   * Computes the set of nodes that should be assigned to the source property.
   * @param target - The target to compute the nodes for.
   * @returns The computed nodes.
   * @remarks
   * Applies filters if provided.
   */
  computeNodes(target) {
    let nodes = this.getNodes(target);
    if ("filter" in this.options) {
      nodes = nodes.filter(this.options.filter);
    }
    return nodes;
  }
};

// node_modules/@microsoft/fast-element/dist/esm/components/hydration.js
var legacyBindingStartMarker = /fe-b\$\$start\$\$(\d+)\$\$(.+)\$\$fe-b/;
var legacyBindingEndMarker = /fe-b\$\$end\$\$(\d+)\$\$(.+)\$\$fe-b/;
var legacyRepeatViewStartMarker = /fe-repeat\$\$start\$\$(\d+)\$\$fe-repeat/;
var legacyRepeatViewEndMarker = /fe-repeat\$\$end\$\$(\d+)\$\$fe-repeat/;
var legacyElementBoundaryStartMarker = /^(?:.{0,1000})fe-eb\$\$start\$\$(.+?)\$\$fe-eb/;
var legacyElementBoundaryEndMarker = /fe-eb\$\$end\$\$(.{0,1000})\$\$fe-eb(?:.{0,1000})$/;
function isComment(node) {
  return node && node.nodeType === Node.COMMENT_NODE;
}
var HydrationMarkup = Object.freeze({
  // Single attribute marker format (count only)
  attributeMarkerName: "data-fe",
  legacyAttributeMarkerName: "data-fe-b",
  legacyCompactAttributeMarkerName: "data-fe-c",
  // Content binding markers (no arguments)
  contentBindingStartMarker() {
    return "fe:b";
  },
  contentBindingEndMarker() {
    return "fe:/b";
  },
  // Repeat item markers (no arguments)
  repeatStartMarker() {
    return "fe:r";
  },
  repeatEndMarker() {
    return "fe:/r";
  },
  // Element boundary markers (no arguments)
  elementBoundaryStartMarker() {
    return "fe:e";
  },
  elementBoundaryEndMarker() {
    return "fe:/e";
  },
  // Detection — simple string equality
  isContentBindingStartMarker(data) {
    return data === "fe:b" || legacyBindingStartMarker.test(data);
  },
  isContentBindingEndMarker(data) {
    return data === "fe:/b" || legacyBindingEndMarker.test(data);
  },
  isRepeatViewStartMarker(data) {
    return data === "fe:r" || legacyRepeatViewStartMarker.test(data);
  },
  isRepeatViewEndMarker(data) {
    return data === "fe:/r" || legacyRepeatViewEndMarker.test(data);
  },
  isElementBoundaryStartMarker(node) {
    return isComment(node) && (node.data === "fe:e" || legacyElementBoundaryStartMarker.test(node.data));
  },
  isElementBoundaryEndMarker(node) {
    return isComment(node) && (node.data === "fe:/e" || legacyElementBoundaryEndMarker.test(node.data));
  },
  /**
   * Returns the count of attribute bindings on the element, or null
   * if no attribute binding marker is present.
   *
   * Parses the `data-fe="N"` attribute format where N is the count
   * of attribute binding factories targeting this element.
   */
  parseAttributeBindingCount(node) {
    const attr2 = node.getAttribute(this.attributeMarkerName);
    if (attr2 === null) {
      return null;
    }
    const trimmed = attr2.trim();
    if (!/^\d+$/.test(trimmed)) {
      throw FAST.error(Message.invalidHydrationAttributeMarker, {
        value: attr2
      });
    }
    const count = parseInt(trimmed, 10);
    if (count < 1) {
      throw FAST.error(Message.invalidHydrationAttributeMarker, {
        value: attr2
      });
    }
    return count;
  },
  parseLegacyAttributeBindingIndices(node) {
    const indices = [];
    const attr2 = node.getAttribute(this.legacyAttributeMarkerName);
    if (attr2 !== null) {
      for (const value of attr2.trim().split(/\s+/)) {
        if (value === "") {
          continue;
        }
        const index = Number(value);
        if (!Number.isInteger(index) || index < 0) {
          throw FAST.error(Message.invalidHydrationAttributeMarker, {
            value: attr2
          });
        }
        indices.push(index);
      }
    }
    const enumeratedPrefix = `${this.legacyAttributeMarkerName}-`;
    const compactPrefix = `${this.legacyCompactAttributeMarkerName}-`;
    for (const name of node.getAttributeNames()) {
      if (name.startsWith(enumeratedPrefix)) {
        const index = Number(name.slice(enumeratedPrefix.length));
        if (!Number.isInteger(index) || index < 0) {
          throw FAST.error(Message.invalidHydrationAttributeMarker, {
            value: name
          });
        }
        indices.push(index);
      } else if (name.startsWith(compactPrefix)) {
        const [start, count] = name.slice(compactPrefix.length).split("-").map((value) => Number(value));
        if (!Number.isInteger(start) || !Number.isInteger(count) || start < 0 || count < 1) {
          throw FAST.error(Message.invalidHydrationAttributeMarker, {
            value: name
          });
        }
        for (let i = 0; i < count; i++) {
          indices.push(start + i);
        }
      }
    }
    return indices.length === 0 ? null : indices;
  },
  removeLegacyAttributeBindingMarkers(node) {
    node.removeAttribute(this.legacyAttributeMarkerName);
    for (const name of node.getAttributeNames()) {
      if (name.startsWith(`${this.legacyAttributeMarkerName}-`) || name.startsWith(`${this.legacyCompactAttributeMarkerName}-`)) {
        node.removeAttribute(name);
      }
    }
  },
  parseLegacyContentBindingStartIndex(data) {
    return parseLegacyIntMarker(legacyBindingStartMarker, data);
  }
});
function parseLegacyIntMarker(pattern, data) {
  const match = pattern.exec(data);
  return match === null ? null : Number(match[1]);
}
var Hydratable = Symbol.for("fe-hydration");
function isHydratable(value) {
  return value[Hydratable] === Hydratable;
}

// node_modules/@microsoft/fast-element/dist/esm/hydration/messages.js
var unknownHostName = "unknown";
function formatDefaultMismatchMessage(hostName, detail) {
  const suffix = detail ? `: ${detail}` : "";
  return `Hydration mismatch in <${hostName}>${suffix}. Install hydrationDebugger() from "@microsoft/fast-element/hydration.js" and pass it as enableHydration({ debugger: hydrationDebugger() }) for an "Expected / Received" report including the SSR HTML snippet.`;
}
var expectedContentAfterStartMarker = "content following `<!--fe:b-->` content binding marker";
var expectedContentEndMarker = "matching `<!--fe:/b-->` content binding close marker";
var expectedElementBoundaryEndMarker = "matching `<!--fe:/e-->` element boundary close marker";
function formatNoMoreAttributeBindings(factoryCount) {
  return `no more attribute bindings (template defines ${factoryCount})`;
}
function formatNoMoreContentBindings(factoryCount) {
  return `no more content bindings (template defines ${factoryCount})`;
}

// node_modules/@microsoft/fast-element/dist/esm/hydration/diagnostics.js
function formatMinimalMessage(hostName, detail) {
  const host = (hostName !== null && hostName !== void 0 ? hostName : unknownHostName).toLowerCase();
  return formatDefaultMismatchMessage(host, detail);
}
var defaultDiagnostic = {
  formatBindingMismatch(_factory, _firstChild, _lastChild, hostName) {
    return {
      message: formatMinimalMessage(hostName, void 0)
    };
  },
  formatStructuralError(_node, hostName, expectedDescription) {
    return {
      message: formatMinimalMessage(hostName, expectedDescription)
    };
  }
};
var activeDiagnostic = defaultDiagnostic;
function getHydrationDiagnostic() {
  return activeDiagnostic;
}
function getHostName(node) {
  var _a2;
  if (!node) {
    return void 0;
  }
  const root = node.getRootNode();
  return (_a2 = root.host) === null || _a2 === void 0 ? void 0 : _a2.nodeName;
}

// node_modules/@microsoft/fast-element/dist/esm/hydration/target-builder.js
var HydrationTargetElementError = class extends Error {
  constructor(message, factories, node, expected, received) {
    super(message);
    this.factories = factories;
    this.node = node;
    this.expected = expected;
    this.received = received;
  }
};
function isComment2(node) {
  return node.nodeType === 8;
}
function isText(node) {
  return node.nodeType === 3;
}
function getNodeLength(node) {
  return isComment2(node) || isText(node) ? node.data.length : node.childNodes.length;
}
var hydrationNodeMask = 133;
function createRangeForNodes(first, last) {
  const range = document.createRange();
  range.setStart(first, 0);
  range.setEnd(last, getNodeLength(last));
  return range;
}
function createHydrationTraversal(firstNode, lastNode) {
  const range = createRangeForNodes(firstNode, lastNode);
  let walker = range.startContainer === firstNode ? document.createTreeWalker(range.commonAncestorContainer, hydrationNodeMask) : null;
  let hasReachedLastNode = lastNode.contains(firstNode);
  let mutationMode = false;
  if (walker !== null) {
    walker.currentNode = firstNode;
  }
  function move(sibling) {
    if (walker === null || !mutationMode && sibling && walker.currentNode.contains(lastNode)) {
      return null;
    }
    const current = walker.currentNode;
    const candidate = sibling ? walker.nextSibling() : walker.nextNode();
    if (mutationMode || !hasReachedLastNode || lastNode.contains(candidate)) {
      if (candidate === lastNode) {
        hasReachedLastNode = true;
      }
      return candidate;
    }
    walker.currentNode = current;
    return null;
  }
  return [
    move,
    () => {
      if (walker !== null && !mutationMode && (range.startContainer !== firstNode || range.endContainer !== lastNode || range.endOffset !== getNodeLength(lastNode) || !range.intersectsNode(walker.currentNode))) {
        const current = walker.currentNode;
        walker = document.createTreeWalker(walker.root, walker.whatToShow, {
          acceptNode: (node) => range.comparePoint(node, 0) === 0 ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT
        });
        walker.currentNode = current;
        mutationMode = true;
      }
    }
  ];
}
function throwHydrationError(node, factories, expected) {
  const result = getHydrationDiagnostic().formatStructuralError(node, getHostName(node), expected);
  throw new HydrationTargetElementError(result.message, factories, node, result.expected, result.received);
}
function buildViewBindingTargets(firstNode, lastNode, factories) {
  const [move, update] = createHydrationTraversal(firstNode, lastNode);
  const targets = {};
  const boundaries = {};
  const hydrationIndexOffset = getHydrationIndexOffset(factories);
  let factoryPointer = hydrationIndexOffset;
  let node = firstNode;
  while (node !== null) {
    switch (node.nodeType) {
      case 1: {
        const element = node;
        const legacyIndices = HydrationMarkup.parseLegacyAttributeBindingIndices(element);
        if (legacyIndices !== null) {
          for (const index of legacyIndices) {
            const factoryIndex = index + hydrationIndexOffset;
            const factory = factories[factoryIndex];
            if (!factory) {
              throwHydrationError(node, factories, formatNoMoreAttributeBindings(factories.length));
            }
            targetFactory(factory, node, targets);
            factoryPointer = Math.max(factoryPointer, factoryIndex + 1);
          }
          HydrationMarkup.removeLegacyAttributeBindingMarkers(element);
          update();
          break;
        }
        const count = HydrationMarkup.parseAttributeBindingCount(element);
        if (count !== null) {
          for (let i = 0; i < count; i++) {
            const factory = factories[factoryPointer++];
            if (!factory) {
              throwHydrationError(node, factories, formatNoMoreAttributeBindings(factories.length));
            }
            targetFactory(factory, node, targets);
          }
          element.removeAttribute(HydrationMarkup.attributeMarkerName);
          update();
        }
        break;
      }
      case 8: {
        const data = node.data;
        if (HydrationMarkup.isElementBoundaryStartMarker(node)) {
          node.data = "";
          skipToElementBoundaryEnd(move, factories, node);
        } else if (HydrationMarkup.isContentBindingStartMarker(data)) {
          const legacyIndex = HydrationMarkup.parseLegacyContentBindingStartIndex(data);
          const factoryIndex = legacyIndex === null ? factoryPointer++ : legacyIndex + hydrationIndexOffset;
          const factory = factories[factoryIndex];
          factoryPointer = Math.max(factoryPointer, factoryIndex + 1);
          if (!factory) {
            throwHydrationError(node, factories, formatNoMoreContentBindings(factories.length));
          }
          targetContentBinding(node, move, factory, factories, targets, boundaries);
        }
        break;
      }
    }
    node = move();
  }
  return { targets, boundaries };
}
function targetContentBinding(node, move, factory, factories, targets, boundaries) {
  let current = move(true);
  node.data = "";
  if (current === null) {
    throwHydrationError(node, factories, expectedContentAfterStartMarker);
  }
  const first = current;
  let depth = 0;
  while (current !== null) {
    if (isComment2(current)) {
      if (HydrationMarkup.isContentBindingStartMarker(current.data)) {
        depth++;
      } else if (HydrationMarkup.isContentBindingEndMarker(current.data)) {
        if (depth === 0)
          break;
        depth--;
      }
    }
    current = move(true);
  }
  if (current === null) {
    throwHydrationError(node, factories, expectedContentEndMarker);
  }
  current.data = "";
  if (isText(first) && first.nextSibling === current) {
    targetFactory(factory, first, targets);
  } else {
    if (current !== first && current.previousSibling !== null) {
      boundaries[factory.targetNodeId] = {
        first,
        last: current.previousSibling
      };
    }
    targetFactory(factory, current.parentNode.insertBefore(document.createTextNode(""), current), targets);
  }
}
function skipToElementBoundaryEnd(move, factories, startNode) {
  let depth = 0;
  let current = move(true);
  while (current !== null) {
    if (isComment2(current)) {
      if (HydrationMarkup.isElementBoundaryStartMarker(current)) {
        current.data = "";
        depth++;
      } else if (HydrationMarkup.isElementBoundaryEndMarker(current)) {
        current.data = "";
        if (depth-- === 0) {
          return;
        }
      }
    }
    current = move(true);
  }
  throwHydrationError(startNode, factories, expectedElementBoundaryEndMarker);
}
function getHydrationIndexOffset(factories) {
  let offset = 0;
  while (offset < factories.length && factories[offset].targetNodeId === "h") {
    offset++;
  }
  return offset;
}
function targetFactory(factory, node, targets) {
  const id3 = factory.targetNodeId;
  if (id3 === void 0) {
    throw new Error("Factory could not be target to the node");
  }
  targets[id3] = node;
}

// node_modules/@microsoft/fast-element/dist/esm/templating/view.js
function removeNodeSequence(firstNode, lastNode) {
  const parent = firstNode.parentNode;
  let current = firstNode;
  let next2;
  while (current !== lastNode) {
    next2 = current.nextSibling;
    if (!next2) {
      throw new Error(`Unmatched first/last child inside "${lastNode.getRootNode().host.nodeName}".`);
    }
    parent.removeChild(current);
    current = next2;
  }
  parent.removeChild(lastNode);
}
var DefaultExecutionContext = class {
  constructor() {
    this.index = 0;
    this.length = 0;
  }
  /**
   * The current event within an event handler.
   */
  get event() {
    return ExecutionContext.getEvent();
  }
  /**
   * Indicates whether the current item within a repeat context
   * has an even index.
   */
  get isEven() {
    return this.index % 2 === 0;
  }
  /**
   * Indicates whether the current item within a repeat context
   * has an odd index.
   */
  get isOdd() {
    return this.index % 2 !== 0;
  }
  /**
   * Indicates whether the current item within a repeat context
   * is the first item in the collection.
   */
  get isFirst() {
    return this.index === 0;
  }
  /**
   * Indicates whether the current item within a repeat context
   * is somewhere in the middle of the collection.
   */
  get isInMiddle() {
    return !this.isFirst && !this.isLast;
  }
  /**
   * Indicates whether the current item within a repeat context
   * is the last item in the collection.
   */
  get isLast() {
    return this.index === this.length - 1;
  }
  /**
   * Returns the typed event detail of a custom event.
   */
  eventDetail() {
    return this.event.detail;
  }
  /**
   * Returns the typed event target of the event.
   */
  eventTarget() {
    return this.event.target;
  }
};
var HTMLView = class extends DefaultExecutionContext {
  /**
   * Constructs an instance of HTMLView.
   * @param fragment - The html fragment that contains the nodes for this view.
   * @param behaviors - The behaviors to be applied to this view.
   */
  constructor(fragment, factories, targets) {
    super();
    this.fragment = fragment;
    this.factories = factories;
    this.targets = targets;
    this.behaviors = null;
    this.unbindables = [];
    this.source = null;
    this.isBound = false;
    this.sourceLifetime = SourceLifetime.unknown;
    this._skipAttrUpdates = false;
    this.isPrerendered = Promise.resolve(false);
    this.isHydrated = Promise.resolve(false);
    this.context = this;
    this.firstChild = fragment.firstChild;
    this.lastChild = fragment.lastChild;
  }
  /**
   * Appends the view's DOM nodes to the referenced node.
   * @param node - The parent node to append the view's DOM nodes to.
   */
  appendTo(node) {
    node.appendChild(this.fragment);
  }
  /**
   * Inserts the view's DOM nodes before the referenced node.
   * @param node - The node to insert the view's DOM before.
   */
  insertBefore(node) {
    if (this.fragment.hasChildNodes()) {
      node.parentNode.insertBefore(this.fragment, node);
    } else {
      const end = this.lastChild;
      if (node.previousSibling === end)
        return;
      const parentNode = node.parentNode;
      let current = this.firstChild;
      let next2;
      while (current !== end) {
        next2 = current.nextSibling;
        parentNode.insertBefore(current, node);
        current = next2;
      }
      parentNode.insertBefore(end, node);
    }
  }
  /**
   * Removes the view's DOM nodes.
   * The nodes are not disposed and the view can later be re-inserted.
   */
  remove() {
    const fragment = this.fragment;
    const end = this.lastChild;
    let current = this.firstChild;
    let next2;
    while (current !== end) {
      next2 = current.nextSibling;
      fragment.appendChild(current);
      current = next2;
    }
    fragment.appendChild(end);
  }
  /**
   * Removes the view and unbinds its behaviors, disposing of DOM nodes afterward.
   * Once a view has been disposed, it cannot be inserted or bound again.
   */
  dispose() {
    removeNodeSequence(this.firstChild, this.lastChild);
    this.unbind();
  }
  onUnbind(behavior) {
    this.unbindables.push(behavior);
  }
  /**
   * Binds a view's behaviors to its binding source.
   *
   * On the first call, this iterates through all compiled factories, calling
   * createBehavior() on each to produce a ViewBehavior instance (e.g., an
   * HTMLBindingDirective), and then immediately binds it. This is where event
   * listeners are registered, expression observers are created, and initial
   * DOM values are set.
   *
   * On subsequent calls with a new source, existing behaviors are re-bound
   * to the new data source, which re-evaluates all binding expressions and
   * updates the DOM accordingly.
   *
   * @param source - The binding source for the view's binding behaviors.
   * @param context - The execution context to run the behaviors within.
   */
  bind(source, context = this) {
    if (this.source === source && this.context === context) {
      return;
    }
    let behaviors = this.behaviors;
    if (behaviors === null) {
      this.source = source;
      this.context = context;
      this.behaviors = behaviors = new Array(this.factories.length);
      const factories = this.factories;
      for (let i = 0, ii = factories.length; i < ii; ++i) {
        const behavior = factories[i].createBehavior();
        behavior.bind(this);
        behaviors[i] = behavior;
      }
    } else {
      if (this.source !== null) {
        this.evaluateUnbindables();
      }
      this.isBound = false;
      this.source = source;
      this.context = context;
      for (let i = 0, ii = behaviors.length; i < ii; ++i) {
        behaviors[i].bind(this);
      }
    }
    this.isBound = true;
  }
  /**
   * Unbinds a view's behaviors from its binding source.
   */
  unbind() {
    if (!this.isBound || this.source === null) {
      return;
    }
    this.evaluateUnbindables();
    this.source = null;
    this.context = this;
    this.isBound = false;
  }
  evaluateUnbindables() {
    const unbindables = this.unbindables;
    for (let i = 0, ii = unbindables.length; i < ii; ++i) {
      unbindables[i].unbind(this);
    }
    unbindables.length = 0;
  }
  /**
   * Efficiently disposes of a contiguous range of synthetic view instances.
   * @param views - A contiguous range of views to be disposed.
   */
  static disposeContiguousBatch(views) {
    if (views.length === 0) {
      return;
    }
    removeNodeSequence(views[0].firstChild, views[views.length - 1].lastChild);
    for (let i = 0, ii = views.length; i < ii; ++i) {
      views[i].unbind();
    }
  }
};
makeSerializationNoop(HTMLView);
Observable.defineProperty(HTMLView.prototype, "index");
Observable.defineProperty(HTMLView.prototype, "length");

// node_modules/@microsoft/fast-element/dist/esm/templating/hydration-view.js
var _a;
var HydrationStage = {
  unhydrated: "unhydrated",
  hydrating: "hydrating",
  hydrated: "hydrated"
};
var HydrationBindingError = class extends Error {
  constructor(message, factory, fragment, templateString, expected, received) {
    super(message);
    this.factory = factory;
    this.fragment = fragment;
    this.templateString = templateString;
    this.expected = expected;
    this.received = received;
  }
};
var HydrationView = class extends DefaultExecutionContext {
  get hydrationStage() {
    return this._hydrationStage;
  }
  get targets() {
    return this._targets;
  }
  get bindingViewBoundaries() {
    return this._bindingViewBoundaries;
  }
  constructor(firstChild, lastChild, sourceTemplate, hostBindingTarget) {
    super();
    this.firstChild = firstChild;
    this.lastChild = lastChild;
    this.sourceTemplate = sourceTemplate;
    this.hostBindingTarget = hostBindingTarget;
    this[_a] = Hydratable;
    this.context = this;
    this.source = null;
    this.isBound = false;
    this.sourceLifetime = SourceLifetime.unknown;
    this.unbindables = [];
    this.fragment = null;
    this.behaviors = null;
    this._hydrationStage = HydrationStage.unhydrated;
    this._bindingViewBoundaries = {};
    this._targets = {};
    this.factories = sourceTemplate.compile().factories;
  }
  /**
   * no-op. Hydrated views are don't need to be moved from a documentFragment
   * to the target node.
   */
  insertBefore(node) {
    if (this.fragment === null) {
      return;
    }
    if (this.fragment.hasChildNodes()) {
      node.parentNode.insertBefore(this.fragment, node);
    } else {
      const end = this.lastChild;
      if (node.previousSibling === end)
        return;
      const parentNode = node.parentNode;
      let current = this.firstChild;
      let next2;
      while (current !== end) {
        next2 = current.nextSibling;
        parentNode.insertBefore(current, node);
        current = next2;
      }
      parentNode.insertBefore(end, node);
    }
  }
  /**
   * Appends the view to a node. In cases where this is called before the
   * view has been removed, the method will no-op.
   * @param node - the node to append the view to.
   */
  appendTo(node) {
    if (this.fragment !== null) {
      node.appendChild(this.fragment);
    }
  }
  remove() {
    const fragment = this.fragment || (this.fragment = document.createDocumentFragment());
    const end = this.lastChild;
    let current = this.firstChild;
    let next2;
    while (current !== end) {
      next2 = current.nextSibling;
      if (!next2) {
        throw new Error(`Unmatched first/last child inside "${end.getRootNode().host.nodeName}".`);
      }
      fragment.appendChild(current);
      current = next2;
    }
    fragment.appendChild(end);
  }
  bind(source, context = this) {
    if (this.source === source && this.context === context) {
      return;
    }
    if (this.hydrationStage !== HydrationStage.hydrated) {
      this._hydrationStage = HydrationStage.hydrating;
    }
    let behaviors = this.behaviors;
    if (behaviors === null) {
      this.source = source;
      this.context = context;
      try {
        const { targets, boundaries } = buildViewBindingTargets(this.firstChild, this.lastChild, this.factories);
        this._targets = targets;
        this._bindingViewBoundaries = boundaries;
      } catch (error) {
        if (error instanceof HydrationTargetElementError) {
          let templateString = this.sourceTemplate.html;
          if (typeof templateString !== "string") {
            templateString = templateString.innerHTML;
          }
          error.templateString = templateString;
        }
        throw error;
      }
      this.behaviors = behaviors = new Array(this.factories.length);
      const factories = this.factories;
      for (let i = 0, ii = factories.length; i < ii; ++i) {
        const factory = factories[i];
        if (factory.targetNodeId === "h" && this.hostBindingTarget) {
          targetFactory(factory, this.hostBindingTarget, this._targets);
        }
        if (factory.targetNodeId in this.targets) {
          const behavior = factory.createBehavior();
          behavior.bind(this);
          behaviors[i] = behavior;
        } else {
          let templateString = this.sourceTemplate.html;
          if (typeof templateString !== "string") {
            templateString = templateString.innerHTML;
          }
          const fragment = createRangeForNodes(this.firstChild, this.lastChild).cloneContents();
          const result = getHydrationDiagnostic().formatBindingMismatch(factory, this.firstChild, this.lastChild, getHostName(this.firstChild));
          throw new HydrationBindingError(result.message, factory, fragment, templateString, result.expected, result.received);
        }
      }
    } else {
      if (this.source !== null) {
        this.evaluateUnbindables();
      }
      this.isBound = false;
      this.source = source;
      this.context = context;
      for (let i = 0, ii = behaviors.length; i < ii; ++i) {
        behaviors[i].bind(this);
      }
    }
    this.isBound = true;
    this._hydrationStage = HydrationStage.hydrated;
  }
  unbind() {
    if (!this.isBound || this.source === null) {
      return;
    }
    this.evaluateUnbindables();
    this.source = null;
    this.context = this;
    this.isBound = false;
  }
  /**
   * Removes the view and unbinds its behaviors, disposing of DOM nodes afterward.
   * Once a view has been disposed, it cannot be inserted or bound again.
   */
  dispose() {
    removeNodeSequence(this.firstChild, this.lastChild);
    this.unbind();
  }
  onUnbind(behavior) {
    this.unbindables.push(behavior);
  }
  evaluateUnbindables() {
    const unbindables = this.unbindables;
    for (let i = 0, ii = unbindables.length; i < ii; ++i) {
      unbindables[i].unbind(this);
    }
    unbindables.length = 0;
  }
};
_a = Hydratable;
makeSerializationNoop(HydrationView);

// node_modules/@microsoft/fast-element/dist/esm/templating/html-binding-directive.js
function isContentTemplate(value) {
  return value.create !== void 0;
}
function updateContent(target, aspect, value, controller) {
  if (value === null || value === void 0) {
    value = "";
  }
  if (isContentTemplate(value)) {
    target.textContent = "";
    let view = target.$fastView;
    if (view === void 0) {
      if (isHydratable(controller) && isHydratable(value) && controller.bindingViewBoundaries[this.targetNodeId] !== void 0 && controller.hydrationStage !== HydrationStage.hydrated) {
        const viewNodes = controller.bindingViewBoundaries[this.targetNodeId];
        view = value.hydrate(viewNodes.first, viewNodes.last);
      } else {
        view = value.create();
      }
    } else {
      if (target.$fastTemplate !== value) {
        if (view.isComposed) {
          view.remove();
          view.unbind();
        }
        view = value.create();
      }
    }
    if (!view.isComposed) {
      view.isComposed = true;
      view.bind(controller.source, controller.context);
      view.insertBefore(target);
      target.$fastView = view;
      target.$fastTemplate = value;
    } else if (view.needsBindOnly) {
      view.needsBindOnly = false;
      view.bind(controller.source, controller.context);
    }
  } else {
    const view = target.$fastView;
    if (view !== void 0 && view.isComposed) {
      view.isComposed = false;
      view.remove();
      if (view.needsBindOnly) {
        view.needsBindOnly = false;
      } else {
        view.unbind();
      }
    }
    if (view === void 0 && isHydratable(controller) && controller.hydrationStage !== HydrationStage.hydrated) {
      const viewNodes = controller.bindingViewBoundaries[this.targetNodeId];
      if (viewNodes !== void 0) {
        removeNodeSequence(viewNodes.first, viewNodes.last);
        delete controller.bindingViewBoundaries[this.targetNodeId];
      }
    }
    target.textContent = value;
  }
}
function updateTokenList(target, aspect, value) {
  var _a2;
  const lookup = `${this.id}-t`;
  const state2 = (_a2 = target[lookup]) !== null && _a2 !== void 0 ? _a2 : target[lookup] = { v: 0, cv: /* @__PURE__ */ Object.create(null) };
  const classVersions = state2.cv;
  let version = state2.v;
  const tokenList = target[aspect];
  if (value !== null && value !== void 0 && value.length) {
    const names = value.split(/\s+/);
    for (let i = 0, ii = names.length; i < ii; ++i) {
      const currentName = names[i];
      if (currentName === "") {
        continue;
      }
      classVersions[currentName] = version;
      tokenList.add(currentName);
    }
  }
  state2.v = version + 1;
  if (version === 0) {
    return;
  }
  version -= 1;
  for (const name in classVersions) {
    if (classVersions[name] === version) {
      tokenList.remove(name);
    }
  }
}
var sinkLookup = {
  [DOMAspect.attribute]: DOM.setAttribute,
  [DOMAspect.booleanAttribute]: DOM.setBooleanAttribute,
  [DOMAspect.property]: (t, a, v) => t[a] = v,
  [DOMAspect.content]: updateContent,
  [DOMAspect.tokenList]: updateTokenList,
  [DOMAspect.event]: () => void 0
};
var HTMLBindingDirective = class {
  /**
   * Creates an instance of HTMLBindingDirective.
   * @param dataBinding - The binding configuration to apply.
   */
  constructor(dataBinding) {
    this.updateTarget = null;
    this.aspectType = DOMAspect.content;
    this.dataBinding = dataBinding;
  }
  /**
   * Creates HTML to be used within a template.
   * @param add - Can be used to add  behavior factories to a template.
   */
  createHTML(add) {
    return Markup.interpolation(add(this));
  }
  /**
   * Creates a behavior.
   */
  createBehavior() {
    var _a2;
    if (this.updateTarget === null) {
      const sink = sinkLookup[this.aspectType];
      const policy = (_a2 = this.dataBinding.policy) !== null && _a2 !== void 0 ? _a2 : this.policy;
      if (!sink) {
        throw FAST.error(Message.unsupportedBindingBehavior);
      }
      this.data = `${this.id}-d`;
      this.updateTarget = policy.protect(this.targetTagName, this.aspectType, this.targetAspect, sink);
    }
    return this;
  }
  /**
   * Attaches this binding to its target DOM node.
   * - For events: stores the controller reference on the target element and registers
   *   this directive as the EventListener via addEventListener. The directive's
   *   handleEvent() method will be called when the event fires.
   * - For content bindings: registers an unbind handler, then falls through to the
   *   default path.
   * - For all non-event bindings: creates (or reuses) an ExpressionObserver, evaluates
   *   the binding expression, and applies the result to the DOM via the updateTarget
   *   sink function. The observer will call handleChange() on future data changes.
   * @internal
   */
  bind(controller) {
    var _a2;
    const target = controller.targets[this.targetNodeId];
    switch (this.aspectType) {
      case DOMAspect.event:
        target[this.data] = controller;
        target.addEventListener(this.targetAspect, this, this.dataBinding.options);
        break;
      case DOMAspect.content:
        controller.onUnbind(this);
      // intentional fall through
      default: {
        const observer = (_a2 = target[this.data]) !== null && _a2 !== void 0 ? _a2 : target[this.data] = this.dataBinding.createObserver(this, this);
        observer.target = target;
        observer.controller = controller;
        const value = observer.bind(controller);
        if (controller._skipAttrUpdates && (this.aspectType === DOMAspect.attribute || this.aspectType === DOMAspect.booleanAttribute)) {
          break;
        }
        this.updateTarget(target, this.targetAspect, value, controller);
        break;
      }
    }
  }
  /** @internal */
  unbind(controller) {
    const target = controller.targets[this.targetNodeId];
    const view = target.$fastView;
    if (view !== void 0 && view.isComposed) {
      view.unbind();
      view.needsBindOnly = true;
    }
  }
  /**
   * Implements the EventListener interface. When a DOM event fires on the target
   * element, this method retrieves the ViewController stored on the element,
   * sets the event on the ExecutionContext so `c.event` is available to the
   * binding expression, and evaluates the expression. If the expression returns
   * anything other than `true`, the event's default action is prevented.
   * @internal
   */
  handleEvent(event) {
    const controller = event.currentTarget[this.data];
    if (controller.isBound) {
      ExecutionContext.setEvent(event);
      const result = this.dataBinding.evaluate(controller.source, controller.context);
      ExecutionContext.setEvent(null);
      if (result !== true) {
        event.preventDefault();
      }
    }
  }
  /**
   * Called by the ExpressionObserver when a tracked dependency changes.
   * Re-evaluates the binding expression via observer.bind() and pushes
   * the new value to the DOM through the updateTarget sink function.
   * This is the reactive update path that keeps the DOM in sync with data.
   *
   * Guards against stale notifications: when a view is unbound (e.g., after
   * a parent `when` directive tears down a child element), coupled-lifetime
   * observers may still hold active subscriptions. If a property change fires
   * on the source element while the view is inactive, this guard prevents
   * the binding expression from evaluating with a null source.
   * @internal
   */
  handleChange(binding, observer) {
    const controller = observer.controller;
    if (!controller.isBound) {
      return;
    }
    const target = observer.target;
    this.updateTarget(target, this.targetAspect, observer.bind(controller), controller);
  }
};
HTMLDirective.define(HTMLBindingDirective, { aspected: true });

// node_modules/@microsoft/fast-element/dist/esm/templating/compiler.js
var targetIdFrom = (parentId, nodeIndex) => `${parentId}.${nodeIndex}`;
var descriptorCache = {};
var next = {
  index: 0,
  node: null
};
function tryWarn(name) {
  if (!name.startsWith("fast-")) {
    FAST.warn(Message.hostBindingWithoutHost, { name });
  }
}
var warningHost = new Proxy(document.createElement("div"), {
  get(target, property) {
    tryWarn(property);
    const value = Reflect.get(target, property);
    return isFunction(value) ? value.bind(target) : value;
  },
  set(target, property, value) {
    tryWarn(property);
    return Reflect.set(target, property, value);
  }
});
var CompilationContext = class {
  constructor(fragment, directives, policy) {
    this.fragment = fragment;
    this.directives = directives;
    this.policy = policy;
    this.proto = null;
    this.nodeIds = /* @__PURE__ */ new Set();
    this.descriptors = {};
    this.factories = [];
  }
  addFactory(factory, parentId, nodeId, targetIndex, tagName3) {
    var _a2, _b;
    if (!this.nodeIds.has(nodeId)) {
      this.nodeIds.add(nodeId);
      this.addTargetDescriptor(parentId, nodeId, targetIndex);
    }
    factory.id = (_a2 = factory.id) !== null && _a2 !== void 0 ? _a2 : nextId();
    factory.targetNodeId = nodeId;
    factory.targetTagName = tagName3;
    factory.policy = (_b = factory.policy) !== null && _b !== void 0 ? _b : this.policy;
    this.factories.push(factory);
  }
  freeze() {
    this.proto = Object.create(null, this.descriptors);
    return this;
  }
  /**
   * Registers a lazy getter on the targets prototype that resolves a DOM node
   * by navigating from its parent's childNodes at the given index. Getters are
   * chained: accessing targets["r.0.2"] first resolves targets["r.0"] (which
   * resolves targets["r"]), then returns childNodes[2]. Results are cached so
   * each node is resolved at most once per view instance.
   */
  addTargetDescriptor(parentId, targetId, targetIndex) {
    const descriptors = this.descriptors;
    if (targetId === "r" || // root
    targetId === "h" || // host
    descriptors[targetId]) {
      return;
    }
    if (!descriptors[parentId]) {
      const index = parentId.lastIndexOf(".");
      const grandparentId = parentId.substring(0, index);
      const childIndex = parseInt(parentId.substring(index + 1), 10);
      this.addTargetDescriptor(grandparentId, parentId, childIndex);
    }
    let descriptor = descriptorCache[targetId];
    if (!descriptor) {
      const field = `_${targetId}`;
      descriptorCache[targetId] = descriptor = {
        get() {
          var _a2;
          return (_a2 = this[field]) !== null && _a2 !== void 0 ? _a2 : this[field] = this[parentId].childNodes[targetIndex];
        }
      };
    }
    descriptors[targetId] = descriptor;
  }
  /**
   * Creates a new HTMLView by cloning the compiled DocumentFragment and building
   * a targets object. The targets prototype contains lazy getters that resolve
   * each binding's target DOM node via childNodes traversal. Accessing every
   * registered nodeId eagerly triggers the getter chain so all nodes are resolved
   * before behaviors are bound.
   */
  createView(hostBindingTarget) {
    const fragment = this.fragment.cloneNode(true);
    const targets = Object.create(this.proto);
    targets.r = fragment;
    targets.h = hostBindingTarget !== null && hostBindingTarget !== void 0 ? hostBindingTarget : warningHost;
    for (const id3 of this.nodeIds) {
      Reflect.get(targets, id3);
    }
    return new HTMLView(fragment, this.factories, targets);
  }
};
function compileAttributes(context, parentId, node, nodeId, nodeIndex, includeBasicValues = false) {
  const attributes = node.attributes;
  const directives = context.directives;
  for (let i = 0, ii = attributes.length; i < ii; ++i) {
    const attr2 = attributes[i];
    const attrValue = attr2.value;
    const parseResult = Parser.parse(attrValue, directives);
    let result = null;
    if (parseResult === null) {
      if (includeBasicValues) {
        result = new HTMLBindingDirective(oneTime(() => attrValue, context.policy));
        HTMLDirective.assignAspect(result, attr2.name);
      }
    } else {
      result = Compiler.aggregate(parseResult, context.policy);
    }
    if (result !== null) {
      node.removeAttributeNode(attr2);
      i--;
      ii--;
      context.addFactory(result, parentId, nodeId, nodeIndex, node.tagName);
    }
  }
}
function compileContent(context, node, parentId, nodeId, nodeIndex) {
  const parseResult = Parser.parse(node.textContent, context.directives);
  if (parseResult === null) {
    next.node = node.nextSibling;
    next.index = nodeIndex + 1;
    return next;
  }
  let currentNode;
  let lastNode = currentNode = node;
  for (let i = 0, ii = parseResult.length; i < ii; ++i) {
    const currentPart = parseResult[i];
    if (i !== 0) {
      nodeIndex++;
      nodeId = targetIdFrom(parentId, nodeIndex);
      currentNode = lastNode.parentNode.insertBefore(document.createTextNode(""), lastNode.nextSibling);
    }
    if (isString(currentPart)) {
      currentNode.textContent = currentPart;
    } else {
      currentNode.textContent = " ";
      HTMLDirective.assignAspect(currentPart);
      context.addFactory(currentPart, parentId, nodeId, nodeIndex, null);
    }
    lastNode = currentNode;
  }
  next.index = nodeIndex + 1;
  next.node = lastNode.nextSibling;
  return next;
}
function compileChildren(context, parent, parentId) {
  let nodeIndex = 0;
  let childNode = parent.firstChild;
  while (childNode) {
    const result = compileNode(context, parentId, childNode, nodeIndex);
    childNode = result.node;
    nodeIndex = result.index;
  }
}
function compileNode(context, parentId, node, nodeIndex) {
  const nodeId = targetIdFrom(parentId, nodeIndex);
  switch (node.nodeType) {
    case 1:
      compileAttributes(context, parentId, node, nodeId, nodeIndex);
      compileChildren(context, node, nodeId);
      break;
    case 3:
      return compileContent(context, node, parentId, nodeId, nodeIndex);
    case 8: {
      const parts = Parser.parse(node.data, context.directives);
      if (parts !== null) {
        context.addFactory(Compiler.aggregate(parts), parentId, nodeId, nodeIndex, null);
      }
      break;
    }
  }
  next.index = nodeIndex + 1;
  next.node = node.nextSibling;
  return next;
}
function isMarker(node, directives) {
  return node && node.nodeType === 8 && Parser.parse(node.data, directives) !== null;
}
var templateTag = "TEMPLATE";
var Compiler = {
  /**
   * Compiles a template and associated directives into a compilation
   * result which can be used to create views.
   * @param html - The html string or template element to compile.
   * @param factories - The behavior factories referenced by the template.
   * @param policy - The security policy to compile the html with.
   * @remarks
   * The template that is provided for compilation is altered in-place
   * and cannot be compiled again. If the original template must be preserved,
   * it is recommended that you clone the original and pass the clone to this API.
   * @public
   */
  compile(html2, factories, policy = DOM.policy) {
    let template3;
    if (isString(html2)) {
      template3 = document.createElement(templateTag);
      template3.innerHTML = policy.createHTML(html2);
      const fec = template3.content.firstElementChild;
      if (fec !== null && fec.tagName === templateTag) {
        template3 = fec;
      }
    } else {
      template3 = html2;
    }
    if (!template3.content.firstChild && !template3.content.lastChild) {
      template3.content.appendChild(document.createComment(""));
    }
    const fragment = document.adoptNode(template3.content);
    const context = new CompilationContext(fragment, factories, policy);
    compileAttributes(
      context,
      "",
      template3,
      /* host */
      "h",
      0,
      true
    );
    if (
      // If the first node in a fragment is a marker, that means it's an unstable first node,
      // because something like a when, repeat, etc. could add nodes before the marker.
      // To mitigate this, we insert a stable first node. However, if we insert a node,
      // that will alter the result of the TreeWalker. So, we also need to offset the target index.
      isMarker(fragment.firstChild, factories) || // Or if there is only one node and a directive, it means the template's content
      // is *only* the directive. In that case, HTMLView.dispose() misses any nodes inserted by
      // the directive. Inserting a new node ensures proper disposal of nodes added by the directive.
      fragment.childNodes.length === 1 && Object.keys(factories).length > 0
    ) {
      fragment.insertBefore(document.createComment(""), fragment.firstChild);
    }
    compileChildren(
      context,
      fragment,
      /* root */
      "r"
    );
    next.node = null;
    return context.freeze();
  },
  /**
   * Sets the default compilation strategy that will be used by the ViewTemplate whenever
   * it needs to compile a view preprocessed with the html template function.
   * @param strategy - The compilation strategy to use when compiling templates.
   */
  setDefaultStrategy(strategy) {
    this.compile = strategy;
  },
  /**
   * Aggregates an array of strings and directives into a single directive.
   * @param parts - A heterogeneous array of static strings interspersed with
   * directives.
   * @param policy - The security policy to use with the aggregated bindings.
   * @returns A single inline directive that aggregates the behavior of all the parts.
   */
  aggregate(parts, policy = DOM.policy) {
    if (parts.length === 1) {
      return parts[0];
    }
    let sourceAspect;
    let isVolatile = false;
    let bindingPolicy = void 0;
    const partCount = parts.length;
    const finalParts = parts.map((x) => {
      if (isString(x)) {
        return () => x;
      }
      sourceAspect = x.sourceAspect || sourceAspect;
      isVolatile = isVolatile || x.dataBinding.isVolatile;
      bindingPolicy = bindingPolicy || x.dataBinding.policy;
      return x.dataBinding.evaluate;
    });
    const expression = (scope, context) => {
      let output = "";
      for (let i = 0; i < partCount; ++i) {
        output += finalParts[i](scope, context);
      }
      return output;
    };
    const directive = new HTMLBindingDirective(oneWay(expression, bindingPolicy !== null && bindingPolicy !== void 0 ? bindingPolicy : policy, isVolatile));
    HTMLDirective.assignAspect(directive, sourceAspect);
    return directive;
  }
};

// node_modules/@microsoft/fast-element/dist/esm/templating/ref.js
var RefDirective = class extends StatelessAttachedAttributeDirective {
  /**
   * Bind this behavior.
   * @param controller - The view controller that manages the lifecycle of this behavior.
   */
  bind(controller) {
    controller.source[this.options] = controller.targets[this.targetNodeId];
  }
};
HTMLDirective.define(RefDirective);
var ref = (propertyName) => new RefDirective(propertyName);

// node_modules/@microsoft/fast-element/dist/esm/templating/template.js
var lastAttributeNameRegex = (
  /* eslint-disable-next-line no-control-regex, max-len */
  /([ \x09\x0a\x0c\x0d])([^\0-\x1F\x7F-\x9F "'>=/]+)([ \x09\x0a\x0c\x0d]*=[ \x09\x0a\x0c\x0d]*(?:[^ \x09\x0a\x0c\x0d"'`<>=]*|"[^"]*|'[^']*))$/
);
var noFactories = /* @__PURE__ */ Object.create(null);
var InlineTemplateDirective = class {
  /**
   * Creates an instance of InlineTemplateDirective.
   * @param template - The template to inline.
   */
  constructor(html2, factories = noFactories) {
    this.html = html2;
    this.factories = factories;
  }
  /**
   * Creates HTML to be used within a template.
   * @param add - Can be used to add  behavior factories to a template.
   */
  createHTML(add) {
    const factories = this.factories;
    for (const key in factories) {
      add(factories[key]);
    }
    return this.html;
  }
};
InlineTemplateDirective.empty = new InlineTemplateDirective("");
HTMLDirective.define(InlineTemplateDirective);
function createHTML2(value, prevString, add, definition3 = HTMLDirective.getForInstance(value)) {
  if (definition3.aspected) {
    const match = lastAttributeNameRegex.exec(prevString);
    if (match !== null) {
      HTMLDirective.assignAspect(value, match[2]);
    }
  }
  return value.createHTML(add);
}
var ViewTemplate = class _ViewTemplate {
  /**
   * Creates an instance of ViewTemplate.
   * @param html - The html representing what this template will instantiate, including placeholders for directives.
   * @param factories - The directives that will be connected to placeholders in the html.
   * @param policy - The security policy to use when compiling this template.
   */
  constructor(html2, factories = {}, policy) {
    this.policy = policy;
    this.result = null;
    this.html = html2;
    this.factories = factories;
  }
  /**
   * @internal
   */
  compile() {
    if (this.result === null) {
      this.result = Compiler.compile(this.html, this.factories, this.policy);
    }
    return this.result;
  }
  /**
   * Returns a directive that can inline the template.
   */
  inline() {
    return new InlineTemplateDirective(isString(this.html) ? this.html : this.html.innerHTML, this.factories);
  }
  /**
   * Sets the DOMPolicy for this template.
   * @param policy - The policy to associated with this template.
   * @returns The modified template instance.
   * @remarks
   * The DOMPolicy can only be set once for a template and cannot be
   * set after the template is compiled.
   */
  withPolicy(policy) {
    if (this.result) {
      throw FAST.error(Message.cannotSetTemplatePolicyAfterCompilation);
    }
    if (this.policy) {
      throw FAST.error(Message.onlySetTemplatePolicyOnce);
    }
    this.policy = policy;
    return this;
  }
  /**
   * Creates an HTMLView from this template, binds it to the source, and then appends it to the host.
   * @param source - The data source to bind the template to.
   * @param host - The Element where the template will be rendered.
   * @param hostBindingTarget - An HTML element to target the host bindings at if different from the
   * host that the template is being attached to.
   */
  render(source, host, hostBindingTarget) {
    const view = this.create(hostBindingTarget);
    view.bind(source);
    view.appendTo(host);
    return view;
  }
  /**
   * Creates an HTMLView instance based on this template definition.
   * @param hostBindingTarget - The element that host behaviors will be bound to.
   */
  create(hostBindingTarget) {
    return this.compile().createView(hostBindingTarget);
  }
  /**
   * Processes the tagged template literal's static strings and interpolated values and
   * creates a ViewTemplate.
   *
   * For each interpolated value:
   * 1. Functions (binding expressions, e.g., `x => x.name`) → wrapped in a one-way HTMLBindingDirective
   * 2. Binding instances → wrapped in an HTMLBindingDirective
   * 3. HTMLDirective instances → used as-is
   * 4. Static values (strings, numbers) → wrapped in a one-time HTMLBindingDirective
   *
   * Each directive's createHTML() is called with an `add` callback that registers
   * the factory in the factories record under a unique ID and returns that ID.
   * The directive inserts a placeholder marker (e.g., `fast-abc123{0}fast-abc123`) into
   * the HTML string so the compiler can later find and associate it with the factory.
   *
   * Aspect detection happens here too: the `lastAttributeNameRegex` checks whether
   * the placeholder appears inside an attribute value, and if so, assignAspect()
   * sets the correct DOMAspect (attribute, property, event, etc.) based on the
   * attribute name prefix.
   *
   * @param strings - The static strings to create the template with.
   * @param values - The dynamic values to create the template with.
   * @param policy - The DOMPolicy to associated with the template.
   * @returns A ViewTemplate.
   * @remarks
   * This API should not be used directly under normal circumstances because constructing
   * a template in this way, if not done properly, can open up the application to XSS
   * attacks. When using this API, provide a strong DOMPolicy that can properly sanitize
   * and also be sure to manually sanitize all static strings particularly if they can
   * come from user input.
   */
  static create(strings, values, policy) {
    let html2 = "";
    const factories = /* @__PURE__ */ Object.create(null);
    const add = (factory) => {
      var _a2;
      const id3 = (_a2 = factory.id) !== null && _a2 !== void 0 ? _a2 : factory.id = nextId();
      factories[id3] = factory;
      return id3;
    };
    for (let i = 0, ii = strings.length - 1; i < ii; ++i) {
      const currentString = strings[i];
      let currentValue = values[i];
      let definition3;
      html2 += currentString;
      if (isFunction(currentValue)) {
        currentValue = new HTMLBindingDirective(oneWay(currentValue));
      } else if (currentValue instanceof Binding) {
        currentValue = new HTMLBindingDirective(currentValue);
      } else if (!(definition3 = HTMLDirective.getForInstance(currentValue))) {
        const staticValue = currentValue;
        currentValue = new HTMLBindingDirective(oneTime(() => staticValue));
      }
      html2 += createHTML2(currentValue, currentString, add, definition3);
    }
    return new _ViewTemplate(html2 + strings[strings.length - 1], factories, policy);
  }
};
makeSerializationNoop(ViewTemplate);
var html = ((strings, ...values) => {
  if (Array.isArray(strings) && Array.isArray(strings.raw)) {
    return ViewTemplate.create(strings, values);
  }
  throw FAST.error(Message.directCallToHTMLTagNotAllowed);
});
html.partial = (html2) => {
  return new InlineTemplateDirective(html2);
};

// node_modules/@microsoft/fast-element/dist/esm/templating/slotted.js
var slotEvent = "slotchange";
var SlottedDirective = class extends NodeObservationDirective {
  /**
   * Begins observation of the nodes.
   * @param target - The target to observe.
   */
  observe(target) {
    target.addEventListener(slotEvent, this);
  }
  /**
   * Disconnects observation of the nodes.
   * @param target - The target to unobserve.
   */
  disconnect(target) {
    target.removeEventListener(slotEvent, this);
  }
  /**
   * Retrieves the raw nodes that should be assigned to the source property.
   * @param target - The target to get the node to.
   */
  getNodes(target) {
    return target.assignedNodes(this.options);
  }
  /** @internal */
  handleEvent(event) {
    const target = event.currentTarget;
    this.updateTarget(this.getSource(target), this.computeNodes(target));
  }
};
HTMLDirective.define(SlottedDirective);
function slotted(propertyOrOptions) {
  if (isString(propertyOrOptions)) {
    propertyOrOptions = { property: propertyOrOptions };
  }
  return new SlottedDirective(propertyOrOptions);
}

// node_modules/@fluentui/web-components/dist/esm/utils/typings.js
function isHTMLElement(...args) {
  return args.every((arg) => arg instanceof HTMLElement);
}

// node_modules/@fluentui/web-components/dist/esm/fluent-design-system.js
var FluentDesignSystem = Object.freeze({
  prefix: "fluent",
  shadowRootMode: "open",
  registry: globalThis.customElements
});

// node_modules/@fluentui/web-components/dist/esm/menu-item/menu-item.options.js
var MenuItemRole = {
  /**
   * The menu item has a "menuitem" role
   */
  menuitem: "menuitem",
  /**
   * The menu item has a "menuitemcheckbox" role
   */
  menuitemcheckbox: "menuitemcheckbox",
  /**
   * The menu item has a "menuitemradio" role
   */
  menuitemradio: "menuitemradio"
};
var roleForMenuItem = {
  [MenuItemRole.menuitem]: "menuitem",
  [MenuItemRole.menuitemcheckbox]: "menuitemcheckbox",
  [MenuItemRole.menuitemradio]: "menuitemradio"
};
function isMenuItem(element, tagName3 = "-menu-item") {
  if (element?.nodeType !== Node.ELEMENT_NODE) {
    return false;
  }
  return element.tagName.toLowerCase().endsWith(tagName3);
}
var tagName = `${FluentDesignSystem.prefix}-menu-item`;

// node_modules/@fluentui/web-components/dist/esm/menu-list/menu-list.base.js
var BaseMenuList = class _BaseMenuList extends FASTElement {
  itemsChanged(oldValue, newValue) {
    if (this.$fastController.isConnected && this.menuChildren !== void 0) {
      this.setItems();
    }
  }
  static {
    this.focusableElementRoles = MenuItemRole;
  }
  constructor() {
    super();
    this.elementInternals = this.attachInternals();
    this.isNestedMenu = () => {
      return this.parentElement !== null && isHTMLElement(this.parentElement) && this.parentElement.getAttribute("role") === "menuitem";
    };
    this.changedMenuItemHandler = (e) => {
      if (this.menuChildren === void 0) {
        return;
      }
      const changedMenuItem = e.target;
      const changeItemIndex = this.menuChildren.indexOf(changedMenuItem);
      if (changeItemIndex === -1) {
        return;
      }
      if (changedMenuItem.role === "menuitemradio" && changedMenuItem.checked === true) {
        for (let i = changeItemIndex - 1; i >= 0; --i) {
          const item = this.menuChildren[i];
          const role = item.role;
          if (role === MenuItemRole.menuitemradio) {
            item.checked = false;
          }
          if (role === "separator") {
            break;
          }
        }
        const maxIndex = this.menuChildren.length - 1;
        for (let i = changeItemIndex + 1; i <= maxIndex; ++i) {
          const item = this.menuChildren[i];
          const role = item.role;
          if (role === MenuItemRole.menuitemradio) {
            item.checked = false;
          }
          if (role === "separator") {
            break;
          }
        }
      }
    };
    this.elementInternals.role = "menu";
  }
  /**
   * @internal
   */
  connectedCallback() {
    super.connectedCallback();
    if (!this.slot && this.isNestedMenu()) {
      this.slot = "submenu";
    }
    Updates.enqueue(() => {
      this.setItems();
    });
    this.addEventListener("change", this.changedMenuItemHandler);
  }
  /**
   * @internal
   */
  disconnectedCallback() {
    super.disconnectedCallback();
    Array.from(this.children).forEach((child) => {
      Observable.getNotifier(child).unsubscribe(this, "hidden");
    });
    this.menuChildren = void 0;
    this.removeEventListener("change", this.changedMenuItemHandler);
  }
  /**
   * Focuses the first item in the menu.
   *
   * @public
   */
  focus() {
    this.menuItems?.find((item) => !item.disabled)?.focus();
  }
  setItems() {
    const children = Array.from(this.children);
    children.forEach((child) => {
      Observable.getNotifier(child).subscribe(this, "hidden");
    });
    this.menuChildren = children.filter((child) => !child.hasAttribute("hidden"));
    this.menuItems = this.menuChildren?.filter(this.isMenuItemElement);
  }
  /**
   * Method for Observable changes to the hidden attribute of child elements
   */
  handleChange(source, propertyName) {
    if (propertyName === "hidden") {
      this.setItems();
    }
  }
  /**
   * check if the item is a menu item
   */
  isMenuItemElement(el) {
    return isMenuItem(el) || isHTMLElement(el) && !!el.role && el.role in _BaseMenuList.focusableElementRoles;
  }
};
__decorate([
  observable
], BaseMenuList.prototype, "items", void 0);

// node_modules/@fluentui/web-components/dist/esm/menu-list/menu-list.js
var MenuList = class extends BaseMenuList {
  disconnectedCallback() {
    this.fg?.disconnect();
    super.disconnectedCallback();
  }
  setItems() {
    super.setItems();
    this.fgItems ??= new ArrayItemCollection(() => this.menuItems?.filter((i) => !i.hidden) ?? []);
    if (!this.fg) {
      this.fg = new FocusGroup(this, this.fgItems, {
        definition: {
          behavior: "menu",
          axis: "block",
          wrap: true
        }
      });
    } else {
      this.fg.update();
    }
  }
};

// node_modules/@fluentui/web-components/dist/esm/menu-list/menu-list.options.js
var tagName2 = `${FluentDesignSystem.prefix}-menu-list`;

// node_modules/@fluentui/web-components/dist/esm/theme/design-tokens.js
var colorNeutralForeground2 = "var(--colorNeutralForeground2)";
var colorNeutralForeground2Hover = "var(--colorNeutralForeground2Hover)";
var colorNeutralForeground2Pressed = "var(--colorNeutralForeground2Pressed)";
var colorNeutralForeground3 = "var(--colorNeutralForeground3)";
var colorNeutralForegroundDisabled = "var(--colorNeutralForegroundDisabled)";
var colorCompoundBrandForeground1Pressed = "var(--colorCompoundBrandForeground1Pressed)";
var colorNeutralBackground1 = "var(--colorNeutralBackground1)";
var colorNeutralBackground1Hover = "var(--colorNeutralBackground1Hover)";
var colorNeutralBackground1Selected = "var(--colorNeutralBackground1Selected)";
var colorNeutralBackgroundDisabled = "var(--colorNeutralBackgroundDisabled)";
var colorTransparentStroke = "var(--colorTransparentStroke)";
var colorStrokeFocus2 = "var(--colorStrokeFocus2)";
var borderRadiusMedium = "var(--borderRadiusMedium)";
var fontFamilyBase = "var(--fontFamilyBase)";
var fontSizeBase200 = "var(--fontSizeBase200)";
var fontSizeBase300 = "var(--fontSizeBase300)";
var fontWeightRegular = "var(--fontWeightRegular)";
var lineHeightBase200 = "var(--lineHeightBase200)";
var lineHeightBase300 = "var(--lineHeightBase300)";
var shadow16 = "var(--shadow16)";

// node_modules/@fluentui/web-components/dist/esm/utils/display.js
var hidden = `:host([hidden]){display:none}`;
function display(displayValue) {
  return `${hidden}:host{display:${displayValue}}`;
}

// node_modules/@fluentui/web-components/dist/esm/menu-list/menu-list.styles.js
var styles = css`
  ${display("grid")}

  :host {
    --_col-indicator-width: 0px;
    --_col-start-width: 0px;
    --_col-content-width: 1fr;
    --_col-end-width: 0px;
    --_col-submenu-width: 0px;

    contain: layout style;
    block-size: auto;
    max-inline-size: 300px;
    min-inline-size: 160px;
    inline-size: auto;
    background-color: ${colorNeutralBackground1};
    border: 1px solid ${colorTransparentStroke};
    border-radius: ${borderRadiusMedium};
    box-shadow: ${shadow16};
    grid-template-columns:
      10px
      var(--_col-indicator-width)
      var(--_col-start-width)
      var(--_col-content-width)
      var(--_col-end-width)
      var(--_col-submenu-width)
      10px;
    gap: 2px 0;
    padding: 4px;
  }

  ::slotted(*) {
    grid-column: 1 / -1;
  }

  :host(:has(> [role='menuitemradio'], > [role='menuitemcheckbox'])) {
    --_col-indicator-width: 24px;
  }

  :host(:has(> * > [slot='start'])) {
    --_col-start-width: 24px;
  }

  :host(:has(> * > [slot='end'])) {
    --_col-end-width: 24px;
  }

  :host(:has(> * > [slot='submenu'])) {
    --_col-submenu-width: 24px;
  }

  @scope {
    :scope:has(> [role='menuitemradio'], > [role='menuitemcheckbox']) {
      --_col-indicator-width: 24px;
    }

    :scope:has(> * > [slot='start']) {
      --_col-start-width: 24px;
    }

    :scope:has(> * > [slot='end']) {
      --_col-end-width: 24px;
    }

    :scope:has(> * > [slot='submenu']) {
      --_col-submenu-width: 24px;
    }
  }
`;

// node_modules/@fluentui/web-components/dist/esm/menu-list/menu-list.template.js
function menuTemplate() {
  return html`
    <template focusgroup="menu">
      <slot ${slotted("items")}></slot>
    </template>
  `;
}
var template = menuTemplate();

// node_modules/@fluentui/web-components/dist/esm/menu-list/menu-list.definition.js
var definition = {
  name: tagName2,
  registry: FluentDesignSystem.registry,
  styles,
  template
};

// node_modules/@fluentui/web-components/dist/esm/utils/template-helpers.js
function staticallyCompose(item) {
  if (!item) {
    return InlineTemplateDirective.empty;
  }
  if (typeof item === "string") {
    return new InlineTemplateDirective(item);
  }
  if ("inline" in item) {
    return item.inline();
  }
  return item;
}

// node_modules/@fluentui/web-components/dist/esm/patterns/start-end.js
var StartEnd = class {
};
function endSlotTemplate(options) {
  return html` <slot name="end" ${ref("end")}>${staticallyCompose(options.end)}</slot> `.inline();
}
function startSlotTemplate(options) {
  return html` <slot name="start" ${ref("start")}>${staticallyCompose(options.start)}</slot> `.inline();
}

// node_modules/@fluentui/web-components/dist/esm/utils/apply-mixins.js
function applyMixins(derivedCtor, ...baseCtors) {
  const derivedAttributes = AttributeConfiguration.locate(derivedCtor);
  baseCtors.forEach((baseCtor) => {
    Object.getOwnPropertyNames(baseCtor.prototype).forEach((name) => {
      if (name !== "constructor") {
        Object.defineProperty(
          derivedCtor.prototype,
          name,
          /* eslint-disable-next-line @typescript-eslint/no-non-null-assertion */
          Object.getOwnPropertyDescriptor(baseCtor.prototype, name)
        );
      }
    });
    const baseAttributes = AttributeConfiguration.locate(baseCtor);
    baseAttributes.forEach((x) => derivedAttributes.push(x));
  });
}

// node_modules/@fluentui/web-components/dist/esm/utils/support.js
var AnchorPositioningCSSSupported = CSS.supports("anchor-name: --a");
var AnchorPositioningHTMLSupported = "anchor" in HTMLElement.prototype;
var CustomStatesSetSupported = CSS.supports("selector(:state(g))");

// node_modules/@fluentui/web-components/dist/esm/utils/element-internals.js
var statesMap = /* @__PURE__ */ new Map();
function stateSelector(state2) {
  return statesMap.get(state2) ?? statesMap.set(state2, CustomStatesSetSupported ? `:state(${state2})` : `[state--${state2}]`).get(state2);
}
function toggleState(elementInternals, state2, force) {
  if (!state2 || !elementInternals) {
    return;
  }
  if (!CustomStatesSetSupported) {
    elementInternals.shadowRoot.host.toggleAttribute(`state--${state2}`, force);
    return;
  }
  if (force ?? !elementInternals.states.has(state2)) {
    elementInternals.states.add(state2);
    return;
  }
  elementInternals.states.delete(state2);
}

// node_modules/@fluentui/web-components/dist/esm/utils/autofocus.js
function maybeSetAutoFocus(element) {
  const doc = element.ownerDocument;
  if (element?.isConnected && element?.hasAttribute("autofocus") && // Note: opacity=0 is considered visible based on the native `autofocus` implementation
  element?.checkVisibility?.({ contentVisibilityAuto: true, visibilityProperty: true }) && [null, element, doc.body, doc.documentElement].includes(doc.activeElement)) {
    Updates.enqueue(() => {
      element.focus();
    });
  }
}

// node_modules/@fluentui/web-components/dist/esm/menu-item/menu-item.js
var MenuItem = class extends FASTElement {
  constructor() {
    super(...arguments);
    this.elementInternals = this.attachInternals();
    this.role = MenuItemRole.menuitem;
    this.checked = false;
    this.handleMenuItemKeyDown = (e) => {
      if (e.defaultPrevented) {
        return false;
      }
      switch (e.key) {
        case "Enter":
        case " ":
          this.invoke();
          return false;
        case "ArrowRight":
          if (!this.disabled) {
            this.submenu?.togglePopover(true);
            this.submenu?.focus();
          }
          return false;
        case "ArrowLeft":
          if (this.parentElement?.hasAttribute("popover")) {
            this.parentElement.togglePopover(false);
            this.parentElement.parentElement?.focus();
          }
          return false;
      }
      return true;
    };
    this.handleMenuItemClick = (e) => {
      if (e.defaultPrevented || this.disabled) {
        return false;
      }
      this.invoke();
      return false;
    };
    this.handleMouseOver = (e) => {
      if (this.disabled) {
        return false;
      }
      this.submenu?.togglePopover(true);
      return false;
    };
    this.handleMouseOut = (e) => {
      if (this.contains(document.activeElement)) {
        return false;
      }
      this.submenu?.togglePopover(false);
      return false;
    };
    this.handleToggle = (e) => {
      if (!(e instanceof ToggleEvent)) {
        return;
      }
      if (e.newState === "open") {
        this.elementInternals.ariaExpanded = "true";
        this.setSubmenuPosition();
      }
      if (e.newState === "closed") {
        this.elementInternals.ariaExpanded = "false";
      }
      this.submenu?.setAttribute("focusgroup", e.newState === "open" ? "menu" : "none");
    };
    this.handleSubmenuFocusOut = (e) => {
      if (e.relatedTarget && this.submenu?.contains(e.relatedTarget)) {
        return;
      }
      this.submenu?.togglePopover(false);
    };
    this.invoke = () => {
      if (this.disabled) {
        return;
      }
      switch (this.role) {
        case MenuItemRole.menuitemcheckbox:
          this.checked = !this.checked;
          break;
        case MenuItemRole.menuitem:
          if (!!this.submenu) {
            this.submenu.togglePopover(true);
            this.submenu.focus();
            break;
          }
          this.$emit("change");
          break;
        case MenuItemRole.menuitemradio:
          if (!this.checked) {
            this.checked = true;
          }
          break;
      }
    };
    this.setSubmenuPosition = () => {
      if (!CSS.supports("anchor-name", "--anchor") && !!this.submenu) {
        const thisRect = this.getBoundingClientRect();
        const thisSubmenuRect = this.submenu.getBoundingClientRect();
        const inlineEnd = getComputedStyle(this).direction === "ltr" ? "right" : "left";
        if (thisRect.width + thisSubmenuRect.width > window.innerWidth * 0.75) {
          this.submenu.style.translate = "0 -100%";
          return;
        }
        if (thisRect[inlineEnd] + thisSubmenuRect.width > window.innerWidth) {
          this.submenu.style.translate = "-100% 0";
          return;
        }
        this.submenu.style.translate = `${thisRect.width - 8}px 0`;
      }
    };
  }
  /**
   * Handles changes to disabled attribute custom states and element internals
   * @param prev - the previous state
   * @param next - the next state
   */
  disabledChanged(prev, next2) {
    this.elementInternals.ariaDisabled = !!next2 ? `${next2}` : null;
    toggleState(this.elementInternals, "disabled", next2);
  }
  /**
   * Handles changes to role attribute element internals properties
   * @param prev - the previous state
   * @param next - the next state
   */
  roleChanged(prev, next2) {
    this.elementInternals.role = next2 ?? MenuItemRole.menuitem;
  }
  /**
   * Handles changes to checked attribute custom states and element internals
   * @param prev - the previous state
   * @param next - the next state
   */
  checkedChanged(prev, next2) {
    const checkableMenuItem = this.role !== MenuItemRole.menuitem;
    this.elementInternals.ariaChecked = checkableMenuItem ? `${!!next2}` : null;
    toggleState(this.elementInternals, "checked", checkableMenuItem ? next2 : false);
    if (this.$fastController.isConnected) {
      this.$emit("change", next2, { bubbles: true });
    }
  }
  /**
   * Sets the submenu and updates its position.
   *
   * @internal
   */
  slottedSubmenuChanged(prev, next2) {
    this.submenu?.removeEventListener("toggle", this.handleToggle);
    this.submenu?.removeEventListener("focusout", this.handleSubmenuFocusOut);
    if (next2.length) {
      this.submenu = next2[0];
      this.submenu.toggleAttribute("popover", true);
      this.submenu.setAttribute("focusgroup", "none");
      this.submenu.addEventListener("toggle", this.handleToggle);
      this.submenu.addEventListener("focusout", this.handleSubmenuFocusOut);
      this.elementInternals.ariaHasPopup = "menu";
      toggleState(this.elementInternals, "submenu", true);
    } else {
      this.elementInternals.ariaHasPopup = null;
      toggleState(this.elementInternals, "submenu", false);
    }
  }
  connectedCallback() {
    super.connectedCallback();
    this.elementInternals.role = this.role ?? MenuItemRole.menuitem;
    this.elementInternals.ariaChecked = this.role !== MenuItemRole.menuitem ? `${!!this.checked}` : null;
    maybeSetAutoFocus(this);
  }
};
__decorate([
  attr({ mode: "boolean" })
], MenuItem.prototype, "disabled", void 0);
__decorate([
  attr
], MenuItem.prototype, "role", void 0);
__decorate([
  attr({ mode: "boolean" })
], MenuItem.prototype, "checked", void 0);
__decorate([
  attr({ mode: "boolean" })
], MenuItem.prototype, "hidden", void 0);
__decorate([
  observable
], MenuItem.prototype, "slottedSubmenu", void 0);
__decorate([
  observable
], MenuItem.prototype, "submenu", void 0);
applyMixins(MenuItem, StartEnd);

// node_modules/@fluentui/web-components/dist/esm/styles/states/index.js
var activeState = stateSelector("active");
var badInputState = stateSelector("bad-input");
var checkedState = stateSelector("checked");
var customErrorState = stateSelector("custom-error");
var descriptionState = stateSelector("description");
var disabledState = stateSelector("disabled");
var errorState = stateSelector("error");
var flipBlockState = stateSelector("flip-block");
var focusVisibleState = stateSelector("focus-visible");
var hasMessageState = stateSelector("has-message");
var indeterminateState = stateSelector("indeterminate");
var multipleState = stateSelector("multiple");
var openState = stateSelector("open");
var patternMismatchState = stateSelector("pattern-mismatch");
var placeholderShownState = stateSelector("placeholder-shown");
var pressedState = stateSelector("pressed");
var rangeOverflowState = stateSelector("range-overflow");
var rangeUnderflowState = stateSelector("range-underflow");
var requiredState = stateSelector("required");
var selectedState = stateSelector("selected");
var stepMismatchState = stateSelector("step-mismatch");
var submenuState = stateSelector("submenu");
var tooLongState = stateSelector("too-long");
var tooShortState = stateSelector("too-short");
var typeMismatchState = stateSelector("type-mismatch");
var userInvalidState = stateSelector("user-invalid");
var validState = stateSelector("valid");
var valueMissingState = stateSelector("value-missing");

// node_modules/@fluentui/web-components/dist/esm/menu-item/menu-item.styles.js
var styles2 = css`
  ${display("flex")}

  :host {
    align-items: center;
    background: ${colorNeutralBackground1};
    border-radius: ${borderRadiusMedium};
    box-sizing: border-box;
    color: ${colorNeutralForeground2};
    cursor: pointer;
    font: ${fontWeightRegular} ${fontSizeBase300} / ${lineHeightBase300} ${fontFamilyBase};
    gap: 4px;
    min-block-size: 32px;
    overflow: visible;
    padding-inline: 10px;
  }

  @supports (grid-template-columns: subgrid) {
    :host {
      display: grid;
      gap: 0;
      grid-template-columns: subgrid;
      padding-inline: unset;
    }
  }

  :host(:hover) {
    background: ${colorNeutralBackground1Hover};
    color: ${colorNeutralForeground2Hover};
  }

  :host(:active) {
    background-color: ${colorNeutralBackground1Selected};
    color: ${colorNeutralForeground2Pressed};
  }

  :host(:active) ::slotted([slot='start']) {
    color: ${colorCompoundBrandForeground1Pressed};
  }

  :host(${disabledState}) {
    background-color: ${colorNeutralBackgroundDisabled};
    color: ${colorNeutralForegroundDisabled};
  }

  :host(${disabledState}) ::slotted([slot='start']),
  :host(${disabledState}) ::slotted([slot='end']) {
    color: ${colorNeutralForegroundDisabled};
  }

  :host(:focus-visible) {
    border-radius: ${borderRadiusMedium};
    outline: 2px solid ${colorStrokeFocus2};
  }

  :host(:not(${checkedState})) .indicator,
  :host(:not(${checkedState})) ::slotted([slot='indicator']),
  :host(:not(${submenuState})) .submenu-glyph,
  :host(:not(${submenuState})) ::slotted([slot='submenu-glyph']) {
    display: none;
  }

  ::slotted([slot='end']) {
    color: ${colorNeutralForeground3};
    font: ${fontWeightRegular} ${fontSizeBase200} / ${lineHeightBase200} ${fontFamilyBase};
    white-space: nowrap;
  }

  .indicator,
  ::slotted([slot='indicator']) {
    grid-column: 2 / span 1;
    width: 20px;
  }

  ::slotted([slot='start']) {
    display: inline-flex;
    grid-column: 3 / span 1;
  }

  .content {
    flex-grow: 1;
    grid-column: 4 / span 1;
    padding: 0 2px;
    white-space: nowrap;
  }

  ::slotted([slot='end']) {
    grid-column: 5 / span 1;
    justify-self: end;
  }

  .submenu-glyph,
  ::slotted([slot='submenu-glyph']) {
    grid-column: 6 / span 1;
    justify-self: end;
  }

  @layer popover {
    :host {
      anchor-name: --menu-trigger;
      position: relative;
    }

    @position-try --inline-inside {
      inset-inline-start: unset;
      inset-inline-end: anchor(inside);
    }

    ::slotted([popover]) {
      margin: 0;
      max-block-size: var(--menu-max-height, auto);
      position: fixed;
      position-anchor: --menu-trigger;
      inset: unset;
      inset-block-start: anchor(inside);
      inset-inline-start: anchor(outside);
      position-try-fallbacks: --inline-inside, flip-block, flip-block --inline-inside;
      z-index: 1;
    }

    ::slotted([popover]:not(:popover-open)) {
      display: none;
    }

    /* Fallback for no anchor-positioning */
    @supports not (anchor-name: --menu-trigger) {
      ::slotted([popover]) {
        align-self: start;
      }
    }
  }

  @media (forced-colors: active) {
    :host(${disabledState}),
    :host(${disabledState}) ::slotted([slot='start']),
    :host(${disabledState}) ::slotted([slot='end']) {
      color: GrayText;
    }
  }
`;

// node_modules/@fluentui/web-components/dist/esm/menu-item/menu-item.template.js
var Checkmark16Filled = html.partial(`<svg class="indicator" fill="currentColor" aria-hidden="true" width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><path d="M14.05 3.49c.28.3.27.77-.04 1.06l-7.93 7.47A.85.85 0 014.9 12L2.22 9.28a.75.75 0 111.06-1.06l2.24 2.27 7.47-7.04a.75.75 0 011.06.04z" fill="currentColor"></path></svg>`);
var chevronRight16Filled = html.partial(`<svg class="submenu-glyph" fill="currentColor" aria-hidden="true" width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><path d="M5.74 3.2a.75.75 0 00-.04 1.06L9.23 8 5.7 11.74a.75.75 0 101.1 1.02l4-4.25a.75.75 0 000-1.02l-4-4.25a.75.75 0 00-1.06-.04z" fill="currentColor"></path></svg>`);
function menuItemTemplate(options = {}) {
  return html`
    <template
      tabindex="0"
      @keydown="${(x, c) => x.handleMenuItemKeyDown(c.event)}"
      @click="${(x, c) => x.handleMenuItemClick(c.event)}"
      @mouseover="${(x, c) => x.handleMouseOver(c.event)}"
      @mouseout="${(x, c) => x.handleMouseOut(c.event)}"
      @toggle="${(x, c) => x.handleToggle(c.event)}"
    >
      <slot name="indicator"> ${staticallyCompose(options.indicator)} </slot>
      ${startSlotTemplate(options)}
      <div part="content" class="content">
        <slot></slot>
      </div>
      ${endSlotTemplate(options)}
      <slot name="submenu-glyph"> ${staticallyCompose(options.submenuGlyph)} </slot>
      <slot name="submenu" ${slotted({ property: "slottedSubmenu" })}></slot>
    </template>
  `;
}
var template2 = menuItemTemplate({
  indicator: Checkmark16Filled,
  submenuGlyph: chevronRight16Filled
});

// node_modules/@fluentui/web-components/dist/esm/menu-item/menu-item.definition.js
var definition2 = {
  name: tagName,
  registry: FluentDesignSystem.registry,
  styles: styles2,
  template: template2
};

// frontend/comfyui/ui/menus/MenuControlRuntime.ts
import "../surface/appearance/SpectrumThemeRuntime.js";
import { readOwnedVisualStyle } from "../surface/appearance/VisualStyleBoundary.js";
if (!customElements.get("fluent-menu-list")) await MenuList.define(definition);
if (!customElements.get("fluent-menu-item")) await MenuItem.define(definition2);
function createMenuList(document2) {
  const menu = document2.createElement(
    readOwnedVisualStyle(document2) === "spectrum" ? "sp-menu" : "fluent-menu-list"
  );
  if (menu.localName === "sp-menu") menu.setAttribute("ignore", "");
  return menu;
}
function createMenuItem(document2) {
  return document2.createElement(
    readOwnedVisualStyle(document2) === "spectrum" ? "sc-spectrum-menu-item" : "fluent-menu-item"
  );
}
function setMenuItemChecked(row, checked) {
  row.setAttribute("aria-checked", String(checked));
  if (checked)
    row.setAttribute(
      readOwnedVisualStyle(row.ownerDocument) === "spectrum" ? "selected" : "checked",
      ""
    );
}
function menuItemSlot(document2, position) {
  return readOwnedVisualStyle(document2) === "spectrum" ? position === "start" ? "icon" : "value" : position;
}
export {
  createMenuItem,
  createMenuList,
  menuItemSlot,
  setMenuItemChecked
};

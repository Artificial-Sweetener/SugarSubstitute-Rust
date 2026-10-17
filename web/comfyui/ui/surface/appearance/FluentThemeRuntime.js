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
    register(definition7) {
      if (typeToDefinition.has(definition7.type)) {
        return false;
      }
      typeToDefinition.set(definition7.type, definition7);
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
  protect(tagName7, aspect, aspectName, sink) {
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
function toNumber(value) {
  if (value === null || value === void 0) {
    return null;
  }
  const number = value * 1;
  return isNaN(number) ? null : number;
}
var nullableNumberConverter = {
  toView(value) {
    const output = toNumber(value);
    return output ? output.toString() : output;
  },
  fromView: toNumber
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
function reduceStyles(styles7) {
  return styles7.reduce((reduced, current) => {
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
  constructor(styles7) {
    this.styles = styles7;
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
  static normalize(styles7) {
    return styles7 === void 0 ? void 0 : Array.isArray(styles7) ? new _ElementStyles(styles7) : styles7 instanceof _ElementStyles ? styles7 : new _ElementStyles([styles7]);
  }
};
ElementStyles.supportsAdoptedStyleSheets = Array.isArray(document.adoptedStyleSheets) && "replace" in CSSStyleSheet.prototype;
function createAdoptedSheetsStrategy() {
  const cache = /* @__PURE__ */ new Map();
  return class FallbackAdoptedSheetsStrategy {
    constructor(styles7) {
      this.sheets = styles7.map((x) => {
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
    constructor(styles7) {
      this.styles = styles7;
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
      const styles7 = t.querySelectorAll(`.${this.styleClass}`);
      for (let i = 0, ii = styles7.length; i < ii; ++i) {
        t.removeChild(styles7[i]);
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
  register(definition7) {
    if (!typeRegistry.register(definition7)) {
      return false;
    }
    const registeredTypes = getRegisteredTypes(definition7.registry);
    if (!Object.prototype.hasOwnProperty.call(registeredTypes, definition7.name)) {
      Observable.defineProperty(registeredTypes, definition7.name);
    }
    registeredTypes[definition7.name] = definition7.type;
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
  const definition7 = getDefinitionForType(registeredTypes[name]);
  if (definition7 !== void 0) {
    return Promise.resolve(definition7);
  }
  return new Promise((resolve) => {
    const notifier = Observable.getNotifier(registeredTypes);
    const subscriber = {
      handleChange: () => {
        const definition8 = getDefinitionForType(registeredTypes[name]);
        if (definition8 === void 0) {
          return;
        }
        notifier.unsubscribe(subscriber, name);
        resolve(definition8);
      }
    };
    notifier.subscribe(subscriber, name);
  });
}
function trackFASTElementInstance(definition7, instance) {
  if (instanceRefs.has(instance)) {
    return false;
  }
  let instances = definitionInstances.get(definition7);
  const isFirstInstance = instances === void 0;
  if (instances === void 0) {
    instances = /* @__PURE__ */ new Set();
    definitionInstances.set(definition7, instances);
  }
  const ref2 = new WeakRef(instance);
  instances.add(ref2);
  instanceRefs.set(instance, ref2);
  return isFirstInstance;
}
function trackedFASTElementInstances(definition7, callback) {
  const instances = definitionInstances.get(definition7);
  if (instances === void 0) {
    return;
  }
  definitionInstances.delete(definition7);
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
function finalizeResolvedTemplate(definition7, template7) {
  pendingTemplateResolutions.delete(definition7);
  if (definition7.template === void 0 && template7 !== void 0) {
    definition7.template = template7;
  }
  if (definition7.template !== void 0) {
    templateResolutionErrors.delete(definition7);
    templateResolvers.delete(definition7);
    return definition7.template;
  }
  return void 0;
}
function applyFASTElementExtensions(definition7, registry3 = definition7.registry, extensions) {
  if (!(extensions === null || extensions === void 0 ? void 0 : extensions.length)) {
    return;
  }
  const typedDefinition = definition7;
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
    extension(definition7);
  }
}
function getLateAttributeLookup(definition7) {
  var _a2;
  return (_a2 = lateAttributeLookups.get(definition7)) !== null && _a2 !== void 0 ? _a2 : null;
}
function resolveFASTElementTemplate(definition7) {
  if (definition7.template !== void 0) {
    templateResolutionErrors.delete(definition7);
    return definition7.template;
  }
  const pendingResolution = pendingTemplateResolutions.get(definition7);
  if (pendingResolution) {
    return pendingResolution;
  }
  const templateResolver = templateResolvers.get(definition7);
  if (!templateResolver) {
    return void 0;
  }
  templateResolutionErrors.delete(definition7);
  let template7;
  try {
    template7 = templateResolver(definition7);
  } catch (error) {
    templateResolutionErrors.set(definition7, error);
    throw error;
  }
  if (isPromiseLike(template7)) {
    const resolution = Promise.resolve(template7).then((resolvedTemplate) => finalizeResolvedTemplate(definition7, resolvedTemplate)).catch((error) => {
      pendingTemplateResolutions.delete(definition7);
      templateResolutionErrors.set(definition7, error);
      throw error;
    });
    pendingTemplateResolutions.set(definition7, resolution);
    return resolution;
  }
  return finalizeResolvedTemplate(definition7, template7);
}
function setFASTElementTemplateError(definition7, error) {
  const typedDefinition = definition7;
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
        void Promise.resolve().then(() => resolveFASTElementTemplate(this)).then((template7) => {
          if (template7 !== void 0 && !registry3.get(this.name)) {
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
    const definition7 = fastElementBaseTypes.has(type) || fastElementRegistry.getByType(type) ? new _FASTElementDefinition(class extends type {
    }, nameOrDef) : new _FASTElementDefinition(type, nameOrDef);
    return Promise.resolve(definition7);
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
      const definition7 = this.definition;
      if (this.source.resolveTemplate) {
        this._template = this.source.resolveTemplate();
      } else if (definition7.template) {
        this._template = (_a2 = definition7.template) !== null && _a2 !== void 0 ? _a2 : null;
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
      const definition7 = this.definition;
      if (this.source.resolveStyles) {
        this._mainStyles = this.source.resolveStyles();
      } else if (definition7.styles) {
        this._mainStyles = (_a2 = definition7.styles) !== null && _a2 !== void 0 ? _a2 : null;
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
  constructor(element, definition7) {
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
    this.definition = definition7;
    this.shadowOptions = definition7.shadowOptions;
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
  addStyles(styles7) {
    var _a2;
    if (!styles7) {
      return;
    }
    const source = this.source;
    if (styles7 instanceof HTMLElement) {
      const target = (_a2 = getShadowRoot(source)) !== null && _a2 !== void 0 ? _a2 : this.source;
      target.append(styles7);
    } else if (!styles7.isAttachedTo(source)) {
      styles7.addStylesTo(source);
    }
  }
  /**
   * Removes styles from this element. Providing an HTMLStyleElement will detach the element instance from the shadowRoot.
   * @param styles - the styles to remove.
   */
  removeStyles(styles7) {
    var _a2;
    if (!styles7) {
      return;
    }
    const source = this.source;
    if (styles7 instanceof HTMLElement) {
      const target = (_a2 = getShadowRoot(source)) !== null && _a2 !== void 0 ? _a2 : source;
      target.removeChild(styles7);
    } else if (styles7.isAttachedTo(source)) {
      styles7.removeStylesFrom(source);
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
  renderTemplate(template7) {
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
    if (template7) {
      const hasPrerenderedContent = this.hasExistingShadowRoot && this.needsInitialization;
      let didHydrate = false;
      if (hasPrerenderedContent && _ElementController.hydrationHook) {
        didHydrate = _ElementController.hydrationHook(this, template7, element, host);
      }
      if (!didHydrate) {
        this.renderClientSide(template7, element, host);
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
  renderClientSide(template7, element, host) {
    if (this.hasExistingShadowRoot) {
      for (let child = host.firstChild; child !== null; child = host.firstChild) {
        host.removeChild(child);
      }
      this.hasExistingShadowRoot = false;
    }
    this.view = template7.render(element, host, element);
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
    const definition7 = FASTElementDefinition.getForInstance(element);
    if (definition7 === void 0) {
      throw FAST.error(Message.missingElementDefinition);
    }
    if (definition7.template === void 0 && trackFASTElementInstance(definition7, element)) {
      const notifier = Observable.getNotifier(definition7);
      const subscriber = {
        handleChange: () => {
          notifier.unsubscribe(subscriber, "template");
          trackedFASTElementInstances(definition7, (tracked) => {
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
    return element.$fastController = new elementControllerStrategy(element, definition7);
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
  constructor(styles7) {
    const styleSheetCache = _AdoptedStyleSheetsStrategy.styleSheetCache;
    this.sheets = styles7.map((x) => {
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
  constructor(styles7) {
    this.styles = styles7;
    this.styleClass = nextStyleId();
  }
  addStylesTo(target) {
    target = usableStyleTarget(normalizeStyleTarget(target));
    const styles7 = this.styles;
    const styleClass = this.styleClass;
    for (let i = 0; i < styles7.length; i++) {
      const element = document.createElement("style");
      element.innerHTML = styles7[i];
      element.className = styleClass;
      target.append(element);
    }
  }
  removeStylesFrom(target) {
    target = usableStyleTarget(normalizeStyleTarget(target));
    const styles7 = target.querySelectorAll(`.${this.styleClass}`);
    for (let i = 0, ii = styles7.length; i < ii; ++i) {
      target.removeChild(styles7[i]);
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
    const template7 = resolveFASTElementTemplate(def);
    if (isPromiseLike2(template7)) {
      return template7.then(() => def.define().type);
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
  const styles7 = [];
  let cssString = "";
  for (let i = 0, ii = strings.length - 1; i < ii; ++i) {
    cssString += strings[i];
    let value = values[i];
    if (CSSDirective.getForInstance(value) !== void 0) {
      value = value.createCSS();
    }
    if (value instanceof ElementStyles || value instanceof CSSStyleSheet) {
      if (cssString.trim() !== "") {
        styles7.push(cssString);
        cssString = "";
      }
      styles7.push(value);
    } else {
      cssString += value;
    }
  }
  cssString += strings[strings.length - 1];
  if (cssString.trim() !== "") {
    styles7.push(cssString);
  }
  return styles7;
}
var css = ((strings, ...values) => {
  return new ElementStyles(collectStyles(strings, values));
});
var CSSPartial = class {
  constructor(styles7) {
    this.value = styles7.length === 0 ? "" : styles7.length === 1 ? styles7[0] : new ElementStyles(styles7);
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
  addFactory(factory, parentId, nodeId, targetIndex, tagName7) {
    var _a2, _b;
    if (!this.nodeIds.has(nodeId)) {
      this.nodeIds.add(nodeId);
      this.addTargetDescriptor(parentId, nodeId, targetIndex);
    }
    factory.id = (_a2 = factory.id) !== null && _a2 !== void 0 ? _a2 : nextId();
    factory.targetNodeId = nodeId;
    factory.targetTagName = tagName7;
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
    let template7;
    if (isString(html2)) {
      template7 = document.createElement(templateTag);
      template7.innerHTML = policy.createHTML(html2);
      const fec = template7.content.firstElementChild;
      if (fec !== null && fec.tagName === templateTag) {
        template7 = fec;
      }
    } else {
      template7 = html2;
    }
    if (!template7.content.firstChild && !template7.content.lastChild) {
      template7.content.appendChild(document.createComment(""));
    }
    const fragment = document.adoptNode(template7.content);
    const context = new CompilationContext(fragment, factories, policy);
    compileAttributes(
      context,
      "",
      template7,
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
function createHTML2(value, prevString, add, definition7 = HTMLDirective.getForInstance(value)) {
  if (definition7.aspected) {
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
      let definition7;
      html2 += currentString;
      if (isFunction(currentValue)) {
        currentValue = new HTMLBindingDirective(oneWay(currentValue));
      } else if (currentValue instanceof Binding) {
        currentValue = new HTMLBindingDirective(currentValue);
      } else if (!(definition7 = HTMLDirective.getForInstance(currentValue))) {
        const staticValue = currentValue;
        currentValue = new HTMLBindingDirective(oneTime(() => staticValue));
      }
      html2 += createHTML2(currentValue, currentString, add, definition7);
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

// node_modules/@fluentui/web-components/dist/esm/utils/unique-id.js
var uniqueIdCounter = 0;
function uniqueId(prefix = "id-") {
  const str = `${prefix}${uniqueIdCounter++}`;
  return document.getElementById(str) ? uniqueId(prefix) : str;
}

// node_modules/@fluentui/web-components/dist/esm/theme/set-theme.js
var SUPPORTS_ADOPTED_STYLE_SHEETS = "adoptedStyleSheets" in document;
var SUPPORTS_CSS_SCOPE = "CSSScopeRule" in window;
var themeStyleTextMap = /* @__PURE__ */ new Map();
var scopedThemeKeyMap = /* @__PURE__ */ new Map();
var shadowAdoptedStyleSheetMap = /* @__PURE__ */ new Map();
var elementThemeMap = /* @__PURE__ */ new Map();
var globalThemeStyleSheet = new CSSStyleSheet();
function setTheme(theme, node = document) {
  if (!node || !isThemeableNode(node)) {
    return;
  }
  if (!SUPPORTS_ADOPTED_STYLE_SHEETS || node instanceof HTMLElement && !node.shadowRoot && !SUPPORTS_CSS_SCOPE) {
    const target = node === document ? document.documentElement : node;
    setThemePropertiesOnElement(theme, target);
    return;
  }
  if ([document, document.documentElement, document.body].includes(node)) {
    setGlobalTheme(theme);
  } else {
    setLocalTheme(theme, node);
  }
}
var TOKEN_NAME_REGEX = /^[a-zA-Z_-][a-zA-Z0-9_-]*$/;
function sanitizeTokenName(name) {
  return TOKEN_NAME_REGEX.test(name) ? name : "";
}
var TOKEN_VALUE_BLOCK_REGEX = /(;|{|}|\/\*|\*\/|@import|url\s*\(|expression\s*\(|javascript:)/i;
function sanitizeTokenValue(value) {
  return TOKEN_VALUE_BLOCK_REGEX.test(value) ? "" : value;
}
function getThemeStyleText(theme) {
  if (!themeStyleTextMap.has(theme)) {
    themeStyleTextMap.set(theme, Object.keys(theme).reduce((acc, token) => {
      const tokenName = sanitizeTokenName(token);
      const tokenValue = sanitizeTokenValue(theme[token].toString());
      return tokenName && tokenValue ? `${acc}--${tokenName}:${tokenValue};` : acc;
    }, ""));
  }
  return themeStyleTextMap.get(theme);
}
function isThemeableNode(node) {
  return [document, document.documentElement].includes(node) || node instanceof HTMLElement && !!node.closest("body");
}
function setGlobalTheme(theme) {
  if (theme === null) {
    if (document.adoptedStyleSheets.includes(globalThemeStyleSheet)) {
      globalThemeStyleSheet.replaceSync("");
    }
    return;
  }
  globalThemeStyleSheet.replaceSync(`
    html {
      ${getThemeStyleText(theme)}
    }
  `);
  if (!document.adoptedStyleSheets.includes(globalThemeStyleSheet)) {
    document.adoptedStyleSheets.push(globalThemeStyleSheet);
  }
}
function setLocalTheme(theme, element) {
  if (theme === null) {
    if (element.shadowRoot && shadowAdoptedStyleSheetMap.has(element)) {
      shadowAdoptedStyleSheetMap.get(element).replaceSync("");
    } else {
      delete element.dataset.fluentTheme;
      forceRepaint(element);
    }
    return;
  }
  if (element.shadowRoot) {
    getShadowAdoptedStyleSheet(element).replaceSync(`
      :host {
        ${getThemeStyleText(theme)}
      }
    `);
  } else {
    element.dataset.fluentTheme = getScopedThemeKey(theme);
    forceRepaint(element);
  }
}
function getShadowAdoptedStyleSheet(element) {
  if (!shadowAdoptedStyleSheetMap.has(element)) {
    const shadowAdoptedStyleSheet = new CSSStyleSheet();
    shadowAdoptedStyleSheetMap.set(element, shadowAdoptedStyleSheet);
    element.shadowRoot?.adoptedStyleSheets.push(shadowAdoptedStyleSheet);
  }
  return shadowAdoptedStyleSheetMap.get(element);
}
function getScopedThemeKey(theme) {
  if (!scopedThemeKeyMap.has(theme)) {
    const themeKey = uniqueId("fluent-theme-");
    const scopedThemeStyleSheet = new CSSStyleSheet();
    scopedThemeKeyMap.set(theme, themeKey);
    scopedThemeStyleSheet.replaceSync(`
      @scope ([data-fluent-theme="${themeKey}"]) {
        :scope {
          ${getThemeStyleText(theme)}
        }
      }
    `);
    document.adoptedStyleSheets.push(scopedThemeStyleSheet);
  }
  return scopedThemeKeyMap.get(theme);
}
function setThemePropertiesOnElement(theme, element) {
  let tokens;
  if (theme === null) {
    if (!elementThemeMap.has(element)) {
      return;
    }
    tokens = elementThemeMap.get(element);
  } else {
    elementThemeMap.set(element, theme);
    tokens = theme;
  }
  for (const [tokenName, tokenValue] of Object.entries(tokens)) {
    if (theme === null) {
      element.style.removeProperty(`--${tokenName}`);
    } else {
      element.style.setProperty(`--${tokenName}`, tokenValue.toString());
    }
  }
}
var { userAgent: UA } = navigator;
var isWebkit = /\bAppleWebKit\/[\d+\.]+\b/.test(UA);
function forceRepaint(element) {
  if (!isWebkit) {
    return;
  }
  const name = "visibility";
  const tempValue = "hidden";
  const currentValue = element.style.getPropertyValue(name);
  element.style.setProperty(name, tempValue);
  Updates.process();
  element.style.setProperty(name, currentValue);
}

// node_modules/@fluentui/tokens/lib/global/colors.js
var grey = {
  "2": "#050505",
  "4": "#0a0a0a",
  "6": "#0f0f0f",
  "8": "#141414",
  "10": "#1a1a1a",
  "12": "#1f1f1f",
  "14": "#242424",
  "16": "#292929",
  "18": "#2e2e2e",
  "20": "#333333",
  "22": "#383838",
  "24": "#3d3d3d",
  "26": "#424242",
  "28": "#474747",
  "30": "#4d4d4d",
  "32": "#525252",
  "34": "#575757",
  "36": "#5c5c5c",
  "38": "#616161",
  "40": "#666666",
  "42": "#6b6b6b",
  "44": "#707070",
  "46": "#757575",
  "48": "#7a7a7a",
  "50": "#808080",
  "52": "#858585",
  "54": "#8a8a8a",
  "56": "#8f8f8f",
  "58": "#949494",
  "60": "#999999",
  "62": "#9e9e9e",
  "64": "#a3a3a3",
  "66": "#a8a8a8",
  "68": "#adadad",
  "70": "#b3b3b3",
  "72": "#b8b8b8",
  "74": "#bdbdbd",
  "76": "#c2c2c2",
  "78": "#c7c7c7",
  "80": "#cccccc",
  "82": "#d1d1d1",
  "84": "#d6d6d6",
  "86": "#dbdbdb",
  "88": "#e0e0e0",
  "90": "#e6e6e6",
  "92": "#ebebeb",
  "94": "#f0f0f0",
  "96": "#f5f5f5",
  "98": "#fafafa",
  "99": "#fcfcfc"
};
var whiteAlpha = {
  "5": "rgba(255, 255, 255, 0.05)",
  "10": "rgba(255, 255, 255, 0.1)",
  "20": "rgba(255, 255, 255, 0.2)",
  "30": "rgba(255, 255, 255, 0.3)",
  "40": "rgba(255, 255, 255, 0.4)",
  "50": "rgba(255, 255, 255, 0.5)",
  "60": "rgba(255, 255, 255, 0.6)",
  "70": "rgba(255, 255, 255, 0.7)",
  "80": "rgba(255, 255, 255, 0.8)",
  "90": "rgba(255, 255, 255, 0.9)"
};
var blackAlpha = {
  "5": "rgba(0, 0, 0, 0.05)",
  "10": "rgba(0, 0, 0, 0.1)",
  "20": "rgba(0, 0, 0, 0.2)",
  "30": "rgba(0, 0, 0, 0.3)",
  "40": "rgba(0, 0, 0, 0.4)",
  "50": "rgba(0, 0, 0, 0.5)",
  "60": "rgba(0, 0, 0, 0.6)",
  "70": "rgba(0, 0, 0, 0.7)",
  "80": "rgba(0, 0, 0, 0.8)",
  "90": "rgba(0, 0, 0, 0.9)"
};
var grey10Alpha = {
  "5": "rgba(26, 26, 26, 0.05)",
  "10": "rgba(26, 26, 26, 0.1)",
  "20": "rgba(26, 26, 26, 0.2)",
  "30": "rgba(26, 26, 26, 0.3)",
  "40": "rgba(26, 26, 26, 0.4)",
  "50": "rgba(26, 26, 26, 0.5)",
  "60": "rgba(26, 26, 26, 0.6)",
  "70": "rgba(26, 26, 26, 0.7)",
  "80": "rgba(26, 26, 26, 0.8)",
  "90": "rgba(26, 26, 26, 0.9)"
};
var grey12Alpha = {
  "5": "rgba(31, 31, 31, 0.05)",
  "10": "rgba(31, 31, 31, 0.1)",
  "20": "rgba(31, 31, 31, 0.2)",
  "30": "rgba(31, 31, 31, 0.3)",
  "40": "rgba(31, 31, 31, 0.4)",
  "50": "rgba(31, 31, 31, 0.5)",
  "60": "rgba(31, 31, 31, 0.6)",
  "70": "rgba(31, 31, 31, 0.7)",
  "80": "rgba(31, 31, 31, 0.8)",
  "90": "rgba(31, 31, 31, 0.9)"
};
var grey14Alpha = {
  "5": "rgba(36, 36, 36, 0.05)",
  "10": "rgba(36, 36, 36, 0.1)",
  "20": "rgba(36, 36, 36, 0.2)",
  "30": "rgba(36, 36, 36, 0.3)",
  "40": "rgba(36, 36, 36, 0.4)",
  "50": "rgba(36, 36, 36, 0.5)",
  "60": "rgba(36, 36, 36, 0.6)",
  "70": "rgba(36, 36, 36, 0.7)",
  "80": "rgba(36, 36, 36, 0.8)",
  "90": "rgba(36, 36, 36, 0.9)"
};
var white = "#ffffff";
var black = "#000000";
var darkRed = {
  shade50: "#130204",
  shade40: "#230308",
  shade30: "#420610",
  shade20: "#590815",
  shade10: "#690a19",
  primary: "#750b1c",
  tint10: "#861b2c",
  tint20: "#962f3f",
  tint30: "#ac4f5e",
  tint40: "#d69ca5",
  tint50: "#e9c7cd",
  tint60: "#f9f0f2"
};
var cranberry = {
  shade50: "#200205",
  shade40: "#3b0509",
  shade30: "#6e0811",
  shade20: "#960b18",
  shade10: "#b10e1c",
  primary: "#c50f1f",
  tint10: "#cc2635",
  tint20: "#d33f4c",
  tint30: "#dc626d",
  tint40: "#eeacb2",
  tint50: "#f6d1d5",
  tint60: "#fdf3f4"
};
var red = {
  shade50: "#210809",
  shade40: "#3f1011",
  shade30: "#751d1f",
  shade20: "#9f282b",
  shade10: "#bc2f32",
  primary: "#d13438",
  tint10: "#d7494c",
  tint20: "#dc5e62",
  tint30: "#e37d80",
  tint40: "#f1bbbc",
  tint50: "#f8dadb",
  tint60: "#fdf6f6"
};
var darkOrange = {
  shade50: "#230900",
  shade40: "#411200",
  shade30: "#7a2101",
  shade20: "#a62d01",
  shade10: "#c43501",
  primary: "#da3b01",
  tint10: "#de501c",
  tint20: "#e36537",
  tint30: "#e9835e",
  tint40: "#f4bfab",
  tint50: "#f9dcd1",
  tint60: "#fdf6f3"
};
var pumpkin = {
  shade50: "#200d03",
  shade40: "#3d1805",
  shade30: "#712d09",
  shade20: "#9a3d0c",
  shade10: "#b6480e",
  primary: "#ca5010",
  tint10: "#d06228",
  tint20: "#d77440",
  tint30: "#df8e64",
  tint40: "#efc4ad",
  tint50: "#f7dfd2",
  tint60: "#fdf7f4"
};
var orange = {
  shade50: "#271002",
  shade40: "#4a1e04",
  shade30: "#8a3707",
  shade20: "#bc4b09",
  shade10: "#de590b",
  primary: "#f7630c",
  tint10: "#f87528",
  tint20: "#f98845",
  tint30: "#faa06b",
  tint40: "#fdcfb4",
  tint50: "#fee5d7",
  tint60: "#fff9f5"
};
var peach = {
  shade50: "#291600",
  shade40: "#4d2a00",
  shade30: "#8f4e00",
  shade20: "#c26a00",
  shade10: "#e67e00",
  primary: "#ff8c00",
  tint10: "#ff9a1f",
  tint20: "#ffa83d",
  tint30: "#ffba66",
  tint40: "#ffddb3",
  tint50: "#ffedd6",
  tint60: "#fffaf5"
};
var marigold = {
  shade50: "#251a00",
  shade40: "#463100",
  shade30: "#835b00",
  shade20: "#b27c00",
  shade10: "#d39300",
  primary: "#eaa300",
  tint10: "#edad1c",
  tint20: "#efb839",
  tint30: "#f2c661",
  tint40: "#f9e2ae",
  tint50: "#fcefd3",
  tint60: "#fefbf4"
};
var yellow = {
  shade50: "#282400",
  shade40: "#4c4400",
  shade30: "#817400",
  shade20: "#c0ad00",
  shade10: "#e4cc00",
  primary: "#fde300",
  tint10: "#fde61e",
  tint20: "#fdea3d",
  tint30: "#feee66",
  tint40: "#fef7b2",
  tint50: "#fffad6",
  tint60: "#fffef5"
};
var gold = {
  shade50: "#1f1900",
  shade40: "#3a2f00",
  shade30: "#6c5700",
  shade20: "#937700",
  shade10: "#ae8c00",
  primary: "#c19c00",
  tint10: "#c8a718",
  tint20: "#d0b232",
  tint30: "#dac157",
  tint40: "#ecdfa5",
  tint50: "#f5eece",
  tint60: "#fdfbf2"
};
var brass = {
  shade50: "#181202",
  shade40: "#2e2103",
  shade30: "#553e06",
  shade20: "#745408",
  shade10: "#89640a",
  primary: "#986f0b",
  tint10: "#a47d1e",
  tint20: "#b18c34",
  tint30: "#c1a256",
  tint40: "#e0cea2",
  tint50: "#efe4cb",
  tint60: "#fbf8f2"
};
var brown = {
  shade50: "#170e07",
  shade40: "#2b1a0e",
  shade30: "#50301a",
  shade20: "#6c4123",
  shade10: "#804d29",
  primary: "#8e562e",
  tint10: "#9c663f",
  tint20: "#a97652",
  tint30: "#bb8f6f",
  tint40: "#ddc3b0",
  tint50: "#edded3",
  tint60: "#faf7f4"
};
var forest = {
  shade50: "#0c1501",
  shade40: "#162702",
  shade30: "#294903",
  shade20: "#376304",
  shade10: "#427505",
  primary: "#498205",
  tint10: "#599116",
  tint20: "#6ba02b",
  tint30: "#85b44c",
  tint40: "#bdd99b",
  tint50: "#dbebc7",
  tint60: "#f6faf0"
};
var seafoam = {
  shade50: "#002111",
  shade40: "#003d20",
  shade30: "#00723b",
  shade20: "#009b51",
  shade10: "#00b85f",
  primary: "#00cc6a",
  tint10: "#19d279",
  tint20: "#34d889",
  tint30: "#5ae0a0",
  tint40: "#a8f0cd",
  tint50: "#cff7e4",
  tint60: "#f3fdf8"
};
var lightGreen = {
  shade50: "#031a02",
  shade40: "#063004",
  shade30: "#0b5a08",
  shade20: "#0e7a0b",
  shade10: "#11910d",
  primary: "#13a10e",
  tint10: "#27ac22",
  tint20: "#3db838",
  tint30: "#5ec75a",
  tint40: "#a7e3a5",
  tint50: "#cef0cd",
  tint60: "#f2fbf2"
};
var green = {
  shade50: "#031403",
  shade40: "#052505",
  shade30: "#094509",
  shade20: "#0c5e0c",
  shade10: "#0e700e",
  primary: "#107c10",
  tint10: "#218c21",
  tint20: "#359b35",
  tint30: "#54b054",
  tint40: "#9fd89f",
  tint50: "#c9eac9",
  tint60: "#f1faf1"
};
var darkGreen = {
  shade50: "#021102",
  shade40: "#032003",
  shade30: "#063b06",
  shade20: "#085108",
  shade10: "#0a5f0a",
  primary: "#0b6a0b",
  tint10: "#1a7c1a",
  tint20: "#2d8e2d",
  tint30: "#4da64d",
  tint40: "#9ad29a",
  tint50: "#c6e7c6",
  tint60: "#f0f9f0"
};
var lightTeal = {
  shade50: "#001d1f",
  shade40: "#00373a",
  shade30: "#00666d",
  shade20: "#008b94",
  shade10: "#00a5af",
  primary: "#00b7c3",
  tint10: "#18bfca",
  tint20: "#32c8d1",
  tint30: "#58d3db",
  tint40: "#a6e9ed",
  tint50: "#cef3f5",
  tint60: "#f2fcfd"
};
var teal = {
  shade50: "#001516",
  shade40: "#012728",
  shade30: "#02494c",
  shade20: "#026467",
  shade10: "#037679",
  primary: "#038387",
  tint10: "#159195",
  tint20: "#2aa0a4",
  tint30: "#4cb4b7",
  tint40: "#9bd9db",
  tint50: "#c7ebec",
  tint60: "#f0fafa"
};
var steel = {
  shade50: "#000f12",
  shade40: "#001b22",
  shade30: "#00333f",
  shade20: "#004555",
  shade10: "#005265",
  primary: "#005b70",
  tint10: "#0f6c81",
  tint20: "#237d92",
  tint30: "#4496a9",
  tint40: "#94c8d4",
  tint50: "#c3e1e8",
  tint60: "#eff7f9"
};
var blue = {
  shade50: "#001322",
  shade40: "#002440",
  shade30: "#004377",
  shade20: "#005ba1",
  shade10: "#006cbf",
  primary: "#0078d4",
  tint10: "#1a86d9",
  tint20: "#3595de",
  tint30: "#5caae5",
  tint40: "#a9d3f2",
  tint50: "#d0e7f8",
  tint60: "#f3f9fd"
};
var royalBlue = {
  shade50: "#000c16",
  shade40: "#00172a",
  shade30: "#002c4e",
  shade20: "#003b6a",
  shade10: "#00467e",
  primary: "#004e8c",
  tint10: "#125e9a",
  tint20: "#286fa8",
  tint30: "#4a89ba",
  tint40: "#9abfdc",
  tint50: "#c7dced",
  tint60: "#f0f6fa"
};
var cornflower = {
  shade50: "#0d1126",
  shade40: "#182047",
  shade30: "#2c3c85",
  shade20: "#3c51b4",
  shade10: "#4760d5",
  primary: "#4f6bed",
  tint10: "#637cef",
  tint20: "#778df1",
  tint30: "#93a4f4",
  tint40: "#c8d1fa",
  tint50: "#e1e6fc",
  tint60: "#f7f9fe"
};
var navy = {
  shade50: "#00061d",
  shade40: "#000c36",
  shade30: "#001665",
  shade20: "#001e89",
  shade10: "#0023a2",
  primary: "#0027b4",
  tint10: "#173bbd",
  tint20: "#3050c6",
  tint30: "#546fd2",
  tint40: "#a3b2e8",
  tint50: "#ccd5f3",
  tint60: "#f2f4fc"
};
var lavender = {
  shade50: "#120f25",
  shade40: "#221d46",
  shade30: "#3f3682",
  shade20: "#5649b0",
  shade10: "#6656d1",
  primary: "#7160e8",
  tint10: "#8172eb",
  tint20: "#9184ee",
  tint30: "#a79cf1",
  tint40: "#d2ccf8",
  tint50: "#e7e4fb",
  tint60: "#f9f8fe"
};
var purple = {
  shade50: "#0f0717",
  shade40: "#1c0e2b",
  shade30: "#341a51",
  shade20: "#46236e",
  shade10: "#532982",
  primary: "#5c2e91",
  tint10: "#6b3f9e",
  tint20: "#7c52ab",
  tint30: "#9470bd",
  tint40: "#c6b1de",
  tint50: "#e0d3ed",
  tint60: "#f7f4fb"
};
var grape = {
  shade50: "#160418",
  shade40: "#29072e",
  shade30: "#4c0d55",
  shade20: "#671174",
  shade10: "#7a1589",
  primary: "#881798",
  tint10: "#952aa4",
  tint20: "#a33fb1",
  tint30: "#b55fc1",
  tint40: "#d9a7e0",
  tint50: "#eaceef",
  tint60: "#faf2fb"
};
var berry = {
  shade50: "#1f091d",
  shade40: "#3a1136",
  shade30: "#6d2064",
  shade20: "#932b88",
  shade10: "#af33a1",
  primary: "#c239b3",
  tint10: "#c94cbc",
  tint20: "#d161c4",
  tint30: "#da7ed0",
  tint40: "#edbbe7",
  tint50: "#f5daf2",
  tint60: "#fdf5fc"
};
var lilac = {
  shade50: "#1c0b1f",
  shade40: "#35153a",
  shade30: "#63276d",
  shade20: "#863593",
  shade10: "#9f3faf",
  primary: "#b146c2",
  tint10: "#ba58c9",
  tint20: "#c36bd1",
  tint30: "#cf87da",
  tint40: "#e6bfed",
  tint50: "#f2dcf5",
  tint60: "#fcf6fd"
};
var pink = {
  shade50: "#24091b",
  shade40: "#441232",
  shade30: "#80215d",
  shade20: "#ad2d7e",
  shade10: "#cd3595",
  primary: "#e43ba6",
  tint10: "#e750b0",
  tint20: "#ea66ba",
  tint30: "#ef85c8",
  tint40: "#f7c0e3",
  tint50: "#fbddf0",
  tint60: "#fef6fb"
};
var magenta = {
  shade50: "#1f0013",
  shade40: "#390024",
  shade30: "#6b0043",
  shade20: "#91005a",
  shade10: "#ac006b",
  primary: "#bf0077",
  tint10: "#c71885",
  tint20: "#ce3293",
  tint30: "#d957a8",
  tint40: "#eca5d1",
  tint50: "#f5cee6",
  tint60: "#fcf2f9"
};
var plum = {
  shade50: "#13000c",
  shade40: "#240017",
  shade30: "#43002b",
  shade20: "#5a003b",
  shade10: "#6b0045",
  primary: "#77004d",
  tint10: "#87105d",
  tint20: "#98246f",
  tint30: "#ad4589",
  tint40: "#d696c0",
  tint50: "#e9c4dc",
  tint60: "#faf0f6"
};
var beige = {
  shade50: "#141313",
  shade40: "#252323",
  shade30: "#444241",
  shade20: "#5d5958",
  shade10: "#6e6968",
  primary: "#7a7574",
  tint10: "#8a8584",
  tint20: "#9a9594",
  tint30: "#afabaa",
  tint40: "#d7d4d4",
  tint50: "#eae8e8",
  tint60: "#faf9f9"
};
var mink = {
  shade50: "#0f0e0e",
  shade40: "#1c1b1a",
  shade30: "#343231",
  shade20: "#474443",
  shade10: "#54514f",
  primary: "#5d5a58",
  tint10: "#706d6b",
  tint20: "#84817e",
  tint30: "#9e9b99",
  tint40: "#cecccb",
  tint50: "#e5e4e3",
  tint60: "#f8f8f8"
};
var platinum = {
  shade50: "#111314",
  shade40: "#1f2426",
  shade30: "#3b4447",
  shade20: "#505c60",
  shade10: "#5f6d71",
  primary: "#69797e",
  tint10: "#79898d",
  tint20: "#89989d",
  tint30: "#a0adb2",
  tint40: "#cdd6d8",
  tint50: "#e4e9ea",
  tint60: "#f8f9fa"
};
var anchor = {
  shade50: "#090a0b",
  shade40: "#111315",
  shade30: "#202427",
  shade20: "#2b3135",
  shade10: "#333a3f",
  primary: "#394146",
  tint10: "#4d565c",
  tint20: "#626c72",
  tint30: "#808a90",
  tint40: "#bcc3c7",
  tint50: "#dbdfe1",
  tint60: "#f6f7f8"
};

// node_modules/@fluentui/tokens/lib/global/colorPalette.js
var statusSharedColors = {
  red,
  green,
  darkOrange,
  yellow,
  berry,
  lightGreen,
  marigold
};
var personaSharedColors = {
  darkRed,
  cranberry,
  pumpkin,
  peach,
  gold,
  brass,
  brown,
  forest,
  seafoam,
  darkGreen,
  lightTeal,
  teal,
  steel,
  blue,
  royalBlue,
  cornflower,
  navy,
  lavender,
  purple,
  grape,
  lilac,
  pink,
  magenta,
  plum,
  beige,
  mink,
  platinum,
  anchor
};
var mappedStatusColors = {
  cranberry,
  green,
  orange
};

// node_modules/@fluentui/tokens/lib/sharedColorNames.js
var statusSharedColorNames = [
  "red",
  "green",
  "darkOrange",
  "yellow",
  "berry",
  "lightGreen",
  "marigold"
];
var personaSharedColorNames = [
  "darkRed",
  "cranberry",
  "pumpkin",
  "peach",
  "gold",
  "brass",
  "brown",
  "forest",
  "seafoam",
  "darkGreen",
  "lightTeal",
  "teal",
  "steel",
  "blue",
  "royalBlue",
  "cornflower",
  "navy",
  "lavender",
  "purple",
  "grape",
  "lilac",
  "pink",
  "magenta",
  "plum",
  "beige",
  "mink",
  "platinum",
  "anchor"
];

// node_modules/@fluentui/tokens/lib/statusColorMapping.js
var statusColorMapping = {
  success: "green",
  warning: "orange",
  danger: "cranberry"
};

// node_modules/@fluentui/tokens/lib/alias/lightColorPalette.js
var statusColorPaletteTokens = statusSharedColorNames.reduce((acc, sharedColor) => {
  const color = sharedColor.slice(0, 1).toUpperCase() + sharedColor.slice(1);
  const sharedColorTokens = {
    [`colorPalette${color}Background1`]: statusSharedColors[sharedColor].tint60,
    [`colorPalette${color}Background2`]: statusSharedColors[sharedColor].tint40,
    [`colorPalette${color}Background3`]: statusSharedColors[sharedColor].primary,
    [`colorPalette${color}Foreground1`]: statusSharedColors[sharedColor].shade10,
    [`colorPalette${color}Foreground2`]: statusSharedColors[sharedColor].shade30,
    [`colorPalette${color}Foreground3`]: statusSharedColors[sharedColor].primary,
    [`colorPalette${color}BorderActive`]: statusSharedColors[sharedColor].primary,
    [`colorPalette${color}Border1`]: statusSharedColors[sharedColor].tint40,
    [`colorPalette${color}Border2`]: statusSharedColors[sharedColor].primary
  };
  return Object.assign(acc, sharedColorTokens);
}, {});
statusColorPaletteTokens.colorPaletteYellowForeground1 = statusSharedColors.yellow.shade30;
statusColorPaletteTokens.colorPaletteRedForegroundInverted = statusSharedColors.red.tint20;
statusColorPaletteTokens.colorPaletteGreenForegroundInverted = statusSharedColors.green.tint20;
statusColorPaletteTokens.colorPaletteYellowForegroundInverted = statusSharedColors.yellow.tint40;
var personaColorPaletteTokens = personaSharedColorNames.reduce((acc, sharedColor) => {
  const color = sharedColor.slice(0, 1).toUpperCase() + sharedColor.slice(1);
  const sharedColorTokens = {
    [`colorPalette${color}Background2`]: personaSharedColors[sharedColor].tint40,
    [`colorPalette${color}Foreground2`]: personaSharedColors[sharedColor].shade30,
    [`colorPalette${color}BorderActive`]: personaSharedColors[sharedColor].primary
  };
  return Object.assign(acc, sharedColorTokens);
}, {});
var colorPaletteTokens = {
  ...statusColorPaletteTokens,
  ...personaColorPaletteTokens
};
var colorStatusTokens = Object.entries(statusColorMapping).reduce((acc, [statusColor, sharedColor]) => {
  const color = statusColor.slice(0, 1).toUpperCase() + statusColor.slice(1);
  const statusColorTokens = {
    [`colorStatus${color}Background1`]: mappedStatusColors[sharedColor].tint60,
    [`colorStatus${color}Background2`]: mappedStatusColors[sharedColor].tint40,
    [`colorStatus${color}Background3`]: mappedStatusColors[sharedColor].primary,
    [`colorStatus${color}Foreground1`]: mappedStatusColors[sharedColor].shade10,
    [`colorStatus${color}Foreground2`]: mappedStatusColors[sharedColor].shade30,
    [`colorStatus${color}Foreground3`]: mappedStatusColors[sharedColor].primary,
    [`colorStatus${color}ForegroundInverted`]: mappedStatusColors[sharedColor].tint30,
    [`colorStatus${color}BorderActive`]: mappedStatusColors[sharedColor].primary,
    [`colorStatus${color}Border1`]: mappedStatusColors[sharedColor].tint40,
    [`colorStatus${color}Border2`]: mappedStatusColors[sharedColor].primary
  };
  return Object.assign(acc, statusColorTokens);
}, {});
colorStatusTokens.colorStatusDangerBackground3Hover = mappedStatusColors[statusColorMapping.danger].shade10;
colorStatusTokens.colorStatusDangerBackground3Pressed = mappedStatusColors[statusColorMapping.danger].shade20;
colorStatusTokens.colorStatusWarningForeground1 = mappedStatusColors[statusColorMapping.warning].shade20;
colorStatusTokens.colorStatusWarningForeground3 = mappedStatusColors[statusColorMapping.warning].shade20;
colorStatusTokens.colorStatusWarningBorder2 = mappedStatusColors[statusColorMapping.warning].shade20;

// node_modules/@fluentui/tokens/lib/alias/lightColor.js
var generateColorTokens = (brand) => ({
  colorNeutralForeground1: grey[14],
  colorNeutralForeground1Hover: grey[14],
  colorNeutralForeground1Pressed: grey[14],
  colorNeutralForeground1Selected: grey[14],
  colorNeutralForeground2: grey[26],
  colorNeutralForeground2Hover: grey[14],
  colorNeutralForeground2Pressed: grey[14],
  colorNeutralForeground2Selected: grey[14],
  colorNeutralForeground2BrandHover: brand[80],
  colorNeutralForeground2BrandPressed: brand[70],
  colorNeutralForeground2BrandSelected: brand[80],
  colorNeutralForeground3: grey[38],
  colorNeutralForeground3Hover: grey[26],
  colorNeutralForeground3Pressed: grey[26],
  colorNeutralForeground3Selected: grey[26],
  colorNeutralForeground3BrandHover: brand[80],
  colorNeutralForeground3BrandPressed: brand[70],
  colorNeutralForeground3BrandSelected: brand[80],
  colorNeutralForeground4: grey[44],
  colorNeutralForeground5: grey[38],
  colorNeutralForeground5Hover: grey[14],
  colorNeutralForeground5Pressed: grey[14],
  colorNeutralForeground5Selected: grey[14],
  colorNeutralForegroundDisabled: grey[74],
  colorNeutralForegroundInvertedDisabled: whiteAlpha[40],
  colorBrandForegroundLink: brand[70],
  colorBrandForegroundLinkHover: brand[60],
  colorBrandForegroundLinkPressed: brand[40],
  colorBrandForegroundLinkSelected: brand[70],
  colorNeutralForeground2Link: grey[26],
  colorNeutralForeground2LinkHover: grey[14],
  colorNeutralForeground2LinkPressed: grey[14],
  colorNeutralForeground2LinkSelected: grey[14],
  colorCompoundBrandForeground1: brand[80],
  colorCompoundBrandForeground1Hover: brand[70],
  colorCompoundBrandForeground1Pressed: brand[60],
  colorBrandForeground1: brand[80],
  colorBrandForeground2: brand[70],
  colorBrandForeground2Hover: brand[60],
  colorBrandForeground2Pressed: brand[30],
  colorNeutralForeground1Static: grey[14],
  colorNeutralForegroundStaticInverted: white,
  colorNeutralForegroundInverted: white,
  colorNeutralForegroundInvertedHover: white,
  colorNeutralForegroundInvertedPressed: white,
  colorNeutralForegroundInvertedSelected: white,
  colorNeutralForegroundInverted2: white,
  colorNeutralForegroundOnBrand: white,
  colorNeutralForegroundInvertedLink: white,
  colorNeutralForegroundInvertedLinkHover: white,
  colorNeutralForegroundInvertedLinkPressed: white,
  colorNeutralForegroundInvertedLinkSelected: white,
  colorBrandForegroundInverted: brand[100],
  colorBrandForegroundInvertedHover: brand[110],
  colorBrandForegroundInvertedPressed: brand[100],
  colorBrandForegroundOnLight: brand[80],
  colorBrandForegroundOnLightHover: brand[70],
  colorBrandForegroundOnLightPressed: brand[50],
  colorBrandForegroundOnLightSelected: brand[60],
  colorNeutralBackground1: white,
  colorNeutralBackground1Hover: grey[96],
  colorNeutralBackground1Pressed: grey[88],
  colorNeutralBackground1Selected: grey[92],
  colorNeutralBackground2: grey[98],
  colorNeutralBackground2Hover: grey[94],
  colorNeutralBackground2Pressed: grey[86],
  colorNeutralBackground2Selected: grey[90],
  colorNeutralBackground3: grey[96],
  colorNeutralBackground3Hover: grey[92],
  colorNeutralBackground3Pressed: grey[84],
  colorNeutralBackground3Selected: grey[88],
  colorNeutralBackground4: grey[94],
  colorNeutralBackground4Hover: grey[98],
  colorNeutralBackground4Pressed: grey[96],
  colorNeutralBackground4Selected: white,
  colorNeutralBackground5: grey[92],
  colorNeutralBackground5Hover: grey[96],
  colorNeutralBackground5Pressed: grey[94],
  colorNeutralBackground5Selected: grey[98],
  colorNeutralBackground6: grey[90],
  colorNeutralBackground7: "#00000000",
  colorNeutralBackground7Hover: grey[92],
  colorNeutralBackground7Pressed: grey[84],
  colorNeutralBackground7Selected: "#00000000",
  colorNeutralBackground8: grey[99],
  colorNeutralBackgroundInverted: grey[16],
  colorNeutralBackgroundInvertedHover: grey[24],
  colorNeutralBackgroundInvertedPressed: grey[12],
  colorNeutralBackgroundInvertedSelected: grey[22],
  colorNeutralBackgroundStatic: grey[20],
  colorNeutralBackgroundAlpha: whiteAlpha[50],
  colorNeutralBackgroundAlpha2: whiteAlpha[80],
  colorSubtleBackground: "transparent",
  colorSubtleBackgroundHover: grey[96],
  colorSubtleBackgroundPressed: grey[88],
  colorSubtleBackgroundSelected: grey[92],
  colorSubtleBackgroundLightAlphaHover: whiteAlpha[70],
  colorSubtleBackgroundLightAlphaPressed: whiteAlpha[50],
  colorSubtleBackgroundLightAlphaSelected: "transparent",
  colorSubtleBackgroundInverted: "transparent",
  colorSubtleBackgroundInvertedHover: blackAlpha[10],
  colorSubtleBackgroundInvertedPressed: blackAlpha[30],
  colorSubtleBackgroundInvertedSelected: blackAlpha[20],
  colorTransparentBackground: "transparent",
  colorTransparentBackgroundHover: "transparent",
  colorTransparentBackgroundPressed: "transparent",
  colorTransparentBackgroundSelected: "transparent",
  colorNeutralBackgroundDisabled: grey[94],
  colorNeutralBackgroundDisabled2: white,
  colorNeutralBackgroundInvertedDisabled: whiteAlpha[10],
  colorNeutralStencil1: grey[90],
  colorNeutralStencil2: grey[98],
  colorNeutralStencil1Alpha: blackAlpha[10],
  colorNeutralStencil2Alpha: blackAlpha[5],
  colorBackgroundOverlay: blackAlpha[40],
  colorScrollbarOverlay: blackAlpha[50],
  colorBrandBackground: brand[80],
  colorBrandBackgroundHover: brand[70],
  colorBrandBackgroundPressed: brand[40],
  colorBrandBackgroundSelected: brand[60],
  colorCompoundBrandBackground: brand[80],
  colorCompoundBrandBackgroundHover: brand[70],
  colorCompoundBrandBackgroundPressed: brand[60],
  colorBrandBackgroundStatic: brand[80],
  colorBrandBackground2: brand[160],
  colorBrandBackground2Hover: brand[150],
  colorBrandBackground2Pressed: brand[130],
  colorBrandBackground3Static: brand[60],
  colorBrandBackground4Static: brand[40],
  colorBrandBackgroundInverted: white,
  colorBrandBackgroundInvertedHover: brand[160],
  colorBrandBackgroundInvertedPressed: brand[140],
  colorBrandBackgroundInvertedSelected: brand[150],
  colorNeutralCardBackground: grey[98],
  colorNeutralCardBackgroundHover: white,
  colorNeutralCardBackgroundPressed: grey[96],
  colorNeutralCardBackgroundSelected: grey[92],
  colorNeutralCardBackgroundDisabled: grey[94],
  colorNeutralStrokeAccessible: grey[38],
  colorNeutralStrokeAccessibleHover: grey[34],
  colorNeutralStrokeAccessiblePressed: grey[30],
  colorNeutralStrokeAccessibleSelected: brand[80],
  colorNeutralStroke1: grey[82],
  colorNeutralStroke1Hover: grey[78],
  colorNeutralStroke1Pressed: grey[70],
  colorNeutralStroke1Selected: grey[74],
  colorNeutralStroke2: grey[88],
  colorNeutralStroke3: grey[94],
  colorNeutralStroke4: grey[92],
  colorNeutralStroke4Hover: grey[88],
  colorNeutralStroke4Pressed: grey[84],
  colorNeutralStroke4Selected: grey[92],
  colorNeutralStrokeSubtle: grey[88],
  colorNeutralStrokeOnBrand: white,
  colorNeutralStrokeOnBrand2: white,
  colorNeutralStrokeOnBrand2Hover: white,
  colorNeutralStrokeOnBrand2Pressed: white,
  colorNeutralStrokeOnBrand2Selected: white,
  colorBrandStroke1: brand[80],
  colorBrandStroke2: brand[140],
  colorBrandStroke2Hover: brand[120],
  colorBrandStroke2Pressed: brand[80],
  colorBrandStroke2Contrast: brand[140],
  colorCompoundBrandStroke: brand[80],
  colorCompoundBrandStrokeHover: brand[70],
  colorCompoundBrandStrokePressed: brand[60],
  colorNeutralStrokeDisabled: grey[88],
  colorNeutralStrokeDisabled2: grey[92],
  colorNeutralStrokeInvertedDisabled: whiteAlpha[40],
  colorTransparentStroke: "transparent",
  colorTransparentStrokeInteractive: "transparent",
  colorTransparentStrokeDisabled: "transparent",
  colorNeutralStrokeAlpha: blackAlpha[5],
  colorNeutralStrokeAlpha2: whiteAlpha[20],
  colorStrokeFocus1: white,
  colorStrokeFocus2: black,
  colorNeutralShadowAmbient: "rgba(0,0,0,0.12)",
  colorNeutralShadowKey: "rgba(0,0,0,0.14)",
  colorNeutralShadowAmbientLighter: "rgba(0,0,0,0.06)",
  colorNeutralShadowKeyLighter: "rgba(0,0,0,0.07)",
  colorNeutralShadowAmbientDarker: "rgba(0,0,0,0.20)",
  colorNeutralShadowKeyDarker: "rgba(0,0,0,0.24)",
  colorBrandShadowAmbient: "rgba(0,0,0,0.30)",
  colorBrandShadowKey: "rgba(0,0,0,0.25)"
});

// node_modules/@fluentui/tokens/lib/global/borderRadius.js
var borderRadius = {
  borderRadiusNone: "0",
  borderRadiusSmall: "2px",
  borderRadiusMedium: "4px",
  borderRadiusLarge: "6px",
  borderRadiusXLarge: "8px",
  borderRadius2XLarge: "12px",
  borderRadius3XLarge: "16px",
  borderRadius4XLarge: "24px",
  borderRadius5XLarge: "32px",
  borderRadius6XLarge: "40px",
  borderRadiusCircular: "10000px"
};

// node_modules/@fluentui/tokens/lib/global/curves.js
var curves = {
  curveAccelerateMax: "cubic-bezier(0.9,0.1,1,0.2)",
  curveAccelerateMid: "cubic-bezier(1,0,1,1)",
  curveAccelerateMin: "cubic-bezier(0.8,0,0.78,1)",
  curveDecelerateMax: "cubic-bezier(0.1,0.9,0.2,1)",
  curveDecelerateMid: "cubic-bezier(0,0,0,1)",
  curveDecelerateMin: "cubic-bezier(0.33,0,0.1,1)",
  curveEasyEaseMax: "cubic-bezier(0.8,0,0.2,1)",
  curveEasyEase: "cubic-bezier(0.33,0,0.67,1)",
  curveLinear: "cubic-bezier(0,0,1,1)"
};

// node_modules/@fluentui/tokens/lib/global/durations.js
var durations = {
  durationUltraFast: "50ms",
  durationFaster: "100ms",
  durationFast: "150ms",
  durationNormal: "200ms",
  durationGentle: "250ms",
  durationSlow: "300ms",
  durationSlower: "400ms",
  durationUltraSlow: "500ms"
};

// node_modules/@fluentui/tokens/lib/global/fonts.js
var fontSizes = {
  fontSizeBase100: "10px",
  fontSizeBase200: "12px",
  fontSizeBase300: "14px",
  fontSizeBase400: "16px",
  fontSizeBase500: "20px",
  fontSizeBase600: "24px",
  fontSizeHero700: "28px",
  fontSizeHero800: "32px",
  fontSizeHero900: "40px",
  fontSizeHero1000: "68px"
};
var lineHeights = {
  lineHeightBase100: "14px",
  lineHeightBase200: "16px",
  lineHeightBase300: "20px",
  lineHeightBase400: "22px",
  lineHeightBase500: "28px",
  lineHeightBase600: "32px",
  lineHeightHero700: "36px",
  lineHeightHero800: "40px",
  lineHeightHero900: "52px",
  lineHeightHero1000: "92px"
};
var fontWeights = {
  fontWeightRegular: 400,
  fontWeightMedium: 500,
  fontWeightSemibold: 600,
  fontWeightBold: 700
};
var fontFamilies = {
  fontFamilyBase: "'Segoe UI', 'Segoe UI Web (West European)', -apple-system, BlinkMacSystemFont, Roboto, 'Helvetica Neue', sans-serif",
  fontFamilyMonospace: "Consolas, 'Courier New', Courier, monospace",
  fontFamilyNumeric: "Bahnschrift, 'Segoe UI', 'Segoe UI Web (West European)', -apple-system, BlinkMacSystemFont, Roboto, 'Helvetica Neue', sans-serif"
};

// node_modules/@fluentui/tokens/lib/global/spacings.js
var spacings = {
  none: "0",
  xxs: "2px",
  xs: "4px",
  sNudge: "6px",
  s: "8px",
  mNudge: "10px",
  m: "12px",
  l: "16px",
  xl: "20px",
  xxl: "24px",
  xxxl: "32px"
};
var horizontalSpacings = {
  spacingHorizontalNone: spacings.none,
  spacingHorizontalXXS: spacings.xxs,
  spacingHorizontalXS: spacings.xs,
  spacingHorizontalSNudge: spacings.sNudge,
  spacingHorizontalS: spacings.s,
  spacingHorizontalMNudge: spacings.mNudge,
  spacingHorizontalM: spacings.m,
  spacingHorizontalL: spacings.l,
  spacingHorizontalXL: spacings.xl,
  spacingHorizontalXXL: spacings.xxl,
  spacingHorizontalXXXL: spacings.xxxl
};
var verticalSpacings = {
  spacingVerticalNone: spacings.none,
  spacingVerticalXXS: spacings.xxs,
  spacingVerticalXS: spacings.xs,
  spacingVerticalSNudge: spacings.sNudge,
  spacingVerticalS: spacings.s,
  spacingVerticalMNudge: spacings.mNudge,
  spacingVerticalM: spacings.m,
  spacingVerticalL: spacings.l,
  spacingVerticalXL: spacings.xl,
  spacingVerticalXXL: spacings.xxl,
  spacingVerticalXXXL: spacings.xxxl
};

// node_modules/@fluentui/tokens/lib/global/strokeWidths.js
var strokeWidths = {
  strokeWidthThin: "1px",
  strokeWidthThick: "2px",
  strokeWidthThicker: "3px",
  strokeWidthThickest: "4px"
};

// node_modules/@fluentui/tokens/lib/utils/shadows.js
function createShadowTokens(ambientColor, keyColor, tokenSuffix = "") {
  return {
    [`shadow2${tokenSuffix}`]: `0 0 2px ${ambientColor}, 0 1px 2px ${keyColor}`,
    [`shadow4${tokenSuffix}`]: `0 0 2px ${ambientColor}, 0 2px 4px ${keyColor}`,
    [`shadow8${tokenSuffix}`]: `0 0 2px ${ambientColor}, 0 4px 8px ${keyColor}`,
    [`shadow16${tokenSuffix}`]: `0 0 2px ${ambientColor}, 0 8px 16px ${keyColor}`,
    [`shadow28${tokenSuffix}`]: `0 0 8px ${ambientColor}, 0 14px 28px ${keyColor}`,
    [`shadow64${tokenSuffix}`]: `0 0 8px ${ambientColor}, 0 32px 64px ${keyColor}`
  };
}

// node_modules/@fluentui/tokens/lib/utils/createLightTheme.js
var createLightTheme = (brand) => {
  const colorTokens = generateColorTokens(brand);
  return {
    ...borderRadius,
    ...fontSizes,
    ...lineHeights,
    ...fontFamilies,
    ...fontWeights,
    ...strokeWidths,
    ...horizontalSpacings,
    ...verticalSpacings,
    ...durations,
    ...curves,
    ...colorTokens,
    ...colorPaletteTokens,
    ...colorStatusTokens,
    ...createShadowTokens(colorTokens.colorNeutralShadowAmbient, colorTokens.colorNeutralShadowKey),
    ...createShadowTokens(colorTokens.colorBrandShadowAmbient, colorTokens.colorBrandShadowKey, "Brand")
  };
};

// node_modules/@fluentui/tokens/lib/global/brandColors.js
var brandWeb = {
  10: `#061724`,
  20: `#082338`,
  30: `#0a2e4a`,
  40: `#0c3b5e`,
  50: `#0e4775`,
  60: `#0f548c`,
  70: `#115ea3`,
  80: `#0f6cbd`,
  90: `#2886de`,
  100: `#479ef5`,
  110: `#62abf5`,
  120: `#77b7f7`,
  130: `#96c6fa`,
  140: `#b4d6fa`,
  150: `#cfe4fa`,
  160: `#ebf3fc`
};

// node_modules/@fluentui/tokens/lib/alias/darkColorPalette.js
var statusColorPaletteTokens2 = statusSharedColorNames.reduce((acc, sharedColor) => {
  const color = sharedColor.slice(0, 1).toUpperCase() + sharedColor.slice(1);
  const sharedColorTokens = {
    [`colorPalette${color}Background1`]: statusSharedColors[sharedColor].shade40,
    [`colorPalette${color}Background2`]: statusSharedColors[sharedColor].shade30,
    [`colorPalette${color}Background3`]: statusSharedColors[sharedColor].primary,
    [`colorPalette${color}Foreground1`]: statusSharedColors[sharedColor].tint30,
    [`colorPalette${color}Foreground2`]: statusSharedColors[sharedColor].tint40,
    [`colorPalette${color}Foreground3`]: statusSharedColors[sharedColor].tint20,
    [`colorPalette${color}BorderActive`]: statusSharedColors[sharedColor].tint30,
    [`colorPalette${color}Border1`]: statusSharedColors[sharedColor].primary,
    [`colorPalette${color}Border2`]: statusSharedColors[sharedColor].tint20
  };
  return Object.assign(acc, sharedColorTokens);
}, {});
statusColorPaletteTokens2.colorPaletteRedForeground3 = statusSharedColors.red.tint30;
statusColorPaletteTokens2.colorPaletteRedBorder2 = statusSharedColors.red.tint30;
statusColorPaletteTokens2.colorPaletteGreenForeground3 = statusSharedColors.green.tint40;
statusColorPaletteTokens2.colorPaletteGreenBorder2 = statusSharedColors.green.tint40;
statusColorPaletteTokens2.colorPaletteDarkOrangeForeground3 = statusSharedColors.darkOrange.tint30;
statusColorPaletteTokens2.colorPaletteDarkOrangeBorder2 = statusSharedColors.darkOrange.tint30;
statusColorPaletteTokens2.colorPaletteRedForegroundInverted = statusSharedColors.red.primary;
statusColorPaletteTokens2.colorPaletteGreenForegroundInverted = statusSharedColors.green.primary;
statusColorPaletteTokens2.colorPaletteYellowForegroundInverted = statusSharedColors.yellow.shade30;
var personaColorPaletteTokens2 = personaSharedColorNames.reduce((acc, sharedColor) => {
  const color = sharedColor.slice(0, 1).toUpperCase() + sharedColor.slice(1);
  const sharedColorTokens = {
    [`colorPalette${color}Background2`]: personaSharedColors[sharedColor].shade30,
    [`colorPalette${color}Foreground2`]: personaSharedColors[sharedColor].tint40,
    [`colorPalette${color}BorderActive`]: personaSharedColors[sharedColor].tint30
  };
  return Object.assign(acc, sharedColorTokens);
}, {});
personaColorPaletteTokens2.colorPaletteDarkRedBackground2 = personaSharedColors.darkRed.shade20;
personaColorPaletteTokens2.colorPalettePlumBackground2 = personaSharedColors.plum.shade20;
var colorPaletteTokens2 = {
  ...statusColorPaletteTokens2,
  ...personaColorPaletteTokens2
};
var colorStatusTokens2 = Object.entries(statusColorMapping).reduce((acc, [statusColor, sharedColor]) => {
  const color = statusColor.slice(0, 1).toUpperCase() + statusColor.slice(1);
  const statusColorTokens = {
    [`colorStatus${color}Background1`]: mappedStatusColors[sharedColor].shade40,
    [`colorStatus${color}Background2`]: mappedStatusColors[sharedColor].shade30,
    [`colorStatus${color}Background3`]: mappedStatusColors[sharedColor].primary,
    [`colorStatus${color}Foreground1`]: mappedStatusColors[sharedColor].tint30,
    [`colorStatus${color}Foreground2`]: mappedStatusColors[sharedColor].tint40,
    [`colorStatus${color}Foreground3`]: mappedStatusColors[sharedColor].tint20,
    [`colorStatus${color}BorderActive`]: mappedStatusColors[sharedColor].tint30,
    [`colorStatus${color}ForegroundInverted`]: mappedStatusColors[sharedColor].shade10,
    [`colorStatus${color}Border1`]: mappedStatusColors[sharedColor].primary,
    [`colorStatus${color}Border2`]: mappedStatusColors[sharedColor].tint20
  };
  return Object.assign(acc, statusColorTokens);
}, {});
colorStatusTokens2.colorStatusDangerBackground3Hover = mappedStatusColors[statusColorMapping.danger].shade10;
colorStatusTokens2.colorStatusDangerBackground3Pressed = mappedStatusColors[statusColorMapping.danger].shade20;
colorStatusTokens2.colorStatusDangerForeground3 = mappedStatusColors[statusColorMapping.danger].tint40;
colorStatusTokens2.colorStatusDangerBorder2 = mappedStatusColors[statusColorMapping.danger].tint30;
colorStatusTokens2.colorStatusSuccessForeground3 = mappedStatusColors[statusColorMapping.success].tint40;
colorStatusTokens2.colorStatusSuccessBorder2 = mappedStatusColors[statusColorMapping.success].tint40;
colorStatusTokens2.colorStatusWarningForegroundInverted = mappedStatusColors[statusColorMapping.warning].shade20;

// node_modules/@fluentui/tokens/lib/themes/web/lightTheme.js
var webLightTheme = createLightTheme(brandWeb);

// node_modules/@fluentui/tokens/lib/alias/darkColor.js
var generateColorTokens2 = (brand) => ({
  colorNeutralForeground1: white,
  colorNeutralForeground1Hover: white,
  colorNeutralForeground1Pressed: white,
  colorNeutralForeground1Selected: white,
  colorNeutralForeground2: grey[84],
  colorNeutralForeground2Hover: white,
  colorNeutralForeground2Pressed: white,
  colorNeutralForeground2Selected: white,
  colorNeutralForeground2BrandHover: brand[100],
  colorNeutralForeground2BrandPressed: brand[90],
  colorNeutralForeground2BrandSelected: brand[100],
  colorNeutralForeground3: grey[68],
  colorNeutralForeground3Hover: grey[84],
  colorNeutralForeground3Pressed: grey[84],
  colorNeutralForeground3Selected: grey[84],
  colorNeutralForeground3BrandHover: brand[100],
  colorNeutralForeground3BrandPressed: brand[90],
  colorNeutralForeground3BrandSelected: brand[100],
  colorNeutralForeground4: grey[60],
  colorNeutralForeground5: grey[68],
  colorNeutralForeground5Hover: white,
  colorNeutralForeground5Pressed: white,
  colorNeutralForeground5Selected: white,
  colorNeutralForegroundDisabled: grey[36],
  colorNeutralForegroundInvertedDisabled: whiteAlpha[40],
  colorBrandForegroundLink: brand[100],
  colorBrandForegroundLinkHover: brand[110],
  colorBrandForegroundLinkPressed: brand[90],
  colorBrandForegroundLinkSelected: brand[100],
  colorNeutralForeground2Link: grey[84],
  colorNeutralForeground2LinkHover: white,
  colorNeutralForeground2LinkPressed: white,
  colorNeutralForeground2LinkSelected: white,
  colorCompoundBrandForeground1: brand[100],
  colorCompoundBrandForeground1Hover: brand[110],
  colorCompoundBrandForeground1Pressed: brand[90],
  colorBrandForeground1: brand[100],
  colorBrandForeground2: brand[110],
  colorBrandForeground2Hover: brand[130],
  colorBrandForeground2Pressed: brand[160],
  colorNeutralForeground1Static: grey[14],
  colorNeutralForegroundStaticInverted: white,
  colorNeutralForegroundInverted: grey[14],
  colorNeutralForegroundInvertedHover: grey[14],
  colorNeutralForegroundInvertedPressed: grey[14],
  colorNeutralForegroundInvertedSelected: grey[14],
  colorNeutralForegroundInverted2: grey[14],
  colorNeutralForegroundOnBrand: white,
  colorNeutralForegroundInvertedLink: white,
  colorNeutralForegroundInvertedLinkHover: white,
  colorNeutralForegroundInvertedLinkPressed: white,
  colorNeutralForegroundInvertedLinkSelected: white,
  colorBrandForegroundInverted: brand[80],
  colorBrandForegroundInvertedHover: brand[70],
  colorBrandForegroundInvertedPressed: brand[60],
  colorBrandForegroundOnLight: brand[80],
  colorBrandForegroundOnLightHover: brand[70],
  colorBrandForegroundOnLightPressed: brand[50],
  colorBrandForegroundOnLightSelected: brand[60],
  colorNeutralBackground1: grey[16],
  colorNeutralBackground1Hover: grey[24],
  colorNeutralBackground1Pressed: grey[12],
  colorNeutralBackground1Selected: grey[22],
  colorNeutralBackground2: grey[12],
  colorNeutralBackground2Hover: grey[20],
  colorNeutralBackground2Pressed: grey[8],
  colorNeutralBackground2Selected: grey[18],
  colorNeutralBackground3: grey[8],
  colorNeutralBackground3Hover: grey[16],
  colorNeutralBackground3Pressed: grey[4],
  colorNeutralBackground3Selected: grey[14],
  colorNeutralBackground4: grey[4],
  colorNeutralBackground4Hover: grey[12],
  colorNeutralBackground4Pressed: black,
  colorNeutralBackground4Selected: grey[10],
  colorNeutralBackground5: black,
  colorNeutralBackground5Hover: grey[8],
  colorNeutralBackground5Pressed: grey[2],
  colorNeutralBackground5Selected: grey[6],
  colorNeutralBackground6: grey[20],
  colorNeutralBackground7: "#00000000",
  colorNeutralBackground7Hover: grey[10],
  colorNeutralBackground7Pressed: grey[4],
  colorNeutralBackground7Selected: "#00000000",
  colorNeutralBackground8: grey[16],
  colorNeutralBackgroundInverted: white,
  colorNeutralBackgroundInvertedHover: grey[96],
  colorNeutralBackgroundInvertedPressed: grey[88],
  colorNeutralBackgroundInvertedSelected: grey[92],
  colorNeutralBackgroundStatic: grey[24],
  colorNeutralBackgroundAlpha: grey10Alpha[50],
  colorNeutralBackgroundAlpha2: grey12Alpha[70],
  colorSubtleBackground: "transparent",
  colorSubtleBackgroundHover: grey[22],
  colorSubtleBackgroundPressed: grey[18],
  colorSubtleBackgroundSelected: grey[20],
  colorSubtleBackgroundLightAlphaHover: grey14Alpha[80],
  colorSubtleBackgroundLightAlphaPressed: grey14Alpha[50],
  colorSubtleBackgroundLightAlphaSelected: "transparent",
  colorSubtleBackgroundInverted: "transparent",
  colorSubtleBackgroundInvertedHover: blackAlpha[10],
  colorSubtleBackgroundInvertedPressed: blackAlpha[30],
  colorSubtleBackgroundInvertedSelected: blackAlpha[20],
  colorTransparentBackground: "transparent",
  colorTransparentBackgroundHover: "transparent",
  colorTransparentBackgroundPressed: "transparent",
  colorTransparentBackgroundSelected: "transparent",
  colorNeutralBackgroundDisabled: grey[8],
  colorNeutralBackgroundDisabled2: grey[16],
  colorNeutralBackgroundInvertedDisabled: whiteAlpha[10],
  colorNeutralStencil1: grey[34],
  colorNeutralStencil2: grey[20],
  colorNeutralStencil1Alpha: whiteAlpha[10],
  colorNeutralStencil2Alpha: whiteAlpha[5],
  colorBackgroundOverlay: blackAlpha[50],
  colorScrollbarOverlay: whiteAlpha[60],
  colorBrandBackground: brand[70],
  colorBrandBackgroundHover: brand[80],
  colorBrandBackgroundPressed: brand[40],
  colorBrandBackgroundSelected: brand[60],
  colorCompoundBrandBackground: brand[100],
  colorCompoundBrandBackgroundHover: brand[110],
  colorCompoundBrandBackgroundPressed: brand[90],
  colorBrandBackgroundStatic: brand[80],
  colorBrandBackground2: brand[20],
  colorBrandBackground2Hover: brand[40],
  colorBrandBackground2Pressed: brand[10],
  colorBrandBackground3Static: brand[60],
  colorBrandBackground4Static: brand[40],
  colorBrandBackgroundInverted: white,
  colorBrandBackgroundInvertedHover: brand[160],
  colorBrandBackgroundInvertedPressed: brand[140],
  colorBrandBackgroundInvertedSelected: brand[150],
  colorNeutralCardBackground: grey[20],
  colorNeutralCardBackgroundHover: grey[24],
  colorNeutralCardBackgroundPressed: grey[18],
  colorNeutralCardBackgroundSelected: grey[22],
  colorNeutralCardBackgroundDisabled: grey[8],
  colorNeutralStrokeAccessible: grey[68],
  colorNeutralStrokeAccessibleHover: grey[74],
  colorNeutralStrokeAccessiblePressed: grey[70],
  colorNeutralStrokeAccessibleSelected: brand[100],
  colorNeutralStroke1: grey[40],
  colorNeutralStroke1Hover: grey[46],
  colorNeutralStroke1Pressed: grey[42],
  colorNeutralStroke1Selected: grey[44],
  colorNeutralStroke2: grey[32],
  colorNeutralStroke3: grey[24],
  colorNeutralStroke4: grey[24],
  colorNeutralStroke4Hover: grey[18],
  colorNeutralStroke4Pressed: grey[14],
  colorNeutralStroke4Selected: grey[24],
  colorNeutralStrokeSubtle: grey[4],
  colorNeutralStrokeOnBrand: grey[16],
  colorNeutralStrokeOnBrand2: white,
  colorNeutralStrokeOnBrand2Hover: white,
  colorNeutralStrokeOnBrand2Pressed: white,
  colorNeutralStrokeOnBrand2Selected: white,
  colorBrandStroke1: brand[100],
  colorBrandStroke2: brand[50],
  colorBrandStroke2Hover: brand[50],
  colorBrandStroke2Pressed: brand[30],
  colorBrandStroke2Contrast: brand[50],
  colorCompoundBrandStroke: brand[100],
  colorCompoundBrandStrokeHover: brand[110],
  colorCompoundBrandStrokePressed: brand[90],
  colorNeutralStrokeDisabled: grey[26],
  colorNeutralStrokeDisabled2: grey[24],
  colorNeutralStrokeInvertedDisabled: whiteAlpha[40],
  colorTransparentStroke: "transparent",
  colorTransparentStrokeInteractive: "transparent",
  colorTransparentStrokeDisabled: "transparent",
  colorNeutralStrokeAlpha: whiteAlpha[10],
  colorNeutralStrokeAlpha2: whiteAlpha[20],
  colorStrokeFocus1: black,
  colorStrokeFocus2: white,
  colorNeutralShadowAmbient: "rgba(0,0,0,0.24)",
  colorNeutralShadowKey: "rgba(0,0,0,0.28)",
  colorNeutralShadowAmbientLighter: "rgba(0,0,0,0.12)",
  colorNeutralShadowKeyLighter: "rgba(0,0,0,0.14)",
  colorNeutralShadowAmbientDarker: "rgba(0,0,0,0.40)",
  colorNeutralShadowKeyDarker: "rgba(0,0,0,0.48)",
  colorBrandShadowAmbient: "rgba(0,0,0,0.30)",
  colorBrandShadowKey: "rgba(0,0,0,0.25)"
});

// node_modules/@fluentui/tokens/lib/utils/createDarkTheme.js
var createDarkTheme = (brand) => {
  const colorTokens = generateColorTokens2(brand);
  return {
    ...borderRadius,
    ...fontSizes,
    ...lineHeights,
    ...fontFamilies,
    ...fontWeights,
    ...strokeWidths,
    ...horizontalSpacings,
    ...verticalSpacings,
    ...durations,
    ...curves,
    ...colorTokens,
    ...colorPaletteTokens2,
    ...colorStatusTokens2,
    ...createShadowTokens(colorTokens.colorNeutralShadowAmbient, colorTokens.colorNeutralShadowKey),
    ...createShadowTokens(colorTokens.colorBrandShadowAmbient, colorTokens.colorBrandShadowKey, "Brand")
  };
};

// node_modules/@fluentui/tokens/lib/themes/web/darkTheme.js
var webDarkTheme = createDarkTheme(brandWeb);

// frontend/comfyui/ui/surface/appearance/AccentPalette.ts
function normalizeAccentColor(value) {
  if (typeof value !== "string") return null;
  const color = value.trim().toLowerCase();
  if (/^#[\da-f]{6}$/.test(color)) return color;
  if (/^#[\da-f]{3}$/.test(color))
    return `#${Array.from(color.slice(1), (character) => character + character).join("")}`;
  return null;
}
function createAccentBrandPalette(color) {
  return {
    10: mixAccentColor(color, "#000000", 0.94),
    20: mixAccentColor(color, "#000000", 0.86),
    30: mixAccentColor(color, "#000000", 0.75),
    40: mixAccentColor(color, "#000000", 0.62),
    50: mixAccentColor(color, "#000000", 0.48),
    60: mixAccentColor(color, "#000000", 0.32),
    70: mixAccentColor(color, "#000000", 0.16),
    80: color,
    90: mixAccentColor(color, "#ffffff", 0.1),
    100: mixAccentColor(color, "#ffffff", 0.2),
    110: mixAccentColor(color, "#ffffff", 0.32),
    120: mixAccentColor(color, "#ffffff", 0.44),
    130: mixAccentColor(color, "#ffffff", 0.56),
    140: mixAccentColor(color, "#ffffff", 0.68),
    150: mixAccentColor(color, "#ffffff", 0.8),
    160: mixAccentColor(color, "#ffffff", 0.92)
  };
}
function accentForeground(color) {
  const luminance = relativeLuminance(color);
  return (luminance + 0.05) / 0.05 >= 1.05 / (luminance + 0.05) ? "#000000" : "#ffffff";
}
function accentIndicatorColor(color, dark) {
  const surface = relativeLuminance(dark ? "#2b2b2b" : "#ffffff");
  for (let step = 0; step <= 20; step += 1) {
    const candidate = mixAccentColor(color, dark ? "#ffffff" : "#000000", step / 20);
    const luminance = relativeLuminance(candidate);
    if ((Math.max(surface, luminance) + 0.05) / (Math.min(surface, luminance) + 0.05) >= 4.5)
      return candidate;
  }
  return dark ? "#ffffff" : "#000000";
}
function mixAccentColor(color, destination, weight) {
  const [red2, green2, blue2] = readChannels(color);
  const [targetRed, targetGreen, targetBlue] = readChannels(destination);
  const pairs = [
    [red2, targetRed],
    [green2, targetGreen],
    [blue2, targetBlue]
  ];
  return `#${pairs.map(
    ([channel, target]) => Math.round(channel * (1 - weight) + target * weight).toString(16).padStart(2, "0")
  ).join("")}`;
}
function linearChannel(channel) {
  const normalized = channel / 255;
  return normalized <= 0.04045 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
}
function relativeLuminance(color) {
  const [red2, green2, blue2] = readChannels(color);
  return linearChannel(red2) * 0.2126 + linearChannel(green2) * 0.7152 + linearChannel(blue2) * 0.0722;
}
function readChannels(color) {
  const normalized = normalizeAccentColor(color);
  if (!normalized) throw new TypeError("Accent colors must be opaque hexadecimal colors.");
  return [
    Number.parseInt(normalized.slice(1, 3), 16),
    Number.parseInt(normalized.slice(3, 5), 16),
    Number.parseInt(normalized.slice(5, 7), 16)
  ];
}

// node_modules/tslib/tslib.es6.mjs
function __decorate(decorators, target, key, desc) {
  var c = arguments.length, r = c < 3 ? target : desc === null ? desc = Object.getOwnPropertyDescriptor(target, key) : desc, d;
  if (typeof Reflect === "object" && typeof Reflect.decorate === "function") r = Reflect.decorate(decorators, target, key, desc);
  else for (var i = decorators.length - 1; i >= 0; i--) if (d = decorators[i]) r = (c < 3 ? d(r) : c > 3 ? d(target, key, r) : d(target, key)) || r;
  return c > 3 && r && Object.defineProperty(target, key, r), r;
}

// node_modules/@fluentui/web-components/dist/esm/utils/direction.js
var Direction = {
  ltr: "ltr",
  rtl: "rtl"
};
var getDirection = (rootNode) => {
  return rootNode.closest("[dir]")?.dir === "rtl" ? Direction.rtl : Direction.ltr;
};

// node_modules/@fluentui/web-components/dist/esm/utils/numbers.js
function limit(min, max, value) {
  return Math.min(Math.max(value, min), max);
}

// node_modules/@fluentui/web-components/dist/esm/utils/orientation.js
var Orientation = {
  horizontal: "horizontal",
  vertical: "vertical"
};

// node_modules/@fluentui/web-components/dist/esm/utils/converters.js
var numberLikeStringConverter = {
  fromView(value) {
    const valueAsNumber = parseFloat(value);
    return Number.isNaN(valueAsNumber) ? "" : valueAsNumber.toString();
  },
  toView(value) {
    const valueAsNumber = parseFloat(value);
    return Number.isNaN(valueAsNumber) ? void 0 : valueAsNumber.toString();
  }
};

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

// node_modules/@fluentui/web-components/dist/esm/slider/slider-utilities.js
function convertPixelToPercent(pixelPos, minPosition, maxPosition, direction) {
  let pct = limit(0, 1, (pixelPos - minPosition) / (maxPosition - minPosition));
  if (direction === Direction.rtl) {
    pct = 1 - pct;
  }
  return pct;
}

// node_modules/@fluentui/web-components/dist/esm/fluent-design-system.js
var FluentDesignSystem = Object.freeze({
  prefix: "fluent",
  shadowRootMode: "open",
  registry: globalThis.customElements
});

// node_modules/@fluentui/web-components/dist/esm/slider/slider.options.js
var SliderOrientation = Orientation;
var SliderMode = {
  singleValue: "single-value"
};
var tagName = `${FluentDesignSystem.prefix}-slider`;

// node_modules/@fluentui/web-components/dist/esm/slider/slider.js
var Slider = class extends FASTElement {
  static {
    this.formAssociated = true;
  }
  /**
   * A reference to all associated `<label>` elements.
   *
   * @public
   */
  get labels() {
    return Object.freeze(Array.from(this.elementInternals.labels));
  }
  handleChange(_, propertyName) {
    switch (propertyName) {
      case "min":
      case "max":
        this.setSliderPosition();
      case "step":
        this.handleStepStyles();
        break;
    }
  }
  /**
   * Handles changes to step styling based on the step value
   * NOTE: This function is not a changed callback, stepStyles is not observable
   */
  handleStepStyles() {
    if (this.step) {
      const totalSteps = 100 / Math.floor((this.maxAsNumber - this.minAsNumber) / this.stepAsNumber);
      if (this.stepStyles !== void 0) {
        this.$fastController.removeStyles(this.stepStyles);
      }
      this.stepStyles = css`
        :host {
          --step-rate: ${totalSteps}%;
        }
      `;
      this.$fastController.addStyles(this.stepStyles);
    } else if (this.stepStyles !== void 0) {
      this.$fastController.removeStyles(this.stepStyles);
    }
  }
  /**
   * Sets the value of the input when the value attribute changes.
   *
   * @param prev - The previous value
   * @param next - The current value
   * @internal
   */
  initialValueChanged(_, next2) {
    if (this.$fastController.isConnected) {
      this.value = next2;
    } else {
      this._value = next2;
    }
  }
  /**
   * The element's validity state.
   *
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/docs/Web/API/ElementInternals/validity | `ElementInternals.validity`} property.
   */
  get validity() {
    return this.elementInternals.validity;
  }
  /**
   * The element's validation message.
   *
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/en-US/docs/Web/API/ElementInternals/validationMessage | `ElemenentInternals.validationMessage`} property.
   */
  get validationMessage() {
    return this.elementInternals.validationMessage;
  }
  /**
   * Whether the element is a candidate for its owning form's constraint validation.
   *
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/en-US/docs/Web/API/ElementInternals/willValidate | `ElemenentInternals.willValidate`} property.
   */
  get willValidate() {
    return this.elementInternals.willValidate;
  }
  /**
   * Checks the element's validity.
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/en-US/docs/Web/API/ElementInternals/checkValidity | `ElemenentInternals.checkValidity`} method.
   */
  checkValidity() {
    return this.elementInternals.checkValidity();
  }
  /**
   * Reports the element's validity.
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/en-US/docs/Web/API/ElementInternals/reportValidity | `ElemenentInternals.reportValidity`} method.
   */
  reportValidity() {
    return this.elementInternals.reportValidity();
  }
  /**
   * Sets a custom validity message.
   * @public
   */
  setCustomValidity(message) {
    this.setValidity({ customError: !!message }, message);
  }
  /**
   * Sets the validity of the control.
   *
   * @param flags - Validity flags to set.
   * @param message - Optional message to supply. If not provided, the control's `validationMessage` will be used.
   * @param anchor - Optional anchor to use for the validation message.
   *
   * @internal
   */
  setValidity(flags, message, anchor2) {
    if (this.$fastController.isConnected) {
      if (this.isDisabled) {
        this.elementInternals.setValidity({});
        return;
      }
      this.elementInternals.setValidity({ customError: !!message, ...flags }, message ?? this.validationMessage, anchor2);
    }
  }
  /**
   * The current value of the input.
   *
   * @public
   */
  get value() {
    Observable.track(this, "value");
    return this._value?.toString() ?? "";
  }
  set value(value) {
    if (!this.$fastController.isConnected) {
      this._value = value.toString();
      return;
    }
    const nextAsNumber = parseFloat(value);
    const newValue = limit(this.minAsNumber, this.maxAsNumber, this.convertToConstrainedValue(nextAsNumber)).toString();
    if (newValue !== value) {
      this.value = newValue;
      return;
    }
    this._value = value.toString();
    this.elementInternals.ariaValueNow = this._value;
    this.elementInternals.ariaValueText = this.valueTextFormatter(this._value);
    this.setSliderPosition();
    this.$emit("change");
    this.setFormValue(value);
    Observable.notify(this, "value");
  }
  /**
   * Resets the form value to its initial value when the form is reset.
   *
   * @internal
   */
  formResetCallback() {
    this.value = this.initialValue ?? this.midpoint;
  }
  /**
   * Disabled the component when its associated form is disabled.
   *
   * @internal
   *
   * @privateRemarks
   * DO NOT change the `disabled` property or attribute here, because if the
   * `disabled` attribute is present, reenabling an ancestor `<fieldset>`
   * element will not reenabling this component.
   */
  formDisabledCallback(disabled) {
    this.setDisabledSideEffect(disabled);
  }
  /**
   * Reflects the {@link https://developer.mozilla.org/docs/Web/API/ElementInternals/setFormValue | `ElementInternals.setFormValue()`} method.
   *
   * @internal
   */
  setFormValue(value, state2) {
    this.elementInternals.setFormValue(value, value ?? state2);
  }
  directionChanged() {
    this.setSliderPosition();
  }
  /**
   * The value property, typed as a number.
   *
   * @public
   */
  get valueAsNumber() {
    return parseFloat(this.value);
  }
  set valueAsNumber(next2) {
    this.value = next2.toString();
  }
  valueTextFormatterChanged() {
    if (typeof this.valueTextFormatter === "function") {
      this.elementInternals.ariaValueText = this.valueTextFormatter(this._value);
    } else {
      this.elementInternals.ariaValueText = "";
    }
  }
  disabledChanged() {
    this.setDisabledSideEffect(this.disabled);
  }
  /**
   * Returns true if the component is disabled, taking into account the `disabled`
   * attribute, `aria-disabled` attribute, and the `:disabled` pseudo-class.
   *
   * @internal
   */
  get isDisabled() {
    return this.disabled || this.elementInternals?.ariaDisabled === "true" || this.isConnected && this.matches(":disabled");
  }
  minChanged() {
    this.elementInternals.ariaValueMin = `${this.minAsNumber}`;
    if (this.$fastController.isConnected && this.minAsNumber > this.valueAsNumber) {
      this.value = this.min;
    }
  }
  /**
   * Returns the min property or the default value
   *
   * @internal
   */
  get minAsNumber() {
    if (this.min !== void 0) {
      const parsed = parseFloat(this.min);
      if (!Number.isNaN(parsed)) {
        return parsed;
      }
    }
    return 0;
  }
  maxChanged() {
    this.elementInternals.ariaValueMax = `${this.maxAsNumber}`;
    if (this.$fastController.isConnected && this.maxAsNumber < this.valueAsNumber) {
      this.value = this.max;
    }
  }
  /**
   * Returns the max property or the default value
   *
   * @internal
   */
  get maxAsNumber() {
    if (this.max !== void 0) {
      const parsed = parseFloat(this.max);
      if (!Number.isNaN(parsed)) {
        return parsed;
      }
    }
    return 100;
  }
  stepChanged() {
    this.updateStepMultiplier();
    if (this.$fastController.isConnected) {
      this.value = this._value;
    }
  }
  /**
   * Returns the step property as a number.
   *
   * @internal
   */
  get stepAsNumber() {
    if (this.step !== void 0) {
      const parsed = parseFloat(this.step);
      if (!Number.isNaN(parsed) && parsed > 0) {
        return parsed;
      }
    }
    return 1;
  }
  orientationChanged(prev, next2) {
    this.elementInternals.ariaOrientation = next2 ?? Orientation.horizontal;
    if (this.$fastController.isConnected) {
      this.setSliderPosition();
    }
  }
  constructor() {
    super();
    this.elementInternals = this.attachInternals();
    this.direction = Direction.ltr;
    this.isDragging = false;
    this.trackWidth = 0;
    this.trackMinWidth = 0;
    this.trackHeight = 0;
    this.trackLeft = 0;
    this.trackMinHeight = 0;
    this.valueTextFormatter = () => "";
    this.disabled = false;
    this.min = "";
    this.max = "";
    this.step = "";
    this.mode = SliderMode.singleValue;
    this.setupTrackConstraints = () => {
      const clientRect = this.track.getBoundingClientRect();
      this.trackWidth = this.track.clientWidth;
      this.trackMinWidth = this.track.clientLeft;
      this.trackHeight = clientRect.top;
      this.trackMinHeight = clientRect.bottom;
      this.trackLeft = this.getBoundingClientRect().left;
      if (this.trackWidth === 0) {
        this.trackWidth = 1;
      }
    };
    this.handleThumbPointerDown = (event) => {
      if (this.isDisabled) {
        return true;
      }
      const windowFn = event !== null ? window.addEventListener : window.removeEventListener;
      windowFn("pointerup", this.handleWindowPointerUp);
      windowFn("pointermove", this.handlePointerMove, { passive: true });
      windowFn("touchmove", this.handlePointerMove, { passive: true });
      windowFn("touchend", this.handleWindowPointerUp);
      this.isDragging = event !== null;
      return true;
    };
    this.handlePointerMove = (event) => {
      if (this.isDisabled || event.defaultPrevented) {
        return;
      }
      const sourceEvent = window.TouchEvent && event instanceof TouchEvent ? event.touches[0] : event;
      const thumbWidth = this.thumb.getBoundingClientRect().width;
      const eventValue = this.orientation === Orientation.vertical ? sourceEvent.pageY - document.documentElement.scrollTop : sourceEvent.pageX - document.documentElement.scrollLeft - this.trackLeft - thumbWidth / 2;
      this.value = `${this.calculateNewValue(eventValue)}`;
    };
    this.handleWindowPointerUp = () => {
      this.stopDragging();
    };
    this.stopDragging = () => {
      this.isDragging = false;
      this.handlePointerDown(null);
      this.handleThumbPointerDown(null);
    };
    this.handlePointerDown = (event) => {
      if (event === null || !this.isDisabled) {
        const windowFn = event !== null ? window.addEventListener : window.removeEventListener;
        const documentFn = event !== null ? document.addEventListener : document.removeEventListener;
        windowFn("pointerup", this.handleWindowPointerUp);
        documentFn("mouseleave", this.handleWindowPointerUp);
        windowFn("pointermove", this.handlePointerMove);
        const thumbWidth = this.thumb.getBoundingClientRect().width;
        if (event) {
          this.setupTrackConstraints();
          const controlValue = this.orientation === Orientation.vertical ? event.pageY - document.documentElement.scrollTop : event.pageX - document.documentElement.scrollLeft - this.trackLeft - thumbWidth / 2;
          this.value = `${this.calculateNewValue(controlValue)}`;
        }
      }
      return true;
    };
    this.elementInternals.role = "slider";
    this.elementInternals.ariaOrientation = this.orientation ?? SliderOrientation.horizontal;
  }
  connectedCallback() {
    super.connectedCallback();
    requestAnimationFrame(() => {
      if (!this.$fastController.isConnected) {
        return;
      }
      this.direction = getDirection(this);
      this.setDisabledSideEffect(this.disabled);
      this.updateStepMultiplier();
      this.setupTrackConstraints();
      this.setupDefaultValue();
      this.setSliderPosition();
      this.handleStepStyles();
      const notifier = Observable.getNotifier(this);
      notifier.subscribe(this, "max");
      notifier.subscribe(this, "min");
      notifier.subscribe(this, "step");
    });
    maybeSetAutoFocus(this);
  }
  /**
   * @internal
   */
  disconnectedCallback() {
    super.disconnectedCallback();
    const notifier = Observable.getNotifier(this);
    notifier.unsubscribe(this, "max");
    notifier.unsubscribe(this, "min");
    notifier.unsubscribe(this, "step");
  }
  /**
   * Increment the value by the step
   *
   * @public
   */
  increment() {
    const newVal = this.direction !== Direction.rtl ? Number(this.value) + this.stepAsNumber : Number(this.value) - this.stepAsNumber;
    const incrementedVal = this.convertToConstrainedValue(newVal);
    const incrementedValString = incrementedVal < this.maxAsNumber ? `${incrementedVal}` : `${this.maxAsNumber}`;
    this.value = incrementedValString;
  }
  /**
   * Decrement the value by the step
   *
   * @public
   */
  decrement() {
    const newVal = this.direction !== Direction.rtl ? Number(this.value) - Number(this.stepAsNumber) : Number(this.value) + Number(this.stepAsNumber);
    const decrementedVal = this.convertToConstrainedValue(newVal);
    const decrementedValString = decrementedVal > this.minAsNumber ? `${decrementedVal}` : `${this.minAsNumber}`;
    this.value = decrementedValString;
  }
  handleKeydown(event) {
    if (this.isDisabled) {
      return true;
    }
    switch (event.key) {
      case "Home":
        event.preventDefault();
        this.value = this.direction !== Direction.rtl && this.orientation !== Orientation.vertical ? `${this.minAsNumber}` : `${this.maxAsNumber}`;
        break;
      case "End":
        event.preventDefault();
        this.value = this.direction !== Direction.rtl && this.orientation !== Orientation.vertical ? `${this.maxAsNumber}` : `${this.minAsNumber}`;
        break;
      case "ArrowRight":
      case "ArrowUp":
        if (!event.shiftKey) {
          event.preventDefault();
          this.increment();
        }
        break;
      case "ArrowLeft":
      case "ArrowDown":
        if (!event.shiftKey) {
          event.preventDefault();
          this.decrement();
        }
        break;
    }
    return true;
  }
  /**
   * Places the thumb based on the current value
   */
  setSliderPosition() {
    const newPct = convertPixelToPercent(parseFloat(this.value), this.minAsNumber, this.maxAsNumber, this.orientation === Orientation.vertical ? void 0 : this.direction);
    const percentage = newPct * 100;
    this.position = `--slider-thumb: ${percentage}%; --slider-progress: ${percentage}%`;
  }
  /**
   * Update the step multiplier used to ensure rounding errors from steps that
   * are not whole numbers
   */
  updateStepMultiplier() {
    const stepString = this.stepAsNumber + "";
    const decimalPlacesOfStep = !!(this.stepAsNumber % 1) ? stepString.length - stepString.indexOf(".") - 1 : 0;
    this.stepMultiplier = Math.pow(10, decimalPlacesOfStep);
  }
  get midpoint() {
    return `${this.convertToConstrainedValue((this.maxAsNumber + this.minAsNumber) / 2)}`;
  }
  setupDefaultValue() {
    if (!this._value) {
      this.value = this.initialValue ?? this.midpoint;
    }
    if (!Number.isNaN(this.valueAsNumber) && (this.valueAsNumber < this.minAsNumber || this.valueAsNumber > this.maxAsNumber)) {
      this.value = this.midpoint;
    }
    this.elementInternals.ariaValueNow = this.value;
  }
  /**
   * Calculate the new value based on the given raw pixel value.
   *
   * @param rawValue - the value to be converted to a constrained value
   * @returns the constrained value
   *
   * @internal
   */
  calculateNewValue(rawValue) {
    this.setupTrackConstraints();
    const newPosition = convertPixelToPercent(rawValue, this.orientation === Orientation.vertical ? this.trackMinHeight : this.trackMinWidth, this.orientation === Orientation.vertical ? this.trackHeight : this.trackWidth, this.orientation === Orientation.vertical ? void 0 : this.direction);
    const newValue = (this.maxAsNumber - this.minAsNumber) * newPosition + this.minAsNumber;
    return this.convertToConstrainedValue(newValue);
  }
  convertToConstrainedValue(value) {
    if (isNaN(value)) {
      value = this.minAsNumber;
    }
    let constrainedValue = value - this.minAsNumber;
    const roundedConstrainedValue = Math.round(constrainedValue / this.stepAsNumber);
    const remainderValue = constrainedValue - roundedConstrainedValue * (this.stepMultiplier * this.stepAsNumber) / this.stepMultiplier;
    constrainedValue = remainderValue >= Number(this.stepAsNumber) / 2 ? constrainedValue - remainderValue + Number(this.stepAsNumber) : constrainedValue - remainderValue;
    return constrainedValue + this.minAsNumber;
  }
  /**
   * Makes sure the side effects of set up when the disabled state changes.
   */
  setDisabledSideEffect(disabled = this.isDisabled) {
    Updates.enqueue(() => {
      this.elementInternals.ariaDisabled = disabled.toString();
      this.tabIndex = disabled ? -1 : 0;
    });
  }
};
__decorate([
  attr
], Slider.prototype, "size", void 0);
__decorate([
  attr({ attribute: "value", mode: "fromView" })
], Slider.prototype, "initialValue", void 0);
__decorate([
  observable
], Slider.prototype, "direction", void 0);
__decorate([
  observable
], Slider.prototype, "isDragging", void 0);
__decorate([
  observable
], Slider.prototype, "position", void 0);
__decorate([
  observable
], Slider.prototype, "trackWidth", void 0);
__decorate([
  observable
], Slider.prototype, "trackMinWidth", void 0);
__decorate([
  observable
], Slider.prototype, "trackHeight", void 0);
__decorate([
  observable
], Slider.prototype, "trackLeft", void 0);
__decorate([
  observable
], Slider.prototype, "trackMinHeight", void 0);
__decorate([
  observable
], Slider.prototype, "valueTextFormatter", void 0);
__decorate([
  attr({ mode: "boolean" })
], Slider.prototype, "disabled", void 0);
__decorate([
  attr({ converter: numberLikeStringConverter })
], Slider.prototype, "min", void 0);
__decorate([
  attr({ converter: numberLikeStringConverter })
], Slider.prototype, "max", void 0);
__decorate([
  attr({ converter: numberLikeStringConverter })
], Slider.prototype, "step", void 0);
__decorate([
  attr
], Slider.prototype, "orientation", void 0);
__decorate([
  attr
], Slider.prototype, "mode", void 0);

// node_modules/@fluentui/web-components/dist/esm/utils/display.js
var hidden = `:host([hidden]){display:none}`;
function display(displayValue) {
  return `${hidden}:host{display:${displayValue}}`;
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
var matchingStateMap = /* @__PURE__ */ new WeakMap();
function hasMatchingState(States, state2) {
  if (!States || !state2) {
    return false;
  }
  if (matchingStateMap.has(States)) {
    return matchingStateMap.get(States).has(state2);
  }
  const stateSet = new Set(Object.values(States));
  matchingStateMap.set(States, stateSet);
  return stateSet.has(state2);
}
function swapStates(elementInternals, prev = "", next2 = "", States, prefix = "") {
  toggleState(elementInternals, `${prefix}${prev}`, false);
  if (!States || hasMatchingState(States, next2)) {
    toggleState(elementInternals, `${prefix}${next2}`, true);
  }
}

// node_modules/@fluentui/web-components/dist/esm/styles/states/index.js
var activeState = stateSelector("active");
var badInputState = stateSelector("bad-input");
var checkedState = stateSelector("checked");
var customErrorState = stateSelector("custom-error");
var descriptionState = stateSelector("description");
var disabledState = stateSelector("disabled");
var nativeDisabledState = ":is([disabled], :disabled)";
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

// node_modules/@fluentui/web-components/dist/esm/theme/design-tokens.js
var colorNeutralForeground1 = "var(--colorNeutralForeground1)";
var colorNeutralForeground1Hover = "var(--colorNeutralForeground1Hover)";
var colorNeutralForeground1Pressed = "var(--colorNeutralForeground1Pressed)";
var colorNeutralForeground2 = "var(--colorNeutralForeground2)";
var colorNeutralForeground2Hover = "var(--colorNeutralForeground2Hover)";
var colorNeutralForeground2Pressed = "var(--colorNeutralForeground2Pressed)";
var colorNeutralForeground2BrandHover = "var(--colorNeutralForeground2BrandHover)";
var colorNeutralForeground2BrandPressed = "var(--colorNeutralForeground2BrandPressed)";
var colorNeutralForeground3 = "var(--colorNeutralForeground3)";
var colorNeutralForeground4 = "var(--colorNeutralForeground4)";
var colorNeutralForegroundDisabled = "var(--colorNeutralForegroundDisabled)";
var colorCompoundBrandForeground1Hover = "var(--colorCompoundBrandForeground1Hover)";
var colorNeutralForegroundOnBrand = "var(--colorNeutralForegroundOnBrand)";
var colorNeutralForegroundInverted = "var(--colorNeutralForegroundInverted)";
var colorNeutralBackground1 = "var(--colorNeutralBackground1)";
var colorNeutralBackground1Hover = "var(--colorNeutralBackground1Hover)";
var colorNeutralBackground1Pressed = "var(--colorNeutralBackground1Pressed)";
var colorNeutralBackground3 = "var(--colorNeutralBackground3)";
var colorNeutralBackgroundInverted = "var(--colorNeutralBackgroundInverted)";
var colorSubtleBackground = "var(--colorSubtleBackground)";
var colorSubtleBackgroundHover = "var(--colorSubtleBackgroundHover)";
var colorSubtleBackgroundPressed = "var(--colorSubtleBackgroundPressed)";
var colorTransparentBackground = "var(--colorTransparentBackground)";
var colorTransparentBackgroundHover = "var(--colorTransparentBackgroundHover)";
var colorTransparentBackgroundPressed = "var(--colorTransparentBackgroundPressed)";
var colorNeutralBackgroundDisabled = "var(--colorNeutralBackgroundDisabled)";
var colorBrandBackground = "var(--colorBrandBackground)";
var colorBrandBackgroundHover = "var(--colorBrandBackgroundHover)";
var colorBrandBackgroundPressed = "var(--colorBrandBackgroundPressed)";
var colorCompoundBrandBackground = "var(--colorCompoundBrandBackground)";
var colorCompoundBrandBackgroundHover = "var(--colorCompoundBrandBackgroundHover)";
var colorCompoundBrandBackgroundPressed = "var(--colorCompoundBrandBackgroundPressed)";
var colorNeutralStrokeAccessible = "var(--colorNeutralStrokeAccessible)";
var colorNeutralStrokeAccessibleHover = "var(--colorNeutralStrokeAccessibleHover)";
var colorNeutralStrokeAccessiblePressed = "var(--colorNeutralStrokeAccessiblePressed)";
var colorNeutralStroke1 = "var(--colorNeutralStroke1)";
var colorNeutralStroke1Hover = "var(--colorNeutralStroke1Hover)";
var colorNeutralStroke1Pressed = "var(--colorNeutralStroke1Pressed)";
var colorCompoundBrandStroke = "var(--colorCompoundBrandStroke)";
var colorCompoundBrandStrokeHover = "var(--colorCompoundBrandStrokeHover)";
var colorCompoundBrandStrokePressed = "var(--colorCompoundBrandStrokePressed)";
var colorNeutralStrokeDisabled = "var(--colorNeutralStrokeDisabled)";
var colorTransparentStroke = "var(--colorTransparentStroke)";
var colorTransparentStrokeInteractive = "var(--colorTransparentStrokeInteractive)";
var colorStrokeFocus1 = "var(--colorStrokeFocus1)";
var colorStrokeFocus2 = "var(--colorStrokeFocus2)";
var colorPaletteRedBorder2 = "var(--colorPaletteRedBorder2)";
var borderRadiusNone = "var(--borderRadiusNone)";
var borderRadiusSmall = "var(--borderRadiusSmall)";
var borderRadiusMedium = "var(--borderRadiusMedium)";
var borderRadiusLarge = "var(--borderRadiusLarge)";
var borderRadiusCircular = "var(--borderRadiusCircular)";
var fontFamilyBase = "var(--fontFamilyBase)";
var fontSizeBase200 = "var(--fontSizeBase200)";
var fontSizeBase300 = "var(--fontSizeBase300)";
var fontSizeBase400 = "var(--fontSizeBase400)";
var fontSizeBase500 = "var(--fontSizeBase500)";
var fontSizeBase600 = "var(--fontSizeBase600)";
var fontWeightRegular = "var(--fontWeightRegular)";
var fontWeightSemibold = "var(--fontWeightSemibold)";
var lineHeightBase200 = "var(--lineHeightBase200)";
var lineHeightBase300 = "var(--lineHeightBase300)";
var lineHeightBase400 = "var(--lineHeightBase400)";
var shadow2 = "var(--shadow2)";
var shadow4 = "var(--shadow4)";
var strokeWidthThin = "var(--strokeWidthThin)";
var strokeWidthThick = "var(--strokeWidthThick)";
var strokeWidthThicker = "var(--strokeWidthThicker)";
var spacingHorizontalXXS = "var(--spacingHorizontalXXS)";
var spacingHorizontalXS = "var(--spacingHorizontalXS)";
var spacingHorizontalSNudge = "var(--spacingHorizontalSNudge)";
var spacingHorizontalS = "var(--spacingHorizontalS)";
var spacingHorizontalMNudge = "var(--spacingHorizontalMNudge)";
var spacingHorizontalM = "var(--spacingHorizontalM)";
var spacingHorizontalL = "var(--spacingHorizontalL)";
var spacingVerticalXXS = "var(--spacingVerticalXXS)";
var spacingVerticalXS = "var(--spacingVerticalXS)";
var spacingVerticalSNudge = "var(--spacingVerticalSNudge)";
var spacingVerticalS = "var(--spacingVerticalS)";
var spacingVerticalMNudge = "var(--spacingVerticalMNudge)";
var spacingVerticalL = "var(--spacingVerticalL)";
var durationUltraFast = "var(--durationUltraFast)";
var durationFaster = "var(--durationFaster)";
var durationNormal = "var(--durationNormal)";
var durationSlow = "var(--durationSlow)";
var curveAccelerateMid = "var(--curveAccelerateMid)";
var curveDecelerateMax = "var(--curveDecelerateMax)";
var curveDecelerateMid = "var(--curveDecelerateMid)";
var curveEasyEase = "var(--curveEasyEase)";

// node_modules/@fluentui/web-components/dist/esm/slider/slider.styles.js
var styles = css`
  ${display("inline-grid")}

  :host {
    --thumb-size: 20px;
    --track-margin-inline: calc(var(--thumb-size) / 2);
    --track-size: 4px;
    --track-overhang: calc(var(--track-size) / -2);
    --rail-color: ${colorCompoundBrandBackground};
    --track-color: ${colorNeutralStrokeAccessible};
    --slider-direction: 90deg;
    --border-radius: ${borderRadiusMedium};
    --step-marker-inset: var(--track-overhang) -1px;

    position: relative;
    align-items: center;
    justify-content: center;
    box-sizing: border-box;
    outline: none;
    user-select: none;
    touch-action: none;
    min-width: 120px;
    min-height: 32px;
    grid-template-rows: 1fr var(--thumb-size) 1fr;
    grid-template-columns: var(--track-margin-inline) 1fr var(--track-margin-inline);
  }

  :host(:hover) {
    --rail-color: ${colorCompoundBrandBackgroundHover};
  }

  :host(:active) {
    --rail-color: ${colorCompoundBrandBackgroundPressed};
  }

  :host(${nativeDisabledState}) {
    --rail-color: ${colorNeutralForegroundDisabled};
    --track-color: ${colorNeutralBackgroundDisabled};
  }

  :host(:not(${nativeDisabledState})) {
    cursor: pointer;
  }

  :host(:dir(rtl)) {
    --slider-direction: -90deg;
  }

  :host([size='small']) {
    --thumb-size: 16px;
    --track-overhang: -1px;
    --track-size: 2px;
    --border-radius: ${borderRadiusSmall};
  }

  :host([orientation='vertical']) {
    --slider-direction: 0deg;
    --step-marker-inset: -1px var(--track-overhang);
    min-height: 120px;
    grid-template-rows: var(--track-margin-inline) 1fr var(--track-margin-inline);
    grid-template-columns: 1fr var(--thumb-size) 1fr;
    width: unset;
    min-width: 32px;
    justify-items: center;
  }

  :host(:not([slot='input']):focus-visible) {
    box-shadow: 0 0 0 2pt ${colorStrokeFocus2};
    outline: 1px solid ${colorStrokeFocus1};
  }

  :host:after,
  .track {
    height: var(--track-size);
    width: 100%;
  }

  :host:after {
    background-image: linear-gradient(
      var(--slider-direction),
      var(--rail-color) 0%,
      var(--rail-color) 50%,
      var(--track-color) 50.1%,
      var(--track-color) 100%
    );
    border-radius: var(--border-radius);
    content: '';
    grid-row: 1 / -1;
    grid-column: 1 / -1;
  }

  .track {
    position: relative;
    background-color: var(--track-color);
    grid-row: 2 / 2;
    grid-column: 2 / 2;
    forced-color-adjust: none;
    overflow: hidden;
  }

  :host([orientation='vertical'])::after,
  :host([orientation='vertical']) .track {
    height: 100%;
    width: var(--track-size);
  }

  .track::before {
    content: '';
    position: absolute;
    height: 100%;
    border-radius: inherit;
    inset-inline-start: 0;
    width: var(--slider-progress);
  }

  :host(:dir(rtl)) .track::before {
    width: calc(100% - var(--slider-progress));
  }

  :host([orientation='vertical']) .track::before {
    width: 100%;
    bottom: 0;
    height: var(--slider-progress);
  }

  :host([step]) .track::after {
    content: '';
    position: absolute;
    border-radius: inherit;
    inset: var(--step-marker-inset);
    background-image: repeating-linear-gradient(
      var(--slider-direction),
      #0000 0%,
      #0000 calc(var(--step-rate) - 1px),
      ${colorNeutralBackground1} calc(var(--step-rate) - 1px),
      ${colorNeutralBackground1} var(--step-rate)
    );
  }

  .thumb-container {
    position: absolute;
    grid-row: 2 / 2;
    grid-column: 2 / 2;
    transform: translateX(-50%);
    left: var(--slider-thumb);
  }

  :host([orientation='vertical']) .thumb-container {
    transform: translateY(50%);
    left: unset;
    bottom: var(--slider-thumb);
  }

  :host(:not(:active)) :is(.thumb-container, .track::before) {
    transition: all 0.2s ease;
  }

  .thumb {
    width: var(--thumb-size);
    height: var(--thumb-size);
    border-radius: ${borderRadiusCircular};
    box-shadow: 0 0 0 calc(var(--thumb-size) * 0.2) ${colorNeutralBackground1} inset;
    border: calc(var(--thumb-size) * 0.05) solid ${colorNeutralStroke1};
    box-sizing: border-box;
  }

  .thumb,
  .track::before {
    background-color: var(--rail-color);
  }

  @media (forced-colors: active) {
    .track:hover,
    .track:active,
    .track {
      background: WindowText;
    }
    .thumb:hover,
    .thumb:active,
    .thumb {
      background: ButtonText;
    }

    :host(:hover) .track::before,
    :host(:active) .track::before,
    .track::before {
      background: Highlight;
    }
  }
`;

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

// node_modules/@fluentui/web-components/dist/esm/slider/slider.template.js
function sliderTemplate(options = {}) {
  return html`
    <template
      @pointerdown="${(x, c) => x.handlePointerDown(c.event)}"
      @keydown="${(x, c) => x.handleKeydown(c.event)}"
    >
      <div ${ref("track")} part="track-container" class="track" style="${(x) => x.position}"></div>
      <div
        ${ref("thumb")}
        part="thumb-container"
        class="thumb-container"
        style="${(x) => x.position}"
        @pointerdown="${(x, c) => x.handleThumbPointerDown(c.event)}"
      >
        <slot name="thumb">${staticallyCompose(options.thumb)}</slot>
      </div>
    </template>
  `;
}
var template = sliderTemplate({
  thumb: `<div class="thumb"></div>`
});

// node_modules/@fluentui/web-components/dist/esm/slider/slider.definition.js
var definition = {
  name: tagName,
  registry: FluentDesignSystem.registry,
  styles,
  template
};

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

// node_modules/@fluentui/web-components/dist/esm/button/button.options.js
var ButtonType = {
  submit: "submit",
  reset: "reset",
  button: "button"
};
var tagName2 = `${FluentDesignSystem.prefix}-button`;

// node_modules/@fluentui/web-components/dist/esm/button/button.base.js
var BaseButton = class extends FASTElement {
  /**
   * Handles changes to the disabled attribute. If the button is disabled, it
   * should not be focusable.
   *
   * @param previous - the previous disabled value
   * @param next - the new disabled value
   * @internal
   */
  disabledChanged() {
    this.setTabIndex();
  }
  /**
   * Sets the element's internal disabled state when the element is focusable while disabled.
   *
   * @param previous - the previous disabledFocusable value
   * @param next - the current disabledFocusable value
   * @internal
   */
  disabledFocusableChanged(previous, next2) {
    if (this.elementInternals) {
      this.elementInternals.ariaDisabled = `${!!next2}`;
    }
  }
  /**
   * The associated form element.
   *
   * @public
   */
  get form() {
    return this.elementInternals.form;
  }
  static {
    this.formAssociated = true;
  }
  /**
   * A reference to all associated label elements.
   *
   * @public
   */
  get labels() {
    return Object.freeze(Array.from(this.elementInternals.labels));
  }
  /**
   * Removes the form submission fallback control when the type changes.
   *
   * @param previous - the previous type value
   * @param next - the new type value
   * @internal
   */
  typeChanged(previous, next2) {
    if (next2 !== ButtonType.submit) {
      this.formSubmissionFallbackControl?.remove();
      this.shadowRoot?.querySelector('slot[name="internal"]')?.remove();
    }
  }
  /**
   * Handles the button click event.
   *
   * @param e - The event object
   * @internal
   */
  clickHandler(e) {
    if (e && this.disabledFocusable) {
      e.stopImmediatePropagation();
      return;
    }
    this.press();
    return true;
  }
  connectedCallback() {
    super.connectedCallback();
    this.elementInternals.ariaDisabled = `${!!this.disabledFocusable}`;
    this.setTabIndex();
    maybeSetAutoFocus(this);
  }
  constructor() {
    super();
    this.disabledFocusable = false;
    this.elementInternals = this.attachInternals();
    this.elementInternals.role = "button";
  }
  /**
   * This fallback creates a new slot, then creates a submit button to mirror the custom element's
   * properties. The submit button is then appended to the slot and the form is submitted.
   *
   * @internal
   * @privateRemarks
   * This is a workaround until {@link https://github.com/WICG/webcomponents/issues/814 | WICG/webcomponents/issues/814} is resolved.
   */
  createAndInsertFormSubmissionFallbackControl() {
    const internalSlot = this.formSubmissionFallbackControlSlot ?? document.createElement("slot");
    internalSlot.setAttribute("name", "internal");
    this.shadowRoot?.appendChild(internalSlot);
    this.formSubmissionFallbackControlSlot = internalSlot;
    const fallbackControl = this.formSubmissionFallbackControl ?? document.createElement("button");
    fallbackControl.style.display = "none";
    fallbackControl.setAttribute("type", "submit");
    fallbackControl.setAttribute("slot", "internal");
    if (this.formNoValidate) {
      fallbackControl.toggleAttribute("formnovalidate", true);
    }
    if (this.elementInternals.form?.id) {
      fallbackControl.setAttribute("form", this.elementInternals.form.id);
    }
    if (this.name) {
      fallbackControl.setAttribute("name", this.name);
    }
    if (this.value) {
      fallbackControl.setAttribute("value", this.value);
    }
    if (this.formAction) {
      fallbackControl.setAttribute("formaction", this.formAction ?? "");
    }
    if (this.formEnctype) {
      fallbackControl.setAttribute("formenctype", this.formEnctype ?? "");
    }
    if (this.formMethod) {
      fallbackControl.setAttribute("formmethod", this.formMethod ?? "");
    }
    if (this.formTarget) {
      fallbackControl.setAttribute("formtarget", this.formTarget ?? "");
    }
    this.append(fallbackControl);
    this.formSubmissionFallbackControl = fallbackControl;
  }
  /**
   * Invoked when a connected component's form or fieldset has its disabled state changed.
   *
   * @param disabled - the disabled value of the form / fieldset
   *
   * @internal
   */
  formDisabledCallback(disabled) {
    this.disabled = disabled;
  }
  /**
   * Handles keypress events for the button.
   *
   * @param e - the keyboard event
   * @returns - the return value of the click handler
   * @public
   */
  keypressHandler(e) {
    if (e && this.disabledFocusable) {
      e.stopImmediatePropagation();
      return;
    }
    if (e.key === "Enter" || e.key === " ") {
      this.click();
      return;
    }
    return true;
  }
  /**
   * Presses the button.
   *
   * @public
   */
  press() {
    switch (this.type) {
      case ButtonType.reset: {
        this.resetForm();
        break;
      }
      case ButtonType.submit: {
        this.submitForm();
        break;
      }
    }
  }
  /**
   * Resets the associated form.
   *
   * @public
   */
  resetForm() {
    this.elementInternals.form?.reset();
  }
  /**
   * Sets the `tabindex` attribute based on the disabled state of the button.
   *
   * @internal
   */
  setTabIndex() {
    if (this.disabled) {
      this.removeAttribute("tabindex");
      return;
    }
    this.tabIndex = Number(this.getAttribute("tabindex") ?? 0) < 0 ? -1 : 0;
  }
  /**
   * Submits the associated form.
   *
   * @internal
   */
  submitForm() {
    if (!this.elementInternals.form || this.disabled || this.type !== ButtonType.submit) {
      return;
    }
    if (!this.name && !this.formAction && !this.formEnctype && !this.formAttribute && !this.formMethod && !this.formNoValidate && !this.formTarget) {
      this.elementInternals.form.requestSubmit();
      return;
    }
    try {
      this.elementInternals.setFormValue(this.value ?? "");
      this.elementInternals.form.requestSubmit(this);
    } catch (e) {
      this.createAndInsertFormSubmissionFallbackControl();
      this.elementInternals.setFormValue(null);
      this.elementInternals.form.requestSubmit(this.formSubmissionFallbackControl);
    }
  }
};
__decorate([
  observable
], BaseButton.prototype, "defaultSlottedContent", void 0);
__decorate([
  attr({ mode: "boolean" })
], BaseButton.prototype, "disabled", void 0);
__decorate([
  attr({ attribute: "disabled-focusable", mode: "boolean" })
], BaseButton.prototype, "disabledFocusable", void 0);
__decorate([
  attr({ attribute: "formaction" })
], BaseButton.prototype, "formAction", void 0);
__decorate([
  attr({ attribute: "form" })
], BaseButton.prototype, "formAttribute", void 0);
__decorate([
  attr({ attribute: "formenctype" })
], BaseButton.prototype, "formEnctype", void 0);
__decorate([
  attr({ attribute: "formmethod" })
], BaseButton.prototype, "formMethod", void 0);
__decorate([
  attr({ attribute: "formnovalidate", mode: "boolean" })
], BaseButton.prototype, "formNoValidate", void 0);
__decorate([
  attr({ attribute: "formtarget" })
], BaseButton.prototype, "formTarget", void 0);
__decorate([
  attr
], BaseButton.prototype, "name", void 0);
__decorate([
  attr
], BaseButton.prototype, "type", void 0);
__decorate([
  attr
], BaseButton.prototype, "value", void 0);

// node_modules/@fluentui/web-components/dist/esm/button/button.js
var Button = class extends BaseButton {
  constructor() {
    super(...arguments);
    this.iconOnly = false;
  }
};
__decorate([
  attr
], Button.prototype, "appearance", void 0);
__decorate([
  attr
], Button.prototype, "shape", void 0);
__decorate([
  attr
], Button.prototype, "size", void 0);
__decorate([
  attr({ attribute: "icon-only", mode: "boolean" })
], Button.prototype, "iconOnly", void 0);
applyMixins(Button, StartEnd);

// node_modules/@fluentui/web-components/dist/esm/button/button.styles.js
var baseButtonStyles = css`
  ${display("inline-flex")}

  :host {
    --icon-spacing: ${spacingHorizontalSNudge};
    position: relative;
    contain: layout style;
    vertical-align: middle;
    align-items: center;
    box-sizing: border-box;
    justify-content: center;
    text-align: center;
    text-decoration-line: none;
    margin: 0;
    min-height: 32px;
    outline-style: none;
    background-color: ${colorNeutralBackground1};
    color: ${colorNeutralForeground1};
    border: ${strokeWidthThin} solid ${colorNeutralStroke1};
    padding: 0 ${spacingHorizontalM};
    min-width: 96px;
    border-radius: ${borderRadiusMedium};
    font-size: ${fontSizeBase300};
    font-family: ${fontFamilyBase};
    font-weight: ${fontWeightSemibold};
    line-height: ${lineHeightBase300};
    transition-duration: ${durationFaster};
    transition-property: background, border, color;
    transition-timing-function: ${curveEasyEase};
    cursor: pointer;
    user-select: none;
  }

  .content {
    display: inherit;
  }

  :host(:hover) {
    background-color: ${colorNeutralBackground1Hover};
    color: ${colorNeutralForeground1Hover};
    border-color: ${colorNeutralStroke1Hover};
  }

  :host(:hover:active) {
    background-color: ${colorNeutralBackground1Pressed};
    border-color: ${colorNeutralStroke1Pressed};
    color: ${colorNeutralForeground1Pressed};
    outline-style: none;
  }

  :host(:focus-visible) {
    border-color: ${colorTransparentStroke};
    outline: ${strokeWidthThick} solid ${colorTransparentStroke};
    box-shadow: ${shadow4}, 0 0 0 2px ${colorStrokeFocus2};
  }

  @media screen and (prefers-reduced-motion: reduce) {
    :host {
      transition-duration: 0.01ms;
    }
  }

  ::slotted(svg) {
    font-size: 20px;
    height: 20px;
    width: 20px;
    fill: currentColor;
  }

  ::slotted([slot='start']) {
    margin-inline-end: var(--icon-spacing);
  }

  ::slotted([slot='end']),
  [slot='end'] {
    flex-shrink: 0;
    margin-inline-start: var(--icon-spacing);
  }

  :host([icon-only]) {
    min-width: 32px;
    max-width: 32px;
  }

  :host([size='small']) {
    --icon-spacing: ${spacingHorizontalXS};
    min-height: 24px;
    min-width: 64px;
    padding: 0 ${spacingHorizontalS};
    border-radius: ${borderRadiusSmall};
    font-size: ${fontSizeBase200};
    line-height: ${lineHeightBase200};
    font-weight: ${fontWeightRegular};
  }

  :host([size='small'][icon-only]) {
    min-width: 24px;
    max-width: 24px;
  }

  :host([size='large']) {
    min-height: 40px;
    border-radius: ${borderRadiusLarge};
    padding: 0 ${spacingHorizontalL};
    font-size: ${fontSizeBase400};
    line-height: ${lineHeightBase400};
  }

  :host([size='large'][icon-only]) {
    min-width: 40px;
    max-width: 40px;
  }

  :host([size='large']) ::slotted(svg) {
    font-size: 24px;
    height: 24px;
    width: 24px;
  }

  :host(:is([shape='circular'], [shape='circular']:focus-visible)) {
    border-radius: ${borderRadiusCircular};
  }

  :host(:is([shape='square'], [shape='square']:focus-visible)) {
    border-radius: ${borderRadiusNone};
  }

  :host([appearance='primary']) {
    background-color: ${colorBrandBackground};
    color: ${colorNeutralForegroundOnBrand};
    border-color: transparent;
  }

  :host([appearance='primary']:hover) {
    background-color: ${colorBrandBackgroundHover};
  }

  :host([appearance='primary']:is(:hover, :hover:active):not(:focus-visible)) {
    border-color: transparent;
  }

  :host([appearance='primary']:is(:hover, :hover:active)) {
    color: ${colorNeutralForegroundOnBrand};
  }

  :host([appearance='primary']:hover:active) {
    background-color: ${colorBrandBackgroundPressed};
  }

  :host([appearance='primary']:focus-visible) {
    border-color: ${colorNeutralForegroundOnBrand};
    box-shadow: ${shadow2}, 0 0 0 2px ${colorStrokeFocus2};
  }

  :host([appearance='outline']) {
    background-color: ${colorTransparentBackground};
  }

  :host([appearance='outline']:hover) {
    background-color: ${colorTransparentBackgroundHover};
  }

  :host([appearance='outline']:hover:active) {
    background-color: ${colorTransparentBackgroundPressed};
  }

  :host([appearance='subtle']) {
    background-color: ${colorSubtleBackground};
    color: ${colorNeutralForeground2};
    border-color: transparent;
  }

  :host([appearance='subtle']:hover) {
    background-color: ${colorSubtleBackgroundHover};
    color: ${colorNeutralForeground2Hover};
    border-color: transparent;
  }

  :host([appearance='subtle']:hover:active) {
    background-color: ${colorSubtleBackgroundPressed};
    color: ${colorNeutralForeground2Pressed};
    border-color: transparent;
  }

  :host([appearance='subtle']:hover) ::slotted(svg) {
    fill: ${colorNeutralForeground2BrandHover};
  }

  :host([appearance='subtle']:hover:active) ::slotted(svg) {
    fill: ${colorNeutralForeground2BrandPressed};
  }

  :host([appearance='transparent']) {
    background-color: ${colorTransparentBackground};
    color: ${colorNeutralForeground2};
  }

  :host([appearance='transparent']:hover) {
    background-color: ${colorTransparentBackgroundHover};
    color: ${colorNeutralForeground2BrandHover};
  }

  :host([appearance='transparent']:hover:active) {
    background-color: ${colorTransparentBackgroundPressed};
    color: ${colorNeutralForeground2BrandPressed};
  }

  :host(:is([appearance='transparent'], [appearance='transparent']:is(:hover, :active))) {
    border-color: transparent;
  }
`;
var styles2 = css`
  ${baseButtonStyles}

  :host(:is(:disabled, [disabled], [disabled-focusable], [appearance]:disabled, [appearance][disabled], [appearance][disabled-focusable])),
  :host(:is(:disabled, [disabled], [disabled-focusable], [appearance]:disabled, [appearance][disabled], [appearance][disabled-focusable]):hover),
  :host(:is(:disabled, [disabled], [disabled-focusable], [appearance]:disabled, [appearance][disabled], [appearance][disabled-focusable]):hover:active) {
    background-color: ${colorNeutralBackgroundDisabled};
    border-color: ${colorNeutralStrokeDisabled};
    color: ${colorNeutralForegroundDisabled};
    cursor: not-allowed;
  }

  :host([appearance='primary']:is(:disabled, [disabled], [disabled-focusable])),
  :host([appearance='primary']:is(:disabled, [disabled], [disabled-focusable]):is(:hover, :hover:active)) {
    border-color: transparent;
  }

  :host([appearance='outline']:is(:disabled, [disabled], [disabled-focusable])),
  :host([appearance='outline']:is(:disabled, [disabled], [disabled-focusable]):is(:hover, :hover:active)) {
    background-color: ${colorTransparentBackground};
  }

  :host([appearance='subtle']:is(:disabled, [disabled], [disabled-focusable])),
  :host([appearance='subtle']:is(:disabled, [disabled], [disabled-focusable]):is(:hover, :hover:active)) {
    background-color: ${colorTransparentBackground};
    border-color: transparent;
  }

  :host([appearance='transparent']:is(:disabled, [disabled], [disabled-focusable])),
  :host([appearance='transparent']:is(:disabled, [disabled], [disabled-focusable]):is(:hover, :hover:active)) {
    border-color: transparent;
    background-color: ${colorTransparentBackground};
  }

  @media (forced-colors: active) {
    :host {
      background-color: ButtonFace;
      color: ButtonText;
    }

    :host(:is(:hover, :focus-visible)) {
      border-color: Highlight !important;
    }

    :host([appearance='primary']:not(:is(:hover, :focus-visible))) {
      background-color: Highlight;
      color: HighlightText;
      forced-color-adjust: none;
    }

    :host(
        :is(
            :disabled,
            [disabled],
            [disabled-focusable],
            [appearance]:disabled,
            [appearance][disabled],
            [appearance][disabled-focusable]
          )
      ) {
      background-color: ButtonFace;
      color: GrayText;
      border-color: ButtonText;
    }
  }
`;

// node_modules/@fluentui/web-components/dist/esm/button/button.template.js
function buttonTemplate(options = {}) {
  return html`
    <template
      @click="${(x, c) => x.clickHandler(c.event)}"
      @keypress="${(x, c) => x.keypressHandler(c.event)}"
    >
      ${startSlotTemplate(options)}
      <span class="content" part="content">
        <slot ${slotted("defaultSlottedContent")}></slot>
      </span>
      ${endSlotTemplate(options)}
    </template>
  `;
}
var template2 = buttonTemplate();

// node_modules/@fluentui/web-components/dist/esm/button/button.definition.js
var definition2 = {
  name: tagName2,
  registry: FluentDesignSystem.registry,
  styles: styles2,
  template: template2
};

// node_modules/@fluentui/web-components/dist/esm/tab/tab.js
var Tab = class extends FASTElement {
  disabledChanged(prev, next2) {
    this.setDisabledSideEffect(next2);
  }
  constructor() {
    super();
    this.elementInternals = this.attachInternals();
    this.elementInternals.role = "tab";
  }
  connectedCallback() {
    super.connectedCallback();
    this.slot ||= "tab";
    this.setDisabledSideEffect(this.disabled);
    if (this.styles) {
      this.$fastController.removeStyles(this.styles);
    }
    this.styles = css`
      :host {
        --textContent: '${this.textContent}';
      }
    `;
    this.$fastController.addStyles(this.styles);
    maybeSetAutoFocus(this);
  }
  setDisabledSideEffect(disabled) {
    if (disabled) {
      this.setAttribute("aria-disabled", "true");
    } else {
      this.removeAttribute("aria-disabled");
    }
    this.tabIndex = disabled && this.getAttribute("aria-selected") !== "true" ? -1 : 0;
  }
};
__decorate([
  attr({ mode: "boolean" })
], Tab.prototype, "disabled", void 0);
applyMixins(Tab, StartEnd);

// node_modules/@fluentui/web-components/dist/esm/tab/tab.options.js
function isTab(element, tagName7 = "-tab") {
  if (element?.nodeType !== Node.ELEMENT_NODE) {
    return false;
  }
  return element.tagName.toLowerCase().endsWith(tagName7);
}
var tagName3 = `${FluentDesignSystem.prefix}-tab`;

// node_modules/@fluentui/web-components/dist/esm/tab/tab.styles.js
var styles3 = css`
  ${display("inline-flex")}

  :host {
    position: relative;
    flex-direction: row;
    align-items: center;
    cursor: pointer;
    box-sizing: border-box;
    justify-content: center;
    line-height: ${lineHeightBase300};
    font-family: ${fontFamilyBase};
    font-size: ${fontSizeBase300};
    color: ${colorNeutralForeground2};
    fill: currentcolor;
    grid-row: 1;
    padding: ${spacingHorizontalM} ${spacingHorizontalMNudge};
    border-radius: ${borderRadiusMedium};
    gap: 4px;
  }

  :host .tab-content {
    display: inline-flex;
    flex-direction: column;
    padding: 0 2px;
    grid-column: 3;
  }

  :host([aria-selected='true']) {
    color: ${colorNeutralForeground1};
    font-weight: ${fontWeightSemibold};
  }

  /* adds hidden textContent to prevent shifting ui on bold / unbolding of text */
  :host .tab-content::after {
    content: var(--textContent);
    visibility: hidden;
    height: 0;
    line-height: ${lineHeightBase300};
    font-weight: ${fontWeightSemibold};
  }

  :host([aria-selected='true'])::after {
    background-color: ${colorCompoundBrandStroke};
    border-radius: ${borderRadiusCircular};
    content: '';
    inset: 0;
    position: absolute;
    z-index: 2;
  }

  :host([aria-selected='false']:hover)::after {
    background-color: ${colorNeutralStroke1Hover};
    border-radius: ${borderRadiusCircular};
    content: '';
    inset: 0;
    position: absolute;
    z-index: 1;
  }

  /*
   * TODO: Remove '(text-size-adjust: auto)' after this bug is fixed:
   * https://bugs.webkit.org/show_bug.cgi?id=298646
   * Also remove the same trick from tablist.styles.ts.
   * Using '@supports (text-size-adjust: auto)' here to exclude Safari 26 from
   * using CSS Anchor Positioning here because it crashes.
   */
  @supports (anchor-name: --a) and (text-size-adjust: auto) {
    :host([aria-selected='true'])::after {
      background-color: transparent;
    }

    :host([aria-selected='true']:hover)::after {
      background-color: ${colorNeutralStroke1Hover};
    }
  }

  :host([aria-selected='true'][disabled])::after {
    background-color: ${colorNeutralForegroundDisabled};
  }

  ::slotted([slot='start']) {
    grid-column: 2;
  }

  ::slotted([slot='end']) {
    grid-column: -1;
  }

  ::slotted([slot='start']),
  ::slotted([slot='end']) {
    display: flex;
  }
  :host([disabled]) {
    cursor: not-allowed;
    fill: ${colorNeutralForegroundDisabled};
    color: ${colorNeutralForegroundDisabled};
    pointer-events: none;
  }

  :host([disabled]:hover)::after {
    background-color: unset;
  }

  :host(:focus) {
    outline: none;
  }

  :host(:focus-visible) {
    border-radius: ${borderRadiusSmall};
    box-shadow: 0 0 0 3px ${colorStrokeFocus2};
    outline: 1px solid ${colorStrokeFocus1};
  }

  @media (forced-colors: active) {
    :host([aria-selected='true'])::after {
      background-color: Highlight;
    }
  }
`;

// node_modules/@fluentui/web-components/dist/esm/tab/tab.template.js
function tabTemplate(options = {}) {
  return html`
    <template slot="tab" role="tab">
      ${startSlotTemplate(options)}
      <span class="tab-content"><slot></slot></span>
      ${endSlotTemplate(options)}
    </template>
  `;
}
var template3 = tabTemplate({});

// node_modules/@fluentui/web-components/dist/esm/tab/tab.definition.js
var definition3 = {
  name: tagName3,
  registry: FluentDesignSystem.registry,
  styles: styles3,
  template: template3
};

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

// node_modules/@fluentui/web-components/dist/esm/utils/request-idle-callback.js
function requestIdleCallback(callback, options) {
  if ("requestIdleCallback" in globalThis) {
    return globalThis.requestIdleCallback(callback, options);
  }
  const start = Date.now();
  return setTimeout(() => {
    callback({
      didTimeout: options?.timeout ? Date.now() - start >= options.timeout : false,
      timeRemaining: () => 0
    });
  }, options?.timeout ?? 1);
}
function waitForConnectedDescendants(target, callback, options) {
  const shallow = options?.shallow ?? false;
  const timeout = options?.timeout ?? 50;
  const useIdleCallback = options?.idleCallback ?? false;
  const selector = `${shallow ? ":scope > " : ""}:not(:defined)`;
  const scheduleCheck = (deadline) => {
    if (target.querySelector(selector) === null || deadline && deadline.timeRemaining() <= 0) {
      if (useIdleCallback) {
        requestIdleCallback(callback, { timeout });
      } else {
        callback();
      }
      return;
    }
    requestIdleCallback(scheduleCheck, { timeout });
  };
  scheduleCheck();
}

// node_modules/@fluentui/web-components/dist/esm/tablist/tablist.options.js
var TablistAppearance = {
  subtle: "subtle",
  transparent: "transparent"
};
var TablistOrientation = Orientation;
var tagName4 = `${FluentDesignSystem.prefix}-tablist`;

// node_modules/@fluentui/web-components/dist/esm/tablist/tablist.base.js
var BaseTablist = class extends FASTElement {
  /** @internal */
  disabledChanged(prev, next2) {
    if (this.elementInternals) {
      toggleState(this.elementInternals, "disabled", next2);
    }
    this.setTabs({ forceDisabled: true });
  }
  orientationChanged(prev, next2) {
    if (this.elementInternals) {
      this.elementInternals.ariaOrientation = next2 ?? TablistOrientation.horizontal;
      swapStates(this.elementInternals, prev, next2, TablistOrientation);
    }
    this.setTabs();
  }
  /** @internal */
  activeidChanged(oldValue, newValue) {
    if (this.tabs?.length > 0) {
      this.changeTab(oldValue, newValue);
    }
  }
  /** @internal */
  slottedTabsChanged(prev, next2) {
    this.tabs = next2?.filter((tab) => isTab(tab)) ?? [];
  }
  /** @internal */
  tabsChanged(prev, next2) {
    if (this.tabs?.length > 0) {
      this.setTabs({ connectToPanel: true });
    }
  }
  /**
   * Function that is invoked whenever the selected tab or the tab collection changes.
   */
  setTabs({ connectToPanel = false, forceDisabled = false } = {}) {
    if (!this.tabs) {
      return;
    }
    const rootNode = this.getRootNode();
    let firstEnabledTabId = "";
    for (const tab of this.tabs) {
      if (tab.slot !== "tab") {
        continue;
      }
      tab.id ||= uniqueId("tab-");
      if (forceDisabled) {
        tab.disabled = this.disabled;
      } else {
        tab.disabled = tab.disabled || this.disabled;
      }
      if (!firstEnabledTabId && !tab.disabled) {
        firstEnabledTabId = tab.id;
      }
      const isSelected = this.activeid === tab.id;
      tab.toggleAttribute("focusgroupstart", isSelected);
      tab.setAttribute("aria-selected", isSelected.toString());
      if (connectToPanel) {
        const ariaControls = tab.getAttribute("aria-controls") ?? "";
        const panel = rootNode.getElementById(ariaControls);
        if (ariaControls && panel) {
          panel.role ??= "tabpanel";
          panel.hidden = this.activeid !== tab.id;
          this.tabPanelMap.set(tab, panel);
        }
      }
    }
    if (!this.disabled) {
      if (this.activeid) {
        this.changeTab(void 0, this.activeid);
      } else if (firstEnabledTabId) {
        this.activeid = firstEnabledTabId;
      }
    }
  }
  /** @internal */
  handleFocusIn(event) {
    this.activeid = event.target.id;
  }
  /** @internal */
  handleClick(event) {
    if (event.isTrusted) {
      return;
    }
    this.activeid = event.target.id;
  }
  changeTab(oldId, newId) {
    const rootNode = this.getRootNode();
    const prevTab = oldId ? rootNode.getElementById(oldId) : null;
    const nextTab = rootNode.getElementById(newId);
    if (!isTab(nextTab) || nextTab.disabled || !this.contains(nextTab)) {
      return;
    }
    if (prevTab) {
      prevTab.setAttribute("aria-selected", "false");
      const prevPanel = this.tabPanelMap.get(prevTab);
      if (prevPanel) {
        prevPanel.hidden = true;
      }
    }
    nextTab.setAttribute("aria-selected", "true");
    const nextPanel = this.tabPanelMap.get(nextTab);
    if (nextPanel) {
      nextPanel.hidden = false;
    }
    this.activetab = nextTab;
    this.change();
  }
  constructor() {
    super();
    this.elementInternals = this.attachInternals();
    this.disabled = false;
    this.orientation = TablistOrientation.horizontal;
    this.tabs = [];
    this.tabPanelMap = /* @__PURE__ */ new WeakMap();
    this.change = () => {
      this.$emit("change", this.activetab);
    };
    this.elementInternals.role = "tablist";
    this.elementInternals.ariaOrientation = this.orientation ?? TablistOrientation.horizontal;
  }
  /**
   * @internal
   */
  connectedCallback() {
    super.connectedCallback();
    waitForConnectedDescendants(this, () => {
      this.setTabs();
    }, { shallow: true });
  }
};
__decorate([
  attr({ mode: "boolean" })
], BaseTablist.prototype, "disabled", void 0);
__decorate([
  attr
], BaseTablist.prototype, "orientation", void 0);
__decorate([
  attr
], BaseTablist.prototype, "activeid", void 0);
__decorate([
  observable
], BaseTablist.prototype, "slottedTabs", void 0);
__decorate([
  observable
], BaseTablist.prototype, "tabs", void 0);

// node_modules/@fluentui/web-components/dist/esm/tablist/tablist.js
var Tablist = class extends BaseTablist {
  constructor() {
    super(...arguments);
    this.appearance = TablistAppearance.transparent;
  }
  disconnectedCallback() {
    this.fg?.disconnect();
    super.disconnectedCallback();
  }
  tabsChanged(prev, next2) {
    super.tabsChanged(prev, next2);
    this.fgItems ??= new ArrayItemCollection(() => this.tabs?.filter((t) => (t.getAttribute("aria-selected") === "true" || !t.disabled) && !t.hidden) ?? [], () => this.activetab ?? null);
    if (!this.fg) {
      this.fg = new FocusGroup(this, this.fgItems, {
        definition: {
          behavior: "tablist",
          axis: void 0,
          memory: false,
          wrap: true
        }
      });
    } else {
      this.fg.update();
    }
  }
};
__decorate([
  attr
], Tablist.prototype, "appearance", void 0);
__decorate([
  attr
], Tablist.prototype, "size", void 0);

// node_modules/@fluentui/web-components/dist/esm/tablist/tablist.styles.js
var styles4 = css`
  ${display("flex")}

  :host {
    --tabPaddingInline: ${spacingHorizontalMNudge};
    --tabPaddingBlock: ${spacingHorizontalM};
    --tabIndicatorInsetInline: var(--tabPaddingInline);
    --tabIndicatorInsetBlock: 0;
    box-sizing: border-box;
    color: ${colorNeutralForeground2};
    flex-direction: row;
    position: relative;
  }

  :host([size='small']) {
    --tabPaddingBlock: ${spacingVerticalSNudge};
    --tabPaddingInline: ${spacingHorizontalSNudge};
  }

  :host([size='large']) {
    --tabPaddingBlock: ${spacingVerticalL};
    --tabPaddingInline: ${spacingHorizontalMNudge};
  }

  :host([orientation='vertical']) {
    --tabPaddingBlock: ${spacingVerticalS};
    --tabIndicatorInsetBlock: ${spacingVerticalS};
    --_col-start-width: 0px;
    display: grid;
    grid-template-columns: ${spacingHorizontalMNudge} var(--_col-start-width) 1fr auto ${spacingHorizontalMNudge};
  }

  :host(:has([slot='start'])) {
    --_col-start-width: 24px;
  }

  @scope {
    :scope:has([slot='start']) {
      --_col-start-width: 24px;
    }
  }

  :host([orientation='vertical'][size='small']) {
    --tabPaddingBlock: ${spacingVerticalXXS};
    --tabIndicatorInsetBlock: ${spacingVerticalSNudge};
  }

  :host([orientation='vertical'][size='large']) {
    --tabPaddingBlock: ${spacingVerticalS};
    --tabIndicatorInsetBlock: ${spacingVerticalMNudge};
  }

  ::slotted([slot='tab']) {
    padding-inline: var(--tabPaddingInline);
    padding-block: var(--tabPaddingBlock);
  }

  :host([orientation='vertical']) ::slotted([role='tab']) {
    justify-content: flex-start;
    display: grid;
    gap: 0;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    grid-row: unset;
    padding-inline: 0;
  }

  :host ::slotted([slot='tab'])::after {
    height: ${strokeWidthThicker};
    margin-block-start: auto;
  }

  :host([orientation='vertical']) ::slotted([slot='tab'])::after {
    width: ${strokeWidthThicker};
    height: unset;
    margin-block-start: unset;
  }

  /* ::before adds a secondary indicator placeholder that appears right after click on the active tab */
  :host ::slotted([slot='tab'])::before {
    height: ${strokeWidthThicker};
    border-radius: ${borderRadiusCircular};
    content: '';
    inset-inline: var(--tabIndicatorInsetInline);
    inset-block: var(--tabIndicatorInsetBlock);
    position: absolute;
    margin-top: auto;
  }

  :host ::slotted([slot='tab'])::before {
    inset-inline: var(--tabIndicatorInsetInline);
    inset-block: var(--tabIndicatorInsetBlock);
  }

  :host ::slotted([slot='tab'][aria-selected='true'])::before {
    background-color: ${colorNeutralForegroundDisabled};
  }

  :host ::slotted([slot='tab'][aria-selected='false']:hover)::after {
    height: ${strokeWidthThicker};
    margin-block-start: auto;
    transform-origin: left;
  }

  :host([orientation='vertical']) ::slotted([slot='tab'])::before,
  :host([orientation='vertical']) ::slotted([slot='tab'][aria-selected='false']:hover)::after {
    height: unset;
    width: ${strokeWidthThicker};
    margin-inline-end: auto;
    transform-origin: top;
  }

  :host([size='small']) ::slotted([slot='tab']) {
    font-size: ${fontSizeBase300};
    line-height: ${lineHeightBase300};
  }

  :host([size='large']) ::slotted([slot='tab']) {
    font-size: ${fontSizeBase400};
    line-height: ${lineHeightBase400};
  }

  /* horizontal spacing for indicator */
  :host ::slotted([slot='tab'])::after,
  :host ::slotted([slot='tab'])::before,
  :host ::slotted([slot='tab']:hover)::after {
    inset-inline: var(--tabIndicatorInsetInline);
  }

  :host([orientation='vertical']) ::slotted([slot='tab'])::after,
  :host([orientation='vertical']) ::slotted([slot='tab'])::before,
  :host([orientation='vertical']) ::slotted([slot='tab']:hover)::after {
    inset-inline: 0;
    inset-block: var(--tabIndicatorInsetBlock);
  }

  /* disabled styles */
  :host([disabled]) {
    cursor: not-allowed;
    color: ${colorNeutralForegroundDisabled};
  }

  :host([disabled]) ::slotted([slot='tab']) {
    pointer-events: none;
    cursor: not-allowed;
    color: ${colorNeutralForegroundDisabled};
  }

  :host([disabled]) ::slotted([slot='tab']:after) {
    background-color: ${colorNeutralForegroundDisabled};
  }

  :host([disabled]) ::slotted([slot='tab'][aria-selected='true'])::after {
    background-color: ${colorNeutralForegroundDisabled};
  }

  :host([disabled]) ::slotted([slot='tab']:hover):before {
    content: unset;
  }

  :host([appearance='subtle']) ::slotted([slot='tab']:hover) {
    background-color: ${colorSubtleBackgroundHover};
    color: ${colorNeutralForeground1Hover};
    fill: ${colorCompoundBrandForeground1Hover};
  }

  :host([appearance='subtle']) ::slotted([slot='tab']:active) {
    background-color: ${colorSubtleBackgroundPressed};
    fill: ${colorSubtleBackgroundPressed};
    color: ${colorNeutralForeground1};
  }

  /*
   * TODO: Remove '(text-size-adjust: auto)' after this bug is fixed:
   * https://bugs.webkit.org/show_bug.cgi?id=298646
   * Also remove the same trick from tab.styles.ts.
   * Using '@supports (text-size-adjust: auto)' here to exclude Safari 26.0 from
   * using CSS Anchor Positioning here because it crashes.
   */
  @supports (anchor-name: --a) and (text-size-adjust: auto) {
    ::slotted([slot='tab'][aria-selected='true']) {
      anchor-name: --tab;
    }

    :host::after {
      background-color: ${colorCompoundBrandStroke};
      content: '';
      inline-size: 100%;
      inset-block: auto anchor(end);
      inset-inline: anchor(center) auto;
      position: fixed;
      position-anchor: --tab;
      transform: translateX(-50%);
      transition-property: inset-inline, width;
      transition-duration: ${durationSlow};
      transition-timing-function: ${curveDecelerateMax};
      z-index: 3;

      /* These styles should be in sync with tab.styles.ts’s :host::after */
      border-radius: ${borderRadiusCircular};
      width: calc(anchor-size() - var(--tabIndicatorInsetInline) * 2);
      height: ${strokeWidthThicker};
    }

    :host(:dir(rtl))::after {
      transform: translateX(50%);
    }

    :host([orientation='vertical'])::after {
      inset-block: anchor(center) auto;
      inset-inline: anchor(start) auto;
      transform: translateY(-50%);
      transition-property: inset-block, height;

      /* These styles should be in sync with #vertical-tab-highlight above */
      width: ${strokeWidthThicker};
      height: calc(anchor-size() - var(--tabIndicatorInsetBlock) * 2);
    }

    :host([disabled])::after {
      background-color: ${colorNeutralForegroundDisabled};
    }
  }
`;

// node_modules/@fluentui/web-components/dist/esm/tablist/tablist.template.js
var template4 = html`
  <template
    role="tablist"
    focusgroup="tablist inline block"
    @click="${(x, c) => x.handleClick(c.event)}"
    @focusin="${(x, c) => x.handleFocusIn(c.event)}"
  >
    <slot name="tab" ${slotted("slottedTabs")}></slot>
  </template>
`;

// node_modules/@fluentui/web-components/dist/esm/tablist/tablist.definition.js
var definition4 = {
  name: tagName4,
  registry: FluentDesignSystem.registry,
  styles: styles4,
  template: template4
};

// node_modules/@fluentui/web-components/dist/esm/checkbox/checkbox.base.js
var BaseCheckbox = class extends FASTElement {
  constructor() {
    super(...arguments);
    this.initialValue = "on";
    this._keydownPressed = false;
    this.dirtyChecked = false;
    this.elementInternals = this.attachInternals();
    this._validationFallbackMessage = "";
    this._value = this.initialValue;
  }
  /**
   * The element's current checked state.
   *
   * @public
   */
  get checked() {
    Observable.track(this, "checked");
    return !!this._checked;
  }
  set checked(next2) {
    this._checked = next2;
    this.setFormValue(next2 ? this.value : null);
    this.setValidity();
    this.setAriaChecked();
    toggleState(this.elementInternals, "checked", next2);
    Observable.notify(this, "checked");
  }
  /**
   * Toggles the disabled state when the user changes the `disabled` property.
   *
   * @internal
   */
  disabledChanged(prev, next2) {
    if (this.disabled) {
      this.removeAttribute("tabindex");
    } else {
      this.tabIndex = Number(this.getAttribute("tabindex") ?? 0) < 0 ? -1 : 0;
    }
    this.elementInternals.ariaDisabled = this.disabled ? "true" : "false";
    toggleState(this.elementInternals, "disabled", this.disabled);
  }
  /**
   * Sets the disabled state when the `disabled` attribute changes.
   *
   * @param prev - the previous value
   * @param next - the current value
   * @internal
   */
  disabledAttributeChanged(prev, next2) {
    this.disabled = !!next2;
  }
  /**
   * Updates the checked state when the `checked` attribute is changed, unless the checked state has been changed by the user.
   *
   * @param prev - The previous initial checked state
   * @param next - The current initial checked state
   * @internal
   */
  initialCheckedChanged(prev, next2) {
    if (!this.dirtyChecked) {
      this.checked = !!next2;
    }
  }
  /**
   * Sets the value of the input when the `value` attribute changes.
   *
   * @param prev - The previous initial value
   * @param next - The current initial value
   * @internal
   */
  initialValueChanged(prev, next2) {
    this._value = next2;
  }
  /**
   * Sets the validity of the control when the required state changes.
   *
   * @param prev - The previous required state
   * @param next - The current required state
   * @internal
   */
  requiredChanged(prev, next2) {
    if (this.elementInternals) {
      this.setValidity();
      this.elementInternals.ariaRequired = this.required ? "true" : "false";
    }
  }
  /**
   * The associated `<form>` element.
   *
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/docs/Web/API/ElementInternals/form | `ElementInternals.form`} property.
   */
  get form() {
    return this.elementInternals.form;
  }
  static {
    this.formAssociated = true;
  }
  /**
   * A reference to all associated `<label>` elements.
   *
   * @public
   */
  get labels() {
    return Object.freeze(Array.from(this.elementInternals.labels));
  }
  /**
   * The validation message. Uses the browser's default validation message for native checkboxes if not otherwise
   * specified (e.g., via `setCustomValidity`).
   *
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/docs/Web/API/ElementInternals/validationMessage | `ElementInternals.validationMessage`} property.
   */
  get validationMessage() {
    if (this.elementInternals?.validationMessage) {
      return this.elementInternals.validationMessage;
    }
    if (!this._validationFallbackMessage) {
      const validationMessageFallbackControl = document.createElement("input");
      validationMessageFallbackControl.type = "checkbox";
      validationMessageFallbackControl.required = true;
      validationMessageFallbackControl.checked = false;
      this._validationFallbackMessage = validationMessageFallbackControl.validationMessage;
    }
    return this._validationFallbackMessage;
  }
  /**
   * The element's validity state.
   *
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/docs/Web/API/ElementInternals/validity | `ElementInternals.validity`} property.
   */
  get validity() {
    return this.elementInternals.validity;
  }
  /**
   * The current value of the input.
   *
   * @public
   */
  get value() {
    Observable.track(this, "value");
    return this._value;
  }
  set value(value) {
    this._value = value;
    if (this.elementInternals) {
      this.setFormValue(value);
      this.setValidity();
    }
    Observable.notify(this, "value");
  }
  /**
   * Determines if the control can be submitted for constraint validation.
   *
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/docs/Web/API/ElementInternals/willValidate | `ElementInternals.willValidate`} property.
   */
  get willValidate() {
    return this.elementInternals.willValidate;
  }
  /**
   * Checks the validity of the element and returns the result.
   *
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/docs/Web/API/ElementInternals/checkValidity | `HTMLInputElement.checkValidity()`} method.
   */
  checkValidity() {
    return this.elementInternals.checkValidity();
  }
  /**
   * Toggles the checked state when the user clicks the element.
   *
   * @param e - the event object
   * @internal
   */
  clickHandler(e) {
    if (this.disabled) {
      return;
    }
    this.dirtyChecked = true;
    const previousChecked = this.checked;
    this.toggleChecked();
    if (previousChecked !== this.checked) {
      this.$emit("change");
      this.$emit("input");
    }
    return true;
  }
  connectedCallback() {
    super.connectedCallback();
    this.disabled = !!this.disabledAttribute;
    this.setAriaChecked();
    this.setValidity();
    maybeSetAutoFocus(this);
  }
  /**
   * Updates the form value when a user changes the `checked` state.
   *
   * @param e - the event object
   * @internal
   */
  inputHandler(e) {
    this.setFormValue(this.value);
    this.setValidity();
    return true;
  }
  /**
   * Prevents scrolling when the user presses the space key, and sets a flag to indicate that the space key was pressed
   * down while the checkbox was focused.
   *
   * @param e - the event object
   * @internal
   */
  keydownHandler(e) {
    if (e.key !== " ") {
      return true;
    }
    this._keydownPressed = true;
  }
  /**
   * Toggles the checked state when the user releases the space key.
   *
   * @param e - the event object
   * @internal
   */
  keyupHandler(e) {
    if (!this._keydownPressed || e.key !== " ") {
      return true;
    }
    this._keydownPressed = false;
    this.click();
  }
  /**
   * Resets the form value to its initial value when the form is reset.
   *
   * @internal
   */
  formResetCallback() {
    this.checked = this.initialChecked ?? false;
    this.dirtyChecked = false;
    this.setValidity();
  }
  /**
   * Reports the validity of the element.
   *
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/docs/Web/API/ElementInternals/reportValidity | `HTMLInputElement.reportValidity()`} method.
   */
  reportValidity() {
    return this.elementInternals.reportValidity();
  }
  /**
   * Sets the ARIA checked state.
   *
   * @param value - The checked state
   * @internal
   */
  setAriaChecked(value = this.checked) {
    if (this.elementInternals) {
      this.elementInternals.ariaChecked = value ? "true" : "false";
    }
  }
  /**
   * Reflects the {@link https://developer.mozilla.org/docs/Web/API/ElementInternals/setFormValue | `ElementInternals.setFormValue()`} method.
   *
   * @internal
   */
  setFormValue(value, state2) {
    this.elementInternals?.setFormValue(value, value ?? state2);
  }
  /**
   * Sets a custom validity message.
   *
   * @param message - The message to set
   * @public
   */
  setCustomValidity(message) {
    this.elementInternals.setValidity({ customError: true }, message);
    this.setValidity();
  }
  /**
   * Sets the validity of the control.
   *
   * @param flags - Validity flags to set.
   * @param message - Optional message to supply. If not provided, the control's `validationMessage` will be used.
   * @param anchor - Optional anchor to use for the validation message.
   *
   * @internal
   */
  setValidity(flags, message, anchor2) {
    if (this.elementInternals) {
      if (this.disabled || !this.required) {
        this.elementInternals.setValidity({});
        return;
      }
      this.elementInternals.setValidity({ valueMissing: !!this.required && !this.checked, ...flags }, message ?? this.validationMessage, anchor2);
    }
  }
  /**
   * Toggles the checked state of the control.
   *
   * @param force - Forces the element to be checked or unchecked
   * @public
   */
  toggleChecked(force = !this.checked) {
    this.checked = force;
  }
};
__decorate([
  observable
], BaseCheckbox.prototype, "disabled", void 0);
__decorate([
  attr({ attribute: "disabled", mode: "boolean" })
], BaseCheckbox.prototype, "disabledAttribute", void 0);
__decorate([
  attr({ attribute: "form" })
], BaseCheckbox.prototype, "formAttribute", void 0);
__decorate([
  attr({ attribute: "checked", mode: "boolean" })
], BaseCheckbox.prototype, "initialChecked", void 0);
__decorate([
  attr({ attribute: "value", mode: "fromView" })
], BaseCheckbox.prototype, "initialValue", void 0);
__decorate([
  attr
], BaseCheckbox.prototype, "name", void 0);
__decorate([
  attr({ mode: "boolean" })
], BaseCheckbox.prototype, "required", void 0);

// node_modules/@fluentui/web-components/dist/esm/checkbox/checkbox.js
var Checkbox = class extends BaseCheckbox {
  /**
   * Updates the indeterminate state when the `indeterminate` property changes.
   *
   * @param prev - the indeterminate state
   * @param next - the current indeterminate state
   * @internal
   */
  indeterminateChanged(prev, next2) {
    this.setAriaChecked();
    toggleState(this.elementInternals, "indeterminate", next2);
  }
  constructor() {
    super();
    this.elementInternals.role = "checkbox";
  }
  /**
   * Sets the ARIA checked state. If the `indeterminate` flag is true, the value will be 'mixed'.
   *
   * @internal
   * @override
   */
  setAriaChecked(value = this.checked) {
    if (this.indeterminate) {
      this.elementInternals.ariaChecked = "mixed";
      return;
    }
    super.setAriaChecked(value);
  }
  /**
   * Toggles the checked state of the control.
   *
   * @param force - Forces the element to be checked or unchecked
   * @public
   */
  toggleChecked(force = !this.checked) {
    this.indeterminate = false;
    super.toggleChecked(force);
  }
};
__decorate([
  observable
], Checkbox.prototype, "indeterminate", void 0);
__decorate([
  attr
], Checkbox.prototype, "shape", void 0);
__decorate([
  attr
], Checkbox.prototype, "size", void 0);

// node_modules/@fluentui/web-components/dist/esm/checkbox/checkbox.options.js
var tagName5 = `${FluentDesignSystem.prefix}-checkbox`;

// node_modules/@fluentui/web-components/dist/esm/checkbox/checkbox.styles.js
var styles5 = css`
  ${display("inline-flex")}

  :host {
    --size: 16px;
    background-color: ${colorNeutralBackground1};
    border-radius: ${borderRadiusSmall};
    border: ${strokeWidthThin} solid ${colorNeutralStrokeAccessible};
    box-sizing: border-box;
    cursor: pointer;
    position: relative;
    width: var(--size);
  }

  :host,
  .indeterminate-indicator,
  .checked-indicator {
    aspect-ratio: 1;
  }

  :host(:hover) {
    border-color: ${colorNeutralStrokeAccessibleHover};
  }

  :host(:active) {
    border-color: ${colorNeutralStrokeAccessiblePressed};
  }

  :host(${checkedState}:hover) {
    background-color: ${colorCompoundBrandBackgroundHover};
    border-color: ${colorCompoundBrandStrokeHover};
  }

  :host(${checkedState}:active) {
    background-color: ${colorCompoundBrandBackgroundPressed};
    border-color: ${colorCompoundBrandStrokePressed};
  }

  :host(:focus-visible) {
    outline: none;
  }

  :host(:not([slot='input']))::after {
    content: '';
    position: absolute;
    inset: -8px;
    box-sizing: border-box;
    outline: none;
    border: ${strokeWidthThick} solid ${colorTransparentStroke};
    border-radius: ${borderRadiusMedium};
  }

  :host(:not([slot='input']):focus-visible)::after {
    border-color: ${colorStrokeFocus2};
  }

  .indeterminate-indicator,
  .checked-indicator {
    color: ${colorNeutralForegroundInverted};
    inset: 0;
    margin: auto;
    position: absolute;
  }

  ::slotted([slot='checked-indicator']),
  .checked-indicator {
    fill: currentColor;
    display: inline-flex;
    flex: 1 0 auto;
    width: 12px;
  }

  :host(:not(${checkedState})) *:is(::slotted([slot='checked-indicator']), .checked-indicator) {
    display: none;
  }

  :host(${checkedState}),
  :host(${indeterminateState}) {
    border-color: ${colorCompoundBrandStroke};
  }

  :host(${checkedState}),
  :host(${indeterminateState}) .indeterminate-indicator {
    background-color: ${colorCompoundBrandBackground};
  }

  :host(${indeterminateState}) .indeterminate-indicator {
    border-radius: ${borderRadiusSmall};
    position: absolute;
    width: calc(var(--size) / 2);
    inset: 0;
  }

  :host([size='large']) {
    --size: 20px;
  }

  :host([size='large']) ::slotted([slot='checked-indicator']),
  :host([size='large']) .checked-indicator {
    width: 16px;
  }

  :host([shape='circular']),
  :host([shape='circular']) .indeterminate-indicator {
    border-radius: ${borderRadiusCircular};
  }

  :host(${nativeDisabledState}),
  :host(${nativeDisabledState}${checkedState}) {
    background-color: ${colorNeutralBackgroundDisabled};
    border-color: ${colorNeutralStrokeDisabled};
  }

  :host(${nativeDisabledState}) {
    cursor: unset;
  }

  :host(${nativeDisabledState}${indeterminateState}) .indeterminate-indicator {
    background-color: ${colorNeutralStrokeDisabled};
  }

  :host(${nativeDisabledState}${checkedState}) .checked-indicator {
    color: ${colorNeutralStrokeDisabled};
  }

  @media (forced-colors: active) {
    :host {
      border-color: FieldText;
    }

    :host(:not([slot='input']:focus-visible))::after {
      border-color: Canvas;
    }

    :host(:not(${nativeDisabledState}):hover),
    :host(${checkedState}:not(${nativeDisabledState}):hover),
    :host(:not([slot='input']):focus-visible)::after {
      border-color: Highlight;
    }

    .indeterminate-indicator,
    .checked-indicator {
      color: HighlightText;
    }

    :host(${checkedState}),
    :host(${indeterminateState}) .indeterminate-indicator {
      background-color: FieldText;
    }

    :host(${checkedState}:not(${nativeDisabledState}):hover),
    :host(${indeterminateState}:not(${nativeDisabledState}):hover) .indeterminate-indicator {
      background-color: Highlight;
    }

    :host(${nativeDisabledState}) {
      border-color: GrayText;
    }

    :host(${nativeDisabledState}${indeterminateState}) .indeterminate-indicator {
      background-color: GrayText;
    }

    :host(${nativeDisabledState}),
    :host(${nativeDisabledState}${checkedState}) .checked-indicator {
      color: GrayText;
    }
  }
`;

// node_modules/@fluentui/web-components/dist/esm/checkbox/checkbox.template.js
var checkedIndicator = html.partial(
  /* html */
  `
    <svg
        fill="currentColor"
        aria-hidden="true"
        class="checked-indicator"
        width="1em"
        height="1em"
        viewBox="0 0 12 12"
        xmlns="http://www.w3.org/2000/svg">
            <path d="M9.76 3.2c.3.29.32.76.04 1.06l-4.25 4.5a.75.75 0 0 1-1.08.02L2.22 6.53a.75.75 0 0 1 1.06-1.06l1.7 1.7L8.7 3.24a.75.75 0 0 1 1.06-.04Z" fill="currentColor"></path>
    </svg>
`
);
var indeterminateIndicator = html.partial(
  /* html */
  `
    <span class="indeterminate-indicator"></span>
`
);
function checkboxTemplate(options = {}) {
  return html`
    <template
      @click="${(x, c) => x.clickHandler(c.event)}"
      @input="${(x, c) => x.inputHandler(c.event)}"
      @keydown="${(x, c) => x.keydownHandler(c.event)}"
      @keyup="${(x, c) => x.keyupHandler(c.event)}"
    >
      <slot name="checked-indicator">${staticallyCompose(options.checkedIndicator)}</slot>
      <slot name="indeterminate-indicator">${staticallyCompose(options.indeterminateIndicator)}</slot>
    </template>
  `;
}
var template5 = checkboxTemplate({
  checkedIndicator,
  indeterminateIndicator
});

// node_modules/@fluentui/web-components/dist/esm/checkbox/checkbox.definition.js
var definition5 = {
  name: tagName5,
  registry: FluentDesignSystem.registry,
  styles: styles5,
  template: template5
};

// node_modules/@fluentui/web-components/dist/esm/text-input/text-input.options.js
var TextInputType = {
  email: "email",
  password: "password",
  tel: "tel",
  text: "text",
  url: "url"
};
var ImplicitSubmissionBlockingTypes = [
  "date",
  "datetime-local",
  "email",
  "month",
  "number",
  "password",
  "search",
  "tel",
  "text",
  "time",
  "url",
  "week"
];
var tagName6 = `${FluentDesignSystem.prefix}-text-input`;

// node_modules/@fluentui/web-components/dist/esm/text-input/text-input.base.js
var BaseTextInput = class extends FASTElement {
  constructor() {
    super(...arguments);
    this.type = TextInputType.text;
    this.dirtyValue = false;
    this.elementInternals = this.attachInternals();
  }
  /**
   * Tracks the current value of the input.
   *
   * @param prev - the previous value
   * @param next - the next value
   *
   * @internal
   */
  currentValueChanged(prev, next2) {
    this.value = next2;
  }
  /**
   * Updates the control label visibility based on the presence of default slotted content.
   *
   * @internal
   */
  defaultSlottedNodesChanged(prev, next2) {
    Updates.enqueue(() => {
      if (this.controlLabel) {
        this.controlLabel.hidden = !next2?.some((node) => node.nodeType === Node.ELEMENT_NODE || node.nodeType === Node.TEXT_NODE && !!node.textContent?.trim());
      }
    });
  }
  /**
   * Sets the value of the element to the initial value.
   *
   * @internal
   */
  initialValueChanged() {
    if (!this.dirtyValue) {
      this.value = this.initialValue;
    }
  }
  /**
   * Syncs the `ElementInternals.ariaReadOnly` property when the `readonly` property changes.
   *
   * @internal
   */
  readOnlyChanged() {
    if (this.$fastController.isConnected) {
      this.elementInternals.ariaReadOnly = `${!!this.readOnly}`;
    }
  }
  /**
   * Syncs the element's internal `aria-required` state with the `required` attribute.
   *
   * @param previous - the previous required state
   * @param next - the current required state
   *
   * @internal
   */
  requiredChanged(previous, next2) {
    if (this.$fastController.isConnected) {
      this.elementInternals.ariaRequired = `${!!next2}`;
    }
  }
  /**
   * Calls the `setValidity` method when the control reference changes.
   *
   * @param prev - the previous control reference
   * @param next - the current control reference
   *
   * @internal
   */
  controlChanged(prev, next2) {
    Updates.enqueue(() => {
      if (this.$fastController.isConnected) {
        this.setValidity();
      }
    });
  }
  static {
    this.formAssociated = true;
  }
  /**
   * The element's validity state.
   *
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/docs/Web/API/ElementInternals/validity | `ElementInternals.validity`} property.
   */
  get validity() {
    return this.elementInternals.validity;
  }
  /**
   * The validation message.
   *
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/docs/Web/API/ElementInternals/validationMessage | `ElementInternals.validationMessage`} property.
   */
  get validationMessage() {
    return this.elementInternals.validationMessage || this.control.validationMessage;
  }
  /**
   * The current value of the input.
   * @public
   */
  get value() {
    Observable.track(this, "value");
    return this.currentValue;
  }
  set value(value) {
    this.currentValue = value;
    if (this.elementInternals && this.control) {
      this.control.value = value ?? "";
      this.setFormValue(value);
      this.setValidity();
      Observable.notify(this, "value");
    }
  }
  /**
   * Determines if the control can be submitted for constraint validation.
   *
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/docs/Web/API/ElementInternals/willValidate | `ElementInternals.willValidate`} property.
   */
  get willValidate() {
    return this.elementInternals.willValidate;
  }
  /**
   * The associated form element.
   *
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/docs/Web/API/ElementInternals/form | `ElementInternals.form`} property.
   */
  get form() {
    return this.elementInternals.form;
  }
  /**
   * Change event handler for inner control.
   *
   * @internal
   * @privateRemarks
   * "Change" events are not `composable` so they will not permeate the shadow DOM boundary. This function effectively
   * proxies the change event, emitting a `change` event whenever the internal control emits a `change` event.
   */
  changeHandler(e) {
    this.setValidity();
    this.$emit("change", e, {
      bubbles: true,
      composed: true
    });
    return true;
  }
  /**
   * Checks the validity of the element and returns the result.
   *
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/docs/Web/API/ElementInternals/checkValidity | `HTMLInputElement.checkValidity()`} method.
   */
  checkValidity() {
    return this.elementInternals.checkValidity();
  }
  /**
   * Clicks the inner control when the component is clicked.
   *
   * @param e - the event object
   */
  clickHandler(e) {
    if (e.target === this) {
      this.control?.click();
    }
    return true;
  }
  connectedCallback() {
    super.connectedCallback();
    this.setFormValue(this.value);
    this.setValidity();
    maybeSetAutoFocus(this);
  }
  /**
   * Resets the value to its initial value when the form is reset.
   *
   * @internal
   */
  formResetCallback() {
    this.value = this.initialValue;
    this.dirtyValue = false;
  }
  /**
   * Handles implicit form submission when the user presses the "Enter" key.
   *
   * @internal
   */
  implicitSubmit() {
    if (!this.elementInternals.form) {
      return;
    }
    if (this.elementInternals.form.elements.length === 1) {
      this.elementInternals.form.requestSubmit();
      return;
    }
    const formElements = [...this.elementInternals.form.elements];
    const submitButton = formElements.find((x) => x.getAttribute("type") === "submit");
    if (submitButton) {
      submitButton.click();
      return;
    }
    const filteredElements = formElements.filter((x) => ImplicitSubmissionBlockingTypes.includes(x.getAttribute("type") ?? ""));
    if (filteredElements.length > 1) {
      return;
    }
    this.elementInternals.form.requestSubmit();
  }
  /**
   * Handles the internal control's `input` event.
   *
   * @internal
   */
  inputHandler(e) {
    this.dirtyValue = true;
    this.value = this.control.value;
    return true;
  }
  /**
   * Handles the internal control's `keydown` event.
   *
   * @param e - the event object
   * @internal
   */
  keydownHandler(e) {
    if (e.key === "Enter") {
      this.implicitSubmit();
    }
    return true;
  }
  /**
   * Selects all the text in the text field.
   *
   * @public
   * @privateRemarks
   * The `select` event does not permeate the shadow DOM boundary. This function effectively proxies the event,
   * emitting a `select` event whenever the internal control emits a `select` event
   *
   */
  select() {
    this.control.select();
    this.$emit("select");
  }
  /**
   * Sets the custom validity message.
   * @param message - The message to set
   *
   * @public
   */
  setCustomValidity(message) {
    this.elementInternals.setValidity({ customError: true }, message);
    this.reportValidity();
  }
  /**
   * Reports the validity of the element.
   *
   * @public
   * @remarks
   * Reflects the {@link https://developer.mozilla.org/docs/Web/API/ElementInternals/reportValidity | `HTMLInputElement.reportValidity()`} method.
   */
  reportValidity() {
    return this.elementInternals.reportValidity();
  }
  /**
   * Reflects the {@link https://developer.mozilla.org/docs/Web/API/ElementInternals/setFormValue | `ElementInternals.setFormValue()`} method.
   *
   * @internal
   */
  setFormValue(value, state2) {
    this.elementInternals?.setFormValue(value, value ?? state2);
  }
  /**
   * Sets the validity of the control.
   *
   * @param flags - Validity flags. If not provided, the control's `validity` will be used.
   * @param message - Optional message to supply. If not provided, the control's `validationMessage` will be used. If the control does not have a `validationMessage`, the message will be empty.
   * @param anchor - Optional anchor to use for the validation message. If not provided, the control will be used.
   *
   * @internal
   */
  setValidity(flags, message, anchor2) {
    if (this.elementInternals && this.control) {
      if (this.disabled) {
        this.elementInternals.setValidity({});
        return;
      }
      this.elementInternals.setValidity(flags ?? this.control.validity, message ?? this.validationMessage, anchor2 ?? this.control);
    }
  }
};
__decorate([
  attr
], BaseTextInput.prototype, "autocomplete", void 0);
__decorate([
  attr({ attribute: "current-value" })
], BaseTextInput.prototype, "currentValue", void 0);
__decorate([
  observable
], BaseTextInput.prototype, "defaultSlottedNodes", void 0);
__decorate([
  attr
], BaseTextInput.prototype, "dirname", void 0);
__decorate([
  attr({ mode: "boolean" })
], BaseTextInput.prototype, "disabled", void 0);
__decorate([
  attr({ attribute: "form" })
], BaseTextInput.prototype, "formAttribute", void 0);
__decorate([
  attr({ attribute: "value", mode: "fromView" })
], BaseTextInput.prototype, "initialValue", void 0);
__decorate([
  attr
], BaseTextInput.prototype, "list", void 0);
__decorate([
  attr({ converter: nullableNumberConverter })
], BaseTextInput.prototype, "maxlength", void 0);
__decorate([
  attr({ converter: nullableNumberConverter })
], BaseTextInput.prototype, "minlength", void 0);
__decorate([
  attr({ mode: "boolean" })
], BaseTextInput.prototype, "multiple", void 0);
__decorate([
  attr
], BaseTextInput.prototype, "name", void 0);
__decorate([
  attr
], BaseTextInput.prototype, "pattern", void 0);
__decorate([
  attr
], BaseTextInput.prototype, "placeholder", void 0);
__decorate([
  attr({ attribute: "readonly", mode: "boolean" })
], BaseTextInput.prototype, "readOnly", void 0);
__decorate([
  attr({ mode: "boolean" })
], BaseTextInput.prototype, "required", void 0);
__decorate([
  attr({ converter: nullableNumberConverter })
], BaseTextInput.prototype, "size", void 0);
__decorate([
  attr({
    converter: {
      fromView: (value) => typeof value === "string" ? ["true", ""].includes(value.trim().toLowerCase()) : null,
      toView: (value) => value.toString()
    }
  })
], BaseTextInput.prototype, "spellcheck", void 0);
__decorate([
  attr
], BaseTextInput.prototype, "type", void 0);
__decorate([
  observable
], BaseTextInput.prototype, "control", void 0);
__decorate([
  observable
], BaseTextInput.prototype, "controlLabel", void 0);

// node_modules/@fluentui/web-components/dist/esm/text-input/text-input.js
var TextInput = class extends BaseTextInput {
};
__decorate([
  attr
], TextInput.prototype, "appearance", void 0);
__decorate([
  attr({ attribute: "control-size" })
], TextInput.prototype, "controlSize", void 0);
applyMixins(TextInput, StartEnd);

// node_modules/@fluentui/web-components/dist/esm/text-input/text-input.styles.js
var styles6 = css`
  ${display("block")}

  :host {
    font-family: ${fontFamilyBase};
    font-size: ${fontSizeBase300};
    font-weight: ${fontWeightRegular};
    line-height: ${lineHeightBase300};
    max-width: 400px;
  }
  .label {
    display: flex;
    color: ${colorNeutralForeground1};
    padding-bottom: ${spacingVerticalXS};
    flex-shrink: 0;
    padding-inline-end: ${spacingHorizontalXS};
  }

  .label[hidden],
  :host(:empty) .label {
    display: none;
  }

  .root {
    align-items: center;
    background-color: ${colorNeutralBackground1};
    border: ${strokeWidthThin} solid ${colorNeutralStroke1};
    border-bottom-color: ${colorNeutralStrokeAccessible};
    border-radius: ${borderRadiusMedium};
    box-sizing: border-box;
    height: 32px;
    display: inline-flex;
    flex-direction: row;
    gap: ${spacingHorizontalXXS};
    padding: 0 ${spacingHorizontalMNudge};
    position: relative;
    width: 100%;
  }

  :has(.control:user-invalid) {
    border-color: ${colorPaletteRedBorder2};
  }

  .root::after {
    box-sizing: border-box;
    content: '';
    position: absolute;
    left: -1px;
    bottom: 0px;
    right: -1px;
    height: max(2px, ${borderRadiusMedium});
    border-radius: 0 0 ${borderRadiusMedium} ${borderRadiusMedium};
    border-bottom: 2px solid ${colorCompoundBrandStroke};
    clip-path: inset(calc(100% - 2px) 1px 0px);
    transform: scaleX(0);
    transition-property: transform;
    transition-duration: ${durationUltraFast};
    transition-delay: ${curveAccelerateMid};
  }
  .control {
    width: 100%;
    height: 100%;
    box-sizing: border-box;
    color: ${colorNeutralForeground1};
    border-radius: ${borderRadiusMedium};
    background: ${colorTransparentBackground};
    font-family: ${fontFamilyBase};
    font-weight: ${fontWeightRegular};
    font-size: ${fontSizeBase300};
    border: none;
    vertical-align: center;
  }
  .control:focus-visible {
    outline: 0;
    border: 0;
  }
  .control::placeholder {
    color: ${colorNeutralForeground4};
  }
  :host ::slotted([slot='start']),
  :host ::slotted([slot='end']) {
    display: flex;
    align-items: center;
    justify-content: center;
    color: ${colorNeutralForeground3};
    font-size: ${fontSizeBase500};
  }
  :host ::slotted([slot='start']) {
    padding-right: ${spacingHorizontalXXS};
  }
  :host ::slotted([slot='end']) {
    padding-left: ${spacingHorizontalXXS};
    gap: ${spacingHorizontalXS};
  }
  :host(:hover) .root {
    border-color: ${colorNeutralStroke1Hover};
    border-bottom-color: ${colorNeutralStrokeAccessibleHover};
  }
  :host(:active) .root {
    border-color: ${colorNeutralStroke1Pressed};
  }
  :host(:focus-within) .root {
    outline: transparent solid 2px;
    border-bottom: 0;
  }
  :host(:focus-within) .root::after {
    transform: scaleX(1);
    transition-property: transform;
    transition-duration: ${durationNormal};
    transition-delay: ${curveDecelerateMid};
  }
  :host(:focus-within:active) .root:after {
    border-bottom-color: ${colorCompoundBrandStrokePressed};
  }
  :host([appearance='outline']:focus-within) .root {
    border: ${strokeWidthThin} solid ${colorNeutralStroke1};
  }
  :host(:focus-within) .control {
    color: ${colorNeutralForeground1};
  }
  :host(${nativeDisabledState}) .root {
    background: ${colorTransparentBackground};
    border: ${strokeWidthThin} solid ${colorNeutralStrokeDisabled};
  }
  :host(${nativeDisabledState}) .control::placeholder,
  :host(${nativeDisabledState}) ::slotted([slot='start']),
  :host(${nativeDisabledState}) ::slotted([slot='end']) {
    color: ${colorNeutralForegroundDisabled};
  }
  ::selection {
    color: ${colorNeutralForegroundInverted};
    background-color: ${colorNeutralBackgroundInverted};
  }
  :host([control-size='small']) .control {
    font-size: ${fontSizeBase200};
    font-weight: ${fontWeightRegular};
    line-height: ${lineHeightBase200};
  }
  :host([control-size='small']) .root {
    height: 24px;
    gap: ${spacingHorizontalXXS};
    padding: 0 ${spacingHorizontalSNudge};
  }
  :host([control-size='small']) ::slotted([slot='start']),
  :host([control-size='small']) ::slotted([slot='end']) {
    font-size: ${fontSizeBase400};
  }
  :host([control-size='large']) .control {
    font-size: ${fontSizeBase400};
    font-weight: ${fontWeightRegular};
    line-height: ${lineHeightBase400};
  }
  :host([control-size='large']) .root {
    height: 40px;
    gap: ${spacingHorizontalS};
    padding: 0 ${spacingHorizontalM};
  }
  :host([control-size='large']) ::slotted([slot='start']),
  :host([control-size='large']) ::slotted([slot='end']) {
    font-size: ${fontSizeBase600};
  }
  :host([appearance='underline']) .root {
    background: ${colorTransparentBackground};
    border: 0;
    border-radius: 0;
    border-bottom: ${strokeWidthThin} solid ${colorNeutralStrokeAccessible};
  }
  :host([appearance='underline']:hover) .root {
    border-bottom-color: ${colorNeutralStrokeAccessibleHover};
  }
  :host([appearance='underline']:active) .root {
    border-bottom-color: ${colorNeutralStrokeAccessiblePressed};
  }
  :host([appearance='underline']:focus-within) .root {
    border: 0;
    border-bottom-color: ${colorNeutralStrokeAccessiblePressed};
  }
  :host([appearance='underline']${nativeDisabledState}) .root {
    border-bottom-color: ${colorNeutralStrokeDisabled};
  }
  :host([appearance='filled-lighter']) .root,
  :host([appearance='filled-darker']) .root {
    border: ${strokeWidthThin} solid ${colorTransparentStroke};
    box-shadow: ${shadow2};
  }
  :host([appearance='filled-lighter']) .root {
    background: ${colorNeutralBackground1};
  }
  :host([appearance='filled-darker']) .root {
    background: ${colorNeutralBackground3};
  }
  :host([appearance='filled-lighter']:hover) .root,
  :host([appearance='filled-darker']:hover) .root {
    border-color: ${colorTransparentStrokeInteractive};
  }
  :host([appearance='filled-lighter']:active) .root,
  :host([appearance='filled-darker']:active) .root {
    border-color: ${colorTransparentStrokeInteractive};
    background: ${colorNeutralBackground3};
  }
`;

// node_modules/@fluentui/web-components/dist/esm/text-input/text-input.template.js
function textInputTemplate(options = {}) {
  return html`
    <template @keydown="${(x, c) => x.keydownHandler(c.event)}">
      <label part="label" for="control" class="label" ${ref("controlLabel")}>
        <slot ${slotted("defaultSlottedNodes")}></slot>
      </label>
      <div class="root" part="root">
        ${startSlotTemplate(options)}
        <input
          class="control"
          part="control"
          id="control"
          @change="${(x, c) => x.changeHandler(c.event)}"
          @input="${(x, c) => x.inputHandler(c.event)}"
          autocomplete="${(x) => x.autocomplete}"
          ?disabled="${(x) => x.disabled}"
          list="${(x) => x.list}"
          maxlength="${(x) => x.maxlength}"
          minlength="${(x) => x.minlength}"
          ?multiple="${(x) => x.multiple}"
          name="${(x) => x.name}"
          pattern="${(x) => x.pattern}"
          placeholder="${(x) => x.placeholder}"
          ?readonly="${(x) => x.readOnly}"
          ?required="${(x) => x.required}"
          size="${(x) => x.size}"
          spellcheck="${(x) => x.spellcheck}"
          type="${(x) => x.type}"
          value="${(x) => x.value}"
          ${ref("control")}
        />
        ${endSlotTemplate(options)}
      </div>
    </template>
  `;
}
var template6 = textInputTemplate();

// node_modules/@fluentui/web-components/dist/esm/text-input/text-input.definition.js
var definition6 = {
  name: tagName6,
  registry: FluentDesignSystem.registry,
  shadowOptions: {
    delegatesFocus: true
  },
  styles: styles6,
  template: template6
};

// frontend/comfyui/ui/surface/appearance/FluentSliderGeometry.ts
function adaptFluentSliderGeometry(slider) {
  const calculate = slider.calculateNewValue.bind(slider);
  slider.calculateNewValue = (screenDistance) => {
    const track = slider.track;
    const screenWidth = track?.getBoundingClientRect().width ?? 0;
    const layoutWidth = track?.clientWidth ?? 0;
    return calculate(
      screenWidth > 0 && layoutWidth > 0 ? screenDistance * layoutWidth / screenWidth : screenDistance
    );
  };
}

// frontend/comfyui/ui/surface/appearance/FluentSliderThumb.ts
function createFluentSliderThumb(documentRef) {
  const thumb = documentRef.createElement("span");
  thumb.slot = "thumb";
  thumb.dataset.scSliderThumb = "";
  thumb.setAttribute("aria-hidden", "true");
  const svgNamespace = "http://www.w3.org/2000/svg";
  const surface = documentRef.createElementNS(svgNamespace, "svg");
  surface.setAttribute("viewBox", "0 0 20 20");
  surface.setAttribute("focusable", "false");
  const ring = documentRef.createElementNS(svgNamespace, "circle");
  ring.dataset.scSliderRing = "";
  ring.setAttribute("cx", "10");
  ring.setAttribute("cy", "10");
  ring.setAttribute("r", "9.5");
  const dot = documentRef.createElementNS(svgNamespace, "circle");
  dot.dataset.scSliderDot = "";
  dot.setAttribute("cx", "10");
  dot.setAttribute("cy", "10");
  dot.setAttribute("r", "5");
  surface.append(ring, dot);
  thumb.append(surface);
  return thumb;
}

// frontend/comfyui/ui/surface/frame/WorkflowTabStyles.ts
var WORKFLOW_TAB_ORB_CUTOUT_RADIUS = 26;
var workflowTabCss = `
  :host { height:30px; width:var(--sc-workflow-tab-width,180px); min-width:var(--sc-workflow-tab-width,180px); max-width:var(--sc-workflow-tab-width,180px); flex:0 0 var(--sc-workflow-tab-width,180px); padding:0 8px 0 20px;
    border-radius:8px 8px 0 0; gap:6px; font:13px/20px var(--sc-font-family); overflow:hidden;
    color:var(--sc-frame-muted); background:transparent; box-shadow:none; }
  :host .tab-content { min-width:0; flex:1; overflow:hidden; padding:0; }
  :host .tab-content::after { display:none; }
  :host([aria-selected='true']) { background:var(--comfy-menu-bg,#202020); color:var(--sc-text); font-weight:600; }
  :host(:not([aria-selected='true']):hover) { background:var(--sc-frame-hover); }
  :host(:not([aria-selected='true'])) { clip-path:inset(1px 1px 0 round 7px 7px 0 0); }
  :host(:not([aria-selected='true']):active) { background:var(--sc-frame-pressed); }
  :host([data-unread]:not([aria-selected='true'])) { background:var(--sc-frame-unread); }
  :host([data-sc-held]:not([aria-selected='true'])) { background:linear-gradient(var(--sc-frame-hover),var(--sc-frame-hover)),var(--sc-tab-drag-backdrop,var(--sc-surface)); }
  :host::after { content:''; position:absolute; inset:0 0 auto; height:2px; width:auto; border-radius:0;
    background:transparent; transform:none; transition:none; }
  :host([aria-selected='true'])::after { background:var(--sc-accent-indicator); }
  :host { --sc-tab-cutout-radius:0px; mask-image:radial-gradient(circle var(--sc-tab-cutout-radius) at var(--sc-tab-cutout-x,-17px) 25px,transparent calc(var(--sc-tab-cutout-radius) - 1px),#000 var(--sc-tab-cutout-radius)); }
  :host([data-orb-adjacent]) { padding-left:32px; --sc-tab-cutout-radius:${String(WORKFLOW_TAB_ORB_CUTOUT_RADIUS)}px; }
`;
var workflowTablistCss = `
  :host::after { content:none !important; display:none !important; }
  :host { height:30px; min-width:0; width:var(--sc-workflow-tabs-width); gap:0; flex:0 0 auto; overflow:clip; }
  :host ::slotted([slot='tab']) { padding:0 8px 0 20px; }
  :host ::slotted([slot='tab'][data-orb-adjacent]) { padding-left:32px; }
  :host ::slotted([slot='tab'])::before { content:none; }
  :host ::slotted([slot='tab'])::after { inset:0 0 auto; height:2px; width:auto; transform:none; border-radius:0; }
`;

// frontend/comfyui/ui/surface/appearance/FluentThemeTokens.ts
function fluentThemeTokens(theme) {
  return {
    text: String(theme.colorNeutralForeground1 ?? ""),
    "text-secondary": String(theme.colorNeutralForeground2 ?? ""),
    "text-muted": String(theme.colorNeutralForeground3 ?? ""),
    "text-subtle": String(theme.colorNeutralForeground4 ?? ""),
    "text-disabled": String(theme.colorNeutralForegroundDisabled ?? ""),
    "text-on-accent": String(theme.colorNeutralForegroundOnBrand ?? ""),
    surface: String(theme.colorNeutralBackground1 ?? ""),
    "surface-hover": String(theme.colorNeutralBackground1Hover ?? ""),
    "surface-pressed": String(theme.colorNeutralBackground1Pressed ?? ""),
    "surface-secondary": String(theme.colorNeutralBackground2 ?? ""),
    "surface-tertiary": String(theme.colorNeutralBackground3 ?? ""),
    "surface-disabled": String(theme.colorNeutralBackgroundDisabled ?? ""),
    border: String(theme.colorNeutralStroke1 ?? ""),
    "border-hover": String(theme.colorNeutralStroke1Hover ?? ""),
    "border-subtle": String(theme.colorNeutralStroke2 ?? ""),
    "border-accessible": String(theme.colorNeutralStrokeAccessible ?? ""),
    "border-accessible-hover": String(theme.colorNeutralStrokeAccessibleHover ?? ""),
    "border-disabled": String(theme.colorNeutralStrokeDisabled ?? ""),
    "brand-fill": String(theme.colorBrandBackground ?? ""),
    "brand-fill-hover": String(theme.colorBrandBackgroundHover ?? ""),
    "brand-fill-pressed": String(theme.colorBrandBackgroundPressed ?? ""),
    "brand-text": String(theme.colorBrandForeground1 ?? ""),
    "brand-border": String(theme.colorBrandStroke1 ?? ""),
    "brand-indicator": String(theme.colorCompoundBrandBackground ?? ""),
    "brand-indicator-hover": String(theme.colorCompoundBrandBackgroundHover ?? ""),
    "danger-text": String(theme.colorPaletteRedForeground1 ?? ""),
    "focus-ring": String(theme.colorStrokeFocus2 ?? ""),
    "subtle-hover": String(theme.colorSubtleBackgroundHover ?? ""),
    "subtle-selected": String(theme.colorSubtleBackgroundSelected ?? ""),
    "font-family": String(theme.fontFamilyBase ?? ""),
    "font-size": String(theme.fontSizeBase300 ?? ""),
    "font-weight": String(theme.fontWeightRegular ?? ""),
    "font-weight-strong": String(theme.fontWeightSemibold ?? ""),
    "control-radius": String(theme.borderRadiusMedium ?? ""),
    "shadow-small": String(theme.shadow4 ?? ""),
    "shadow-medium": String(theme.shadow16 ?? ""),
    "shadow-large": String(theme.shadow64 ?? ""),
    "motion-faster": String(theme.durationFaster ?? ""),
    "motion-fast": String(theme.durationFast ?? ""),
    "motion-duration": String(theme.durationNormal ?? ""),
    "motion-ease": String(theme.curveEasyEase ?? ""),
    "motion-decelerate": String(theme.curveDecelerateMid ?? "")
  };
}

// frontend/comfyui/ui/surface/appearance/ThemeTokens.ts
function themeTokenDeclarations(tokens) {
  return Object.entries(tokens).map(([role, value]) => `--sc-${role}:${value};`).join("");
}

// frontend/comfyui/ui/surface/appearance/FluentThemeRuntime.ts
var WorkflowTab = class extends Tab {
};
var WorkflowTablist = class extends Tablist {
};
var PresentationSlider = class extends Slider {
  /** Expose the official component through the shared numeric-control boundary. */
  get root() {
    return this;
  }
  /** Normalize Microsoft's string value before it reaches native widget bindings. */
  readValue() {
    return Number(this.value);
  }
  /** Translate native numeric values into Microsoft's string-based properties. */
  synchronize(minimum, maximum, step, disabled, value) {
    this.min = String(minimum);
    this.max = String(maximum);
    this.step = String(step);
    this.disabled = disabled;
    if (value !== null) this.value = String(value);
  }
  /** Preserve mounted state while native layout observes the new parent width. */
  connectedMoveCallback() {
  }
};
var PresentationButton = class extends Button {
  /** Keep mounted bindings intact when its enclosing card changes presentation. */
  connectedMoveCallback() {
  }
};
var PresentationTab = class extends Tab {
  /** Keep mounted bindings intact when the enclosing field changes presentation. */
  connectedMoveCallback() {
  }
};
var PresentationTablist = class extends Tablist {
  /** Preserve keyboard bindings while the same native field changes parent. */
  connectedMoveCallback() {
  }
  /** Leave teardown to the binding because Comfy parks native fields between presentation mounts. */
  disconnectedCallback() {
  }
  /** Release Microsoft's bindings and focus group once the owning field adapter retires. */
  releasePresentation() {
    super.disconnectedCallback();
  }
};
var PresentationCheckbox = class extends Checkbox {
  /** Keep mounted bindings intact when its enclosing card changes presentation. */
  connectedMoveCallback() {
  }
};
var PresentationTextInput = class extends TextInput {
  /** Apply the shared accessible name before focusing the real input. */
  focusInput(selectAll) {
    this.control.setAttribute("aria-label", this.getAttribute("aria-label") ?? "Search masonry");
    this.control.focus();
    if (selectAll) this.select();
  }
};
if (!customElements.get("fluent-slider")) await PresentationSlider.define(definition);
if (!customElements.get("fluent-button")) await PresentationButton.define(definition2);
if (!customElements.get("fluent-tab")) await PresentationTab.define(definition3);
if (!customElements.get("fluent-tablist")) await PresentationTablist.define(definition4);
if (!customElements.get("fluent-checkbox")) await PresentationCheckbox.define(definition5);
if (!customElements.get("fluent-text-input"))
  await PresentationTextInput.define(definition6);
var workflowTabDefinition = {
  ...definition3,
  name: "sc-workflow-tab",
  styles: [
    styles3,
    css`
      ${workflowTabCss}
    `
  ]
};
var workflowTablistDefinition = {
  ...definition4,
  name: "sc-workflow-tablist",
  styles: [
    styles4,
    css`
      ${workflowTablistCss}
    `
  ]
};
if (!customElements.get("sc-workflow-tab")) await WorkflowTab.define(workflowTabDefinition);
if (!customElements.get("sc-workflow-tablist"))
  await WorkflowTablist.define(workflowTablistDefinition);
function createWorkflowTablist() {
  return new WorkflowTablist();
}
function createWorkflowTab(documentRef) {
  return documentRef.createElement("sc-workflow-tab");
}
function createFinderInputControl(documentRef) {
  const input = documentRef.createElement("fluent-text-input");
  if (!(input instanceof PresentationTextInput))
    throw new TypeError("The registered Fluent input is incompatible.");
  return input;
}
function createBooleanCheckboxControl(documentRef) {
  const checkbox = documentRef.createElement("fluent-checkbox");
  if (!(checkbox instanceof Checkbox))
    throw new TypeError("The registered Fluent checkbox has an incompatible implementation.");
  return checkbox;
}
var nextBooleanSegmentId = 0;
function createBooleanSegmentControls(documentRef) {
  const GroupConstructor = documentRef.defaultView?.customElements.get("fluent-tablist");
  if (!GroupConstructor) throw new TypeError("The Fluent tablist is not registered.");
  const group = new GroupConstructor();
  group.setAttribute("size", "small");
  group.setAttribute("appearance", "subtle");
  const off = documentRef.createElement("fluent-tab");
  const on = documentRef.createElement("fluent-tab");
  if (!(group instanceof PresentationTablist) || !(off instanceof Tab) || !(on instanceof Tab))
    throw new TypeError("The registered Fluent tab controls have incompatible implementations.");
  off.id = `sc-boolean-${++nextBooleanSegmentId}`;
  on.id = `sc-boolean-${++nextBooleanSegmentId}`;
  off.slot = "tab";
  on.slot = "tab";
  return { group, segments: [off, on], release: () => group.releasePresentation() };
}
function createFluentNumericSlider(documentRef) {
  const slider = documentRef.createElement("fluent-slider");
  if (!(slider instanceof PresentationSlider))
    throw new TypeError("The registered Fluent slider has an incompatible implementation.");
  adaptFluentSliderGeometry(slider);
  slider.append(createFluentSliderThumb(documentRef));
  return slider;
}
var fontFamilyBase2 = '"Segoe UI Variable", "Segoe UI", sans-serif';
var themes = {
  dark: { ...webDarkTheme, fontFamilyBase: fontFamilyBase2 },
  light: { ...webLightTheme, fontFamilyBase: fontFamilyBase2 }
};
var appliedThemes = /* @__PURE__ */ new WeakMap();
function fluentTokenDeclarations(dark) {
  const theme = dark ? themes.dark : themes.light;
  return Object.entries(theme).map(([name, value]) => `--${name}:${value};`).join("") + themeTokenDeclarations(fluentThemeTokens(theme));
}
function applyOwnedFluentTheme(element, dark) {
  if (element !== element.ownerDocument.documentElement && !element.closest("body")) {
    appliedThemes.delete(element);
    return;
  }
  if (appliedThemes.has(element) && appliedThemes.get(element) === dark) return;
  setTheme(dark === null ? null : dark ? themes.dark : themes.light, element);
  for (const [role, value] of Object.entries(
    fluentThemeTokens(dark ? themes.dark : themes.light)
  )) {
    if (dark === null) element.style.removeProperty(`--sc-${role}`);
    else element.style.setProperty(`--sc-${role}`, value);
  }
  appliedThemes.set(element, dark);
}
function fluentAccentColorTokens(color, dark) {
  const theme = (dark ? createDarkTheme : createLightTheme)(createAccentBrandPalette(color));
  const indicator = accentIndicatorColor(color, dark);
  const tokens = Object.fromEntries(
    Object.entries(theme).flatMap(
      ([name, value]) => /Brand|colorStrokeFocus2/.test(name) && typeof value === "string" ? [[name, value]] : []
    )
  );
  for (const name of ["colorBrandBackground"]) tokens[name] = color;
  for (const name of [
    "colorBrandForeground1",
    "colorCompoundBrandForeground1",
    "colorCompoundBrandBackground",
    "colorBrandStroke1",
    "colorCompoundBrandStroke",
    "colorStrokeFocus2"
  ])
    tokens[name] = indicator;
  tokens.colorBrandBackgroundHover = mixAccentColor(color, "#000000", 0.08);
  tokens.colorBrandBackgroundPressed = mixAccentColor(color, "#000000", 0.16);
  tokens.colorCompoundBrandBackgroundHover = mixAccentColor(indicator, "#000000", 0.08);
  tokens.colorCompoundBrandBackgroundPressed = mixAccentColor(indicator, "#000000", 0.16);
  tokens.colorNeutralForegroundOnBrand = accentForeground(color);
  return tokens;
}
export {
  applyOwnedFluentTheme,
  createBooleanCheckboxControl,
  createBooleanSegmentControls,
  createFinderInputControl,
  createFluentNumericSlider,
  createWorkflowTab,
  createWorkflowTablist,
  fluentAccentColorTokens,
  fluentTokenDeclarations
};

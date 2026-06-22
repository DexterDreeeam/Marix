"use strict";
  // Project alias resolution for the overview UI.
  //
  // Canonical values live in the repository root `.alias/` folder. Each file's
  // name is a placeholder key and its content is the replacement text. UI strings
  // use `{{name}}` placeholders; the values are fetched at runtime into a dynamic
  // map. The key set is not fixed — any `.alias/` file can back a placeholder, and
  // the keys actually needed are discovered from the placeholders in use.
  const ALIAS_PATTERN = /\{\{\s*([A-Za-z0-9_]+)\s*\}\}/g;
  const aliasValues = {};

  function resolveAliases(text) {
    if (typeof text !== "string" || text.indexOf("{{") === -1) return text;
    return text.replace(ALIAS_PATTERN, (match, key) =>
      Object.prototype.hasOwnProperty.call(aliasValues, key) ? aliasValues[key] : match
    );
  }

  function resolveAliasesDeep(value) {
    if (typeof value === "string") return resolveAliases(value);
    if (Array.isArray(value)) {
      for (let i = 0; i < value.length; i++) value[i] = resolveAliasesDeep(value[i]);
      return value;
    }
    if (value && typeof value === "object") {
      for (const key of Object.keys(value)) value[key] = resolveAliasesDeep(value[key]);
      return value;
    }
    return value;
  }

  function collectAliasKeys(value, keys) {
    if (typeof value === "string") {
      let match;
      ALIAS_PATTERN.lastIndex = 0;
      while ((match = ALIAS_PATTERN.exec(value)) !== null) keys.add(match[1]);
    } else if (Array.isArray(value)) {
      value.forEach(item => collectAliasKeys(item, keys));
    } else if (value && typeof value === "object") {
      Object.keys(value).forEach(key => collectAliasKeys(value[key], keys));
    }
    return keys;
  }

  async function fetchAliasValue(key) {
    const candidates = [`.alias/${key}`, `../.alias/${key}`, `/.alias/${key}`];
    for (const url of candidates) {
      try {
        const response = await fetch(url, { cache: "no-cache" });
        if (response && response.ok) {
          const text = (await response.text()).trim();
          if (text) return text;
        }
      } catch (e) {
        // Try the next candidate path.
      }
    }
    return null;
  }

  async function loadAliasValues(keys) {
    const pending = Array.from(new Set(keys)).filter(
      key => !Object.prototype.hasOwnProperty.call(aliasValues, key)
    );
    const values = await Promise.all(pending.map(fetchAliasValue));
    pending.forEach((key, index) => {
      if (values[index] !== null) aliasValues[key] = values[index];
    });
    return aliasValues;
  }

  function domAliasTextNodes() {
    const nodes = [];
    if (typeof document === "undefined" || typeof document.createTreeWalker !== "function" || !document.body) {
      return nodes;
    }
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null);
    while (walker.nextNode()) {
      const node = walker.currentNode;
      if (node.nodeValue && node.nodeValue.indexOf("{{") !== -1) nodes.push(node);
    }
    return nodes;
  }

  function collectDomAliasKeys(keys) {
    if (typeof document !== "undefined" && document.title) collectAliasKeys(document.title, keys);
    for (const node of domAliasTextNodes()) collectAliasKeys(node.nodeValue, keys);
    return keys;
  }

  function resolveDomAliases() {
    if (typeof document !== "undefined" && document.title) {
      document.title = resolveAliases(document.title);
    }
    for (const node of domAliasTextNodes()) {
      node.nodeValue = resolveAliases(node.nodeValue);
    }
  }

  async function resolveConfigAliases() {
    const keys = collectAliasKeys(I18N, new Set());
    collectAliasKeys(STORAGE_KEYS, keys);
    collectAliasKeys(LOG_PREFIX, keys);
    collectAliasKeys(GITHUB_OWNER, keys);
    collectAliasKeys(GITHUB_REPO, keys);
    collectAliasKeys(LOCAL_DB_NAME, keys);
    collectDomAliasKeys(keys);
    await loadAliasValues(keys);
    resolveAliasesDeep(I18N);
    resolveAliasesDeep(STORAGE_KEYS);
    LOG_PREFIX = resolveAliases(LOG_PREFIX);
    GITHUB_OWNER = resolveAliases(GITHUB_OWNER);
    GITHUB_REPO = resolveAliases(GITHUB_REPO);
    LOCAL_DB_NAME = resolveAliases(LOCAL_DB_NAME);
    resolveDomAliases();
  }

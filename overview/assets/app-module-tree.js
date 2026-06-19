"use strict";

function buildModuleTree(files) {
  const root = createModuleNode("Marix", "");

  for (const path of Object.keys(files).sort()) {
    if (!isSourcePath(path) || isHiddenPath(path)) continue;
    const parts = path.split("/");
    let node = root;

    for (let i = 0; i < parts.length - 1; i++) {
      const childPath = parts.slice(0, i + 1).join("/");
      node.childrenMap[parts[i]] ||= createModuleNode(parts[i], childPath);
      node = node.childrenMap[parts[i]];
    }

    node.files.push(path);
    categorizeModuleFile(node, path);
  }

  markChangedModules(root);
  finalizeModuleTree(root);
  return root;
}

function createModuleNode(name, path) {
  return {
    name,
    path,
    files: [],
    changedFiles: [],
    interfaceFiles: [],
    dataStorageFiles: [],
    implementationFiles: [],
    children: [],
    childrenMap: {},
    changed: false,
    hasRust: false
  };
}

function categorizeModuleFile(node, path) {
  const fileName = path.split("/").pop().toLowerCase();
  const ext = fileName.includes(".") ? fileName.split(".").pop() : "";

  if (ext === "rs") {
    node.hasRust = true;
    node.implementationFiles.push(path);
    if (["lib.rs", "main.rs", "mod.rs"].includes(fileName) || fileName.endsWith("_api.rs") || fileName.endsWith("_interface.rs")) {
      node.interfaceFiles.push(path);
    }
    return;
  }

  if (ext === "py") {
    node.implementationFiles.push(path);
    if (fileName === "__init__.py" || fileName.endsWith("_api.py") || fileName.endsWith("_interface.py")) {
      node.interfaceFiles.push(path);
    }
    return;
  }

  if (["json", "yaml", "yml", "toml", "sql", "sqlite", "db"].includes(ext)) {
    node.dataStorageFiles.push(path);
  }

  if (["js", "ts", "ps1", "sh", "html", "css"].includes(ext)) {
    node.implementationFiles.push(path);
  }
}

function markChangedModules(root) {
  const changes = ((manifest.diff || {}).changes || {});
  for (const path of Object.keys(changes)) {
    if (!shouldIncludeVisibleSourcePath(path)) continue;
    const parts = path.split("/");
    let node = root;
    node.changed = true;
    node.changedFiles.push(path);

    for (let i = 0; i < parts.length - 1; i++) {
      if (!node.childrenMap[parts[i]]) break;
      node = node.childrenMap[parts[i]];
      node.changed = true;
      node.changedFiles.push(path);
    }
  }
}

function finalizeModuleTree(node) {
  node.children = Object.values(node.childrenMap).sort((a, b) => a.name.localeCompare(b.name));
  delete node.childrenMap;
  for (const child of node.children) {
    finalizeModuleTree(child);
    node.hasRust = node.hasRust || child.hasRust;
  }
}

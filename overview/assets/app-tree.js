"use strict";
  function buildTreeStructure(files) {
    const root = { __children: {}, __files: [] };
    const paths = Object.keys(files).sort();
    for (const p of paths) {
      const parts = p.split("/");
      let node = root;
      for (let i = 0; i < parts.length - 1; i++) {
        if (!node.__children[parts[i]]) {
          node.__children[parts[i]] = { __children: {}, __files: [] };
        }
        node = node.__children[parts[i]];
      }
      node.__files.push({ name: parts[parts.length - 1], path: p });
    }
    return root;
  }

  function renderTree(filter) {
    const container = document.getElementById("file-tree");
    container.innerHTML = "";
    const tree = buildTreeStructure(getTreeFiles());
    const filterLower = filter ? filter.toLowerCase() : null;
    const showAll = viewAllFiles || !hasVisibleDiffChanges();
    renderNode(container, tree, 0, "", filterLower, showAll);
  }

  function getTreeFiles() {
    if (viewAllFiles || !hasVisibleDiffChanges()) return manifest.files || {};

    const files = {};
    for (const path of Object.keys(((manifest.diff || {}).changes || {})).sort()) {
      files[path] = (manifest.files || {})[path] || { size: 0, content: "" };
    }
    return files;
  }

  function collapseAllDirectories() {
    for (const el of document.querySelectorAll(".dir-children")) {
      el.classList.add("collapsed");
    }
    for (const el of document.querySelectorAll(".tree-toggle")) {
      el.classList.add("collapsed");
    }
  }

  function renderNode(parent, node, depth, prefix, filter, showAll) {
    for (const dirName of Object.keys(node.__children).sort()) {
      const dirPath = prefix ? `${prefix}/${dirName}` : dirName;
      const child = node.__children[dirName];

      if (filter && !hasMatchingDescendant(child, dirPath, filter)) continue;
      if (!showAll && dirPath !== SOURCE_ROOT && !hasDiffDescendant(child, dirPath)) continue;

      const dirEl = createTreeItem(dirName, depth, true, null, dirPath);
      parent.appendChild(dirEl);

      const childContainer = document.createElement("div");
      childContainer.className = "dir-children";
      parent.appendChild(childContainer);
      dirEl.addEventListener("click", () => {
        childContainer.classList.toggle("collapsed");
        const toggle = dirEl.querySelector(".tree-toggle");
        if (toggle) toggle.classList.toggle("collapsed");

        if (overviewMode === "star") {
          openDirectoryModule(dirPath, dirEl);
        } else {
          openDirectoryChanges(dirPath, dirEl);
        }
      });

      const toggle = dirEl.querySelector(".tree-toggle");
      if (toggle) {
        toggle.addEventListener("click", evt => {
          evt.stopPropagation();
          dirEl.click();
        });
        toggle.addEventListener("dblclick", evt => evt.stopPropagation());
      }

      renderNode(childContainer, child, depth + 1, dirPath, filter, showAll);
    }

    for (const file of node.__files) {
      if (filter && !file.path.toLowerCase().includes(filter) && !file.name.toLowerCase().includes(filter)) continue;
      const change = getChange(file.path);
      if (!showAll && !change) continue;

      const el = createTreeItem(file.name, depth, false, change ? change.status : null, file.path);
      el.addEventListener("click", () => {
        if (overviewMode === "star") {
          openFileScope(file.path, el);
        } else {
          openFile(file.path, el);
        }
      });
      parent.appendChild(el);
    }
  }

  function hasVisibleDiffChanges() {
    return Object.keys(((manifest.diff || {}).changes || {})).length > 0;
  }

  function hasMatchingDescendant(node, prefix, filter) {
    for (const f of node.__files) {
      const fp = `${prefix}/${f.name}`;
      if (fp.toLowerCase().includes(filter) || f.name.toLowerCase().includes(filter)) return true;
    }
    for (const [dn, child] of Object.entries(node.__children)) {
      if (hasMatchingDescendant(child, `${prefix}/${dn}`, filter)) return true;
    }
    return false;
  }

  function hasDiffDescendant(node, prefix) {
    for (const f of node.__files) {
      if (getChange(`${prefix}/${f.name}`)) return true;
    }

    for (const [dn, child] of Object.entries(node.__children)) {
      if (hasDiffDescendant(child, `${prefix}/${dn}`)) return true;
    }
    return false;
  }

  function createTreeItem(name, depth, isDir, status, path) {
    const el = document.createElement("div");
    el.className = `tree-item${isDir ? " dir" : ""}`;
    el.dataset.path = path;

    const indent = document.createElement("span");
    indent.className = "tree-indent";
    indent.style.width = `${depth * 16}px`;
    el.appendChild(indent);

    const icon = document.createElement("span");
    icon.className = `tree-icon${isDir ? " tree-toggle" : ` tree-icon-${getFileIconClass(name)}`}`;
    if (isDir) {
      icon.innerHTML = '<i class="bi bi-chevron-down"></i>';
    } else {
      icon.textContent = getFileIcon(name);
    }
    el.appendChild(icon);

    const nameEl = document.createElement("span");
    nameEl.className = "tree-name";
    nameEl.textContent = name;
    el.appendChild(nameEl);

    if (status) {
      const badge = document.createElement("span");
      const labels = { M: ["M", "badge-modified"], A: ["A", "badge-added"], D: ["D", "badge-deleted"], R: ["R", "badge-renamed"] };
      const [label, cls] = labels[status] || ["?", "badge-modified"];
      badge.className = `badge ${cls}`;
      badge.textContent = label;
      el.appendChild(badge);
    }

    return el;
  }

  function getFileIcon(name) {
    const ext = name.split(".").pop().toLowerCase();
    const icons = {
      py: "PY", rs: "RS", md: "MD", yaml: "YML", yml: "YML",
      toml: "TOML", json: "JSON", html: "HTML", css: "CSS", js: "JS",
      png: "IMG", jpg: "IMG", jpeg: "IMG", gif: "IMG", svg: "IMG",
      txt: "TXT", sh: "SH", bat: "BAT", ps1: "PS1"
    };
    return icons[ext] || "FILE";
  }

  function getFileIconClass(name) {
    const ext = name.includes(".") ? name.split(".").pop().toLowerCase() : "file";
    return ext.replace(/[^a-z0-9]+/g, "-") || "file";
  }

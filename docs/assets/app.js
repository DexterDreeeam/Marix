/* Marix File Explorer — App Logic */

(function () {
  "use strict";

  let manifest = null;
  let diffOnly = false;
  let currentFile = null;

  // ── Init ──
  async function init() {
    try {
      const resp = await fetch("manifest.json");
      manifest = await resp.json();
    } catch (e) {
      manifest = { files: {}, diff: { prev_tag: null, latest_tag: null, changes: {} }, generated_at: "" };
    }
    renderTagInfo();
    renderTree();
    bindEvents();
  }

  // ── Tag Info ──
  function renderTagInfo() {
    const el = document.getElementById("tag-info");
    const d = manifest.diff;
    if (d.prev_tag && d.latest_tag) {
      el.textContent = `${d.prev_tag} → ${d.latest_tag}`;
    } else if (d.latest_tag) {
      el.textContent = `Since: ${d.latest_tag}`;
    } else {
      el.textContent = "No tags";
    }
  }

  // ── Build File Tree Structure ──
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

  // ── Render Tree ──
  function renderTree(filter) {
    const container = document.getElementById("file-tree");
    container.innerHTML = "";
    const tree = buildTreeStructure(manifest.files);
    const filterLower = filter ? filter.toLowerCase() : null;
    renderNode(container, tree, 0, "", filterLower);
  }

  function renderNode(parent, node, depth, prefix, filter) {
    // Directories
    const dirNames = Object.keys(node.__children).sort();
    for (const dirName of dirNames) {
      const dirPath = prefix ? prefix + "/" + dirName : dirName;
      const child = node.__children[dirName];

      // Check if any descendant matches filter / diff
      if (filter && !hasMatchingDescendant(child, dirPath, filter)) continue;
      if (diffOnly && !hasDiffDescendant(child, dirPath)) continue;

      const dirEl = createTreeItem(dirName, depth, true, null, dirPath);
      parent.appendChild(dirEl);

      const childContainer = document.createElement("div");
      childContainer.className = "dir-children";
      parent.appendChild(childContainer);

      dirEl.addEventListener("click", () => {
        childContainer.classList.toggle("collapsed");
        const toggle = dirEl.querySelector(".tree-toggle");
        if (toggle) toggle.classList.toggle("collapsed");
      });

      renderNode(childContainer, child, depth + 1, dirPath, filter);
    }

    // Files
    for (const file of node.__files) {
      if (filter && !file.path.toLowerCase().includes(filter) && !file.name.toLowerCase().includes(filter)) continue;
      const change = manifest.diff.changes[file.path];
      if (diffOnly && !change) continue;

      const status = change ? change.status : null;
      const el = createTreeItem(file.name, depth, false, status, file.path);
      el.addEventListener("click", () => openFile(file.path, el));
      parent.appendChild(el);
    }
  }

  function hasMatchingDescendant(node, prefix, filter) {
    for (const f of node.__files) {
      const fp = prefix + "/" + f.name;
      if (fp.toLowerCase().includes(filter) || f.name.toLowerCase().includes(filter)) return true;
    }
    for (const [dn, child] of Object.entries(node.__children)) {
      if (hasMatchingDescendant(child, prefix + "/" + dn, filter)) return true;
    }
    return false;
  }

  function hasDiffDescendant(node, prefix) {
    for (const f of node.__files) {
      const fp = prefix + "/" + f.name;
      if (manifest.diff.changes[fp]) return true;
    }
    for (const [dn, child] of Object.entries(node.__children)) {
      if (hasDiffDescendant(child, prefix + "/" + dn)) return true;
    }
    return false;
  }

  function createTreeItem(name, depth, isDir, status, path) {
    const el = document.createElement("div");
    el.className = "tree-item" + (isDir ? " dir" : "");
    el.dataset.path = path;

    // Indent
    const indent = document.createElement("span");
    indent.className = "tree-indent";
    indent.style.width = (depth * 16) + "px";
    el.appendChild(indent);

    // Icon
    const icon = document.createElement("span");
    icon.className = "tree-icon" + (isDir ? " tree-toggle" : "");
    icon.textContent = isDir ? "▾" : getFileIcon(name);
    el.appendChild(icon);

    // Name
    const nameEl = document.createElement("span");
    nameEl.className = "tree-name";
    nameEl.textContent = name;
    el.appendChild(nameEl);

    // Status badge
    if (status) {
      const badge = document.createElement("span");
      const labels = { M: ["M", "badge-modified"], A: ["A", "badge-added"], D: ["D", "badge-deleted"], R: ["R", "badge-renamed"] };
      const [label, cls] = labels[status] || ["?", "badge-modified"];
      badge.className = "badge " + cls;
      badge.textContent = label;
      el.appendChild(badge);
    }

    return el;
  }

  function getFileIcon(name) {
    const ext = name.split(".").pop().toLowerCase();
    const icons = {
      py: "🐍", rs: "🦀", md: "📝", yaml: "⚙", yml: "⚙",
      toml: "⚙", json: "{ }", html: "🌐", css: "🎨", js: "📜",
      png: "🖼", jpg: "🖼", jpeg: "🖼", gif: "🖼", svg: "🖼",
      txt: "📄", sh: "🔧", bat: "🔧", ps1: "🔧"
    };
    return icons[ext] || "📄";
  }

  // ── Open File ──
  function openFile(path, treeEl) {
    // Update active state
    document.querySelectorAll(".tree-item.active").forEach(el => el.classList.remove("active"));
    if (treeEl) treeEl.classList.add("active");

    currentFile = path;

    // Update header
    document.getElementById("file-path").textContent = path;
    const change = manifest.diff.changes[path];
    const statusEl = document.getElementById("file-status");
    if (change) {
      const labels = { M: "Modified", A: "Added", D: "Deleted", R: "Renamed" };
      const classes = { M: "badge-modified", A: "badge-added", D: "badge-deleted", R: "badge-renamed" };
      statusEl.innerHTML = `<span class="badge ${classes[change.status] || "badge-modified"}">${labels[change.status] || change.status}</span>`;
    } else {
      statusEl.innerHTML = "";
    }

    // Hide all views
    hideAllViews();

    const fileData = manifest.files[path];
    if (!fileData) {
      showWelcome("File content not available.");
      return;
    }

    const ext = path.split(".").pop().toLowerCase();

    // Image
    if (["png", "jpg", "jpeg", "gif", "svg", "webp", "ico"].includes(ext)) {
      showImage(fileData);
      return;
    }

    // Show diff if file has changes
    if (change && change.hunks && change.hunks.length > 0) {
      showDiffView(path, fileData, change);
      return;
    }

    // Markdown
    if (ext === "md") {
      showMarkdown(fileData.content || "");
      return;
    }

    // Code
    showCode(fileData.content || "", ext);
  }

  function hideAllViews() {
    document.getElementById("welcome").style.display = "none";
    document.getElementById("markdown-view").style.display = "none";
    document.getElementById("image-view").style.display = "none";
    document.getElementById("code-view").style.display = "none";
    document.getElementById("diff-view").style.display = "none";
    document.getElementById("diff-summary").style.display = "none";
  }

  function showWelcome(msg) {
    const el = document.getElementById("welcome");
    el.style.display = "block";
    if (msg) el.innerHTML = `<p>${msg}</p>`;
  }

  function showMarkdown(content) {
    const el = document.getElementById("markdown-view");
    el.style.display = "block";
    el.innerHTML = marked.parse(content);
    // Highlight code blocks in markdown
    el.querySelectorAll("pre code").forEach(block => hljs.highlightElement(block));
  }

  function showImage(fileData) {
    const el = document.getElementById("image-view");
    el.style.display = "flex";
    const img = document.getElementById("image-el");
    if (fileData.base64) {
      img.src = `data:${fileData.mime || "image/png"};base64,${fileData.base64}`;
    } else {
      img.src = fileData.url || "";
    }
  }

  function showCode(content, ext) {
    const view = document.getElementById("code-view");
    view.style.display = "block";
    const codeEl = document.getElementById("code-el");
    codeEl.textContent = content;
    codeEl.className = "";
    const langMap = { py: "python", rs: "rust", js: "javascript", ts: "typescript", yml: "yaml", yaml: "yaml", sh: "bash", md: "markdown", html: "html", css: "css", json: "json", toml: "toml" };
    const lang = langMap[ext];
    if (lang) {
      codeEl.classList.add("language-" + lang);
    }
    hljs.highlightElement(codeEl);
  }

  function showDiffView(path, fileData, change) {
    const diffView = document.getElementById("diff-view");
    diffView.style.display = "block";

    let html = "";

    // Diff summary with reasons
    if (change.hunks && change.hunks.length > 0) {
      html += `<div class="diff-summary"><h4>Changes in this file (${change.hunks.length} section${change.hunks.length > 1 ? "s" : ""})</h4>`;
      for (const hunk of change.hunks) {
        html += `<div class="diff-hunk">`;
        html += `<div class="diff-hunk-header">${escapeHtml(hunk.header || "")}</div>`;
        if (hunk.reason) {
          html += `<div class="diff-hunk-reason">Reason: ${escapeHtml(hunk.reason)}</div>`;
        }
        html += `</div>`;
      }
      html += `</div>`;
    }

    // Diff lines
    if (change.diff_lines && change.diff_lines.length > 0) {
      for (const line of change.diff_lines) {
        const type = line[0]; // '+', '-', ' ', '@@'
        const content = line.substring(1);
        let cls = "";
        if (type === "+") cls = "diff-line-add";
        else if (type === "-") cls = "diff-line-del";
        else if (type === "@") cls = "diff-line-hunk";
        html += `<div class="diff-line ${cls}">`;
        html += `<span class="diff-line-content">${escapeHtml(line)}</span>`;
        html += `</div>`;
      }
    }

    diffView.innerHTML = html;
  }

  function escapeHtml(text) {
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  }

  // ── Events ──
  function bindEvents() {
    // Search
    const searchInput = document.getElementById("search-input");
    let searchTimeout;
    searchInput.addEventListener("input", () => {
      clearTimeout(searchTimeout);
      searchTimeout = setTimeout(() => {
        renderTree(searchInput.value.trim());
      }, 200);
    });

    // Diff toggle
    const btnDiff = document.getElementById("btn-toggle-diff");
    btnDiff.addEventListener("click", () => {
      diffOnly = !diffOnly;
      btnDiff.classList.toggle("active", diffOnly);
      renderTree(searchInput.value.trim());
    });
  }

  // ── Start ──
  document.addEventListener("DOMContentLoaded", init);
})();

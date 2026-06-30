"use strict";
  async function openFile(path, treeEl) {
    document.querySelectorAll(".tree-item.active").forEach(el => el.classList.remove("active"));
    const activeTreeItem = treeEl || findTreeItem(path);
    if (activeTreeItem) activeTreeItem.classList.add("active");

    applyStarMapState({
      selectionKind: "file",
      filePath: path,
      scopePath: getParentPath(path)
    }, {
      eventName: "file-view-file",
      render: false,
      syncTree: false
    });
    const change = getChange(path);
    const fileData = (manifest.files || {})[path];
    const statusEl = document.getElementById("file-status");

    document.getElementById("file-path").textContent = path;
    statusEl.innerHTML = change ? renderStatusBadge(change.status) : "";
    hideAllViews();

    if (!fileData) {
      if (change) {
        showChangedSections(path, change);
        return;
      }
      showWelcome(t("fileUnavailable"));
      return;
    }

    if (!viewWholeFile) {
      await ensureFileContent(path);
      showChangedSections(path, change || { diff_lines: [], hunks: [] });
      return;
    }

    const ext = path.split(".").pop().toLowerCase();
    await ensureFileContent(path);
    if (["png", "jpg", "jpeg", "gif", "svg", "webp", "ico"].includes(ext)) {
      showImage(fileData);
    } else if (change) {
      showFullFileWithChanges(path, fileData.content || "", change, ext);
    } else if (ext === "md") {
      showMarkdown(fileData.content || "");
    } else {
      showCode(fileData.content || "", ext);
    }
  }

  function openFileScope(path, treeEl, options = {}) {
    focusStarMapFile(path, {
      setScopeToParent: true,
      fit: options.openPopover !== true,
      openPopover: options.openPopover === true
    });
    if (treeEl) markTreeSelection(path);
  }

  function isFocusedStarMapFile(path) {
    return starMapSelection.kind === "file" && starMapSelection.path === path;
  }

  function renderStatusBadge(status) {
    const normalized = normalizeChangeStatus(status);
    const labels = {
      modified: [t("statusModified"), "badge-modified"],
      added: [t("statusAdded"), "badge-added"],
      deleted: [t("statusDeleted"), "badge-deleted"],
      renamed: [t("statusRenamed"), "badge-renamed"]
    };
    const [label, cls] = labels[normalized] || [status, "badge-modified"];
    return `<span class="badge ${cls}">${escapeHtml(label)}</span>`;
  }

  function findTreeItem(path) {
    for (const el of document.querySelectorAll(".tree-item")) {
      if (el.dataset.path === path) return el;
    }
    return null;
  }

  function hideAllViews() {
    hideFileViews();
    document.getElementById("star-map-workspace").style.display = "none";
  }

  function hideFileViews() {
    document.getElementById("welcome").style.display = "none";
    document.getElementById("markdown-view").style.display = "none";
    document.getElementById("image-view").style.display = "none";
    document.getElementById("code-view").style.display = "none";
    document.getElementById("diff-view").style.display = "none";
  }

  function restoreFileView() {
    if (currentFile && (manifest.files || {})[currentFile]) {
      openFile(currentFile, findTreeItem(currentFile));
      return;
    }

    if (currentDirectory) {
      openDirectoryChanges(currentDirectory, findTreeItem(currentDirectory));
      return;
    }

    hideAllViews();
    renderWelcome();
    document.getElementById("welcome").style.display = "block";
    document.getElementById("file-path").textContent = t("selectFile");
    document.getElementById("file-status").innerHTML = "";
  }

  function openDirectoryChanges(dirPath, treeEl) {
    document.querySelectorAll(".tree-item.active").forEach(el => el.classList.remove("active"));
    if (treeEl) treeEl.classList.add("active");
    applyStarMapState({
      selectionKind: "module",
      modulePath: dirPath
    }, {
      eventName: "file-view-directory",
      render: false,
      syncTree: false
    });
    document.getElementById("file-path").textContent = `${dirPath}/`;
    document.getElementById("file-status").innerHTML = "";
    hideAllViews();

    renderDirectoryChanges(dirPath);
  }

  async function renderDirectoryChanges(dirPath) {
    // Folder detail views are always based on changed files, regardless of the tree filter.
    const changedFiles = listChangedFilesUnder(dirPath);
    if (viewWholeFile) {
      await showFolderWholeFiles(dirPath, changedFiles);
    } else {
      await showFolderChangedSections(dirPath, changedFiles);
    }
  }

  function openDirectoryModule(dirPath, treeEl) {
    const module = findModule(moduleRoot, dirPath);
    if (module) {
      selectStarMapModule(module.path, treeEl ? { syncTree: false } : {});
      if (treeEl) markTreeSelection(module.path);
    }
  }

  function listChangedFilesUnder(dirPath) {
    const prefix = `${dirPath}/`;
    return getChangedVisiblePaths()
      .filter(path => path === dirPath || path.startsWith(prefix))
      .sort();
  }

  function getParentPath(path) {
    const index = path.lastIndexOf("/");
    return index >= 0 ? path.slice(0, index) : "";
  }

  async function showFolderChangedSections(dirPath, files) {
    const diffView = document.getElementById("diff-view");
    diffView.style.display = "block";
    if (files.length === 0) {
      diffView.innerHTML = `<div class="diff-empty">${escapeHtml(t("noChangedSections"))}</div>`;
      return;
    }

    await Promise.all(files.map(async file => {
      if ((manifest.files || {})[file]) await ensureFileContent(file);
    }));

    const panels = files.map(file => {
      const change = getChange(file);
      const sections = splitDiffSections(change.diff_lines || [], change.hunks || []);
      if (sections.length === 0) return "";
      const filePanels = sections.map((section, index) => renderDiffPanel(file, section, index + 1)).filter(Boolean).join("");
      if (!filePanels) return "";
      return `
        <section class="folder-change-group">
          <h4>${escapeHtml(file)}</h4>
          <div class="diff-panel-list">${filePanels}</div>
        </section>
      `;
    }).join("");

    diffView.innerHTML = `
      <div class="diff-view-header">
        <h3>${escapeHtml(t("folderChangesTitle"))}</h3>
        <p>${escapeHtml(t("folderChangesSubtitle"))}</p>
      </div>
      ${panels || `<div class="diff-empty">${escapeHtml(t("noChangedSections"))}</div>`}
    `;
  }

  async function showFolderWholeFiles(dirPath, files) {
    const diffView = document.getElementById("diff-view");
    diffView.style.display = "block";
    if (files.length === 0) {
      diffView.innerHTML = `<div class="diff-empty">${escapeHtml(t("noChangedSections"))}</div>`;
      return;
    }

    diffView.innerHTML = `<div class="diff-empty">${escapeHtml(t("loadingOverview"))}</div>`;

    const panels = (await Promise.all(files.map(async file => {
      const data = (manifest.files || {})[file];
      const change = getChange(file);
      if (!data || !change) return "";
      await ensureFileContent(file);
      const ext = file.split(".").pop().toLowerCase();
      return renderFullFilePanel(file, data.content || "", change, ext);
    }))).join("");

    diffView.innerHTML = `
      <div class="diff-view-header">
        <h3>${escapeHtml(t("folderChangesTitle"))}</h3>
        <p>${escapeHtml(t("folderChangesSubtitle"))}</p>
      </div>
      ${panels || `<div class="diff-empty">${escapeHtml(t("noChangedSections"))}</div>`}
    `;
  }

  function showWelcome(msg) {
    const el = document.getElementById("welcome");
    el.style.display = "block";
    if (msg) el.innerHTML = `<p>${escapeHtml(msg)}</p>`;
  }

  function showMarkdown(content) {
    const el = document.getElementById("markdown-view");
    const parts = splitPrivateSourceContent(content);
    el.style.display = "block";
    el.innerHTML = `
      ${marked.parse(parts.publicContent)}
      ${renderPrivateCodeReveal(parts.privateContent ? `<div class="private-markdown-content">${marked.parse(parts.privateContent)}</div>` : "")}
    `;
    el.querySelectorAll("pre code").forEach(block => hljs.highlightElement(block));
  }

  function showImage(fileData) {
    const el = document.getElementById("image-view");
    const img = document.getElementById("image-el");
    const src = fileData.base64 ? `data:${fileData.mime || "image/png"};base64,${fileData.base64}` : (fileData.url || "");
    if (!src) {
      el.style.display = "none";
      showWelcome(t("fileUnavailable"));
      return;
    }
    img.src = src;
    img.alt = currentFile || "";
    el.style.display = "flex";
  }

  function showCode(content, ext) {
    const view = document.getElementById("code-view");
    view.style.display = "block";
    const codeEl = document.getElementById("code-el");
    const langMap = { py: "python", rs: "rust", js: "javascript", ts: "typescript", yml: "yaml", yaml: "yaml", sh: "bash", md: "markdown", html: "html", css: "css", json: "json", toml: "toml" };
    const lang = langMap[ext];
    codeEl.className = "";
    codeEl.removeAttribute("data-highlighted");
    if (lang) codeEl.classList.add(`language-${lang}`);
    const parts = splitPrivateSourceContent(content);
    codeEl.innerHTML = highlightSource(parts.publicContent, lang);
    const existingReveal = view.querySelector(".private-code-reveal");
    if (existingReveal) existingReveal.remove();
    if (parts.privateContent) {
      const languageClass = lang ? ` class="language-${escapeHtml(lang)}"` : "";
      const privateHtml = `<pre class="private-plain-code"><code${languageClass}>${highlightSource(parts.privateContent, lang)}</code></pre>`;
      view.insertAdjacentHTML("beforeend", renderPrivateCodeReveal(privateHtml));
    }
  }

  function showChangedSections(path, change) {
    const diffView = document.getElementById("diff-view");
    diffView.style.display = "block";

    const sections = splitDiffSections(change.diff_lines || [], change.hunks || []);
    if (sections.length === 0) {
      diffView.innerHTML = `<div class="diff-empty">${escapeHtml(t("noChangedSections"))}</div>`;
      return;
    }

    const panels = sections.map((section, index) => renderDiffPanel(path, section, index + 1)).filter(Boolean).join("");
    if (!panels) {
      diffView.innerHTML = `<div class="diff-empty">${escapeHtml(t("noChangedSections"))}</div>`;
      return;
    }
    diffView.innerHTML = `
      <div class="diff-view-header">
        <h3>${escapeHtml(t("diffPanelTitle"))}</h3>
        <p>${escapeHtml(t("diffPanelSubtitle"))}</p>
      </div>
      <div class="diff-panel-list">${panels}</div>
    `;
  }

  function showFullFileWithChanges(path, content, change, ext) {
      const diffView = document.getElementById("diff-view");
      diffView.style.display = "block";

      diffView.innerHTML = `
        <div class="diff-view-header">
          <h3>${escapeHtml(t("fullFileTitle"))}</h3>
          <p>${escapeHtml(t("fullFileSubtitle"))}</p>
        </div>
        ${renderFullFilePanel(path, content, change, ext)}
      `;
    }

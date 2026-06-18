"use strict";
  async function openFile(path, treeEl) {
    document.querySelectorAll(".tree-item.active").forEach(el => el.classList.remove("active"));
    const activeTreeItem = treeEl || findTreeItem(path);
    if (activeTreeItem) activeTreeItem.classList.add("active");

    currentFile = path;
    currentDirectory = null;
    setScopePath(getParentPath(path));
    localStorage.setItem(STORAGE_KEYS.currentFile, path);
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

  function openFileScope(path, treeEl) {
    document.querySelectorAll(".tree-item.active").forEach(el => el.classList.remove("active"));
    if (treeEl) treeEl.classList.add("active");
    currentFile = path;
    currentDirectory = null;
    setScopePath(getParentPath(path));
    localStorage.setItem(STORAGE_KEYS.currentFile, path);
    requestStarMapFit();
    selectedModule = getScopeModule();
    currentDirectory = selectedModule.path;
    renderModuleDetails(selectedModule);
    renderStarMap();
  }

  function renderStatusBadge(status) {
    const labels = {
      M: [t("statusModified"), "badge-modified"],
      A: [t("statusAdded"), "badge-added"],
      D: [t("statusDeleted"), "badge-deleted"],
      R: [t("statusRenamed"), "badge-renamed"]
    };
    const [label, cls] = labels[status] || [status, "badge-modified"];
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
    currentFile = null;
    currentDirectory = dirPath;
    setScopePath(dirPath);
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
      showFolderChangedSections(dirPath, changedFiles);
    }
  }

  function openDirectoryModule(dirPath, treeEl) {
    document.querySelectorAll(".tree-item.active").forEach(el => el.classList.remove("active"));
    if (treeEl) treeEl.classList.add("active");
    const module = findModule(moduleRoot, dirPath);
    if (module) {
      setScopePath(dirPath);
      requestStarMapFit();
      selectedModule = module;
      renderModuleDetails(module);
      renderStarMap();
    }
  }

  function listChangedFilesUnder(dirPath) {
    const prefix = `${dirPath}/`;
    return Object.keys(((manifest.diff || {}).changes || {}))
      .filter(path => isSourcePath(path))
      .filter(path => path === dirPath || path.startsWith(prefix))
      .sort();
  }

  function getParentPath(path) {
    const index = path.lastIndexOf("/");
    return index >= 0 ? path.slice(0, index) : "";
  }

  function showFolderChangedSections(dirPath, files) {
    const diffView = document.getElementById("diff-view");
    diffView.style.display = "block";
    if (files.length === 0) {
      diffView.innerHTML = `<div class="diff-empty">${escapeHtml(t("noChangedSections"))}</div>`;
      return;
    }

    const panels = files.map(file => {
      const change = getChange(file);
      const sections = splitDiffSections(change.diff_lines || [], change.hunks || []);
      if (sections.length === 0) return "";
      return `
        <section class="folder-change-group">
          <h4>${escapeHtml(file)}</h4>
          <div class="diff-panel-list">${sections.map((section, index) => renderDiffPanel(file, section, index + 1)).join("")}</div>
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
    el.style.display = "block";
    el.innerHTML = marked.parse(content);
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
    codeEl.textContent = content;
    codeEl.className = "";
    codeEl.removeAttribute("data-highlighted");
    const langMap = { py: "python", rs: "rust", js: "javascript", ts: "typescript", yml: "yaml", yaml: "yaml", sh: "bash", md: "markdown", html: "html", css: "css", json: "json", toml: "toml" };
    const lang = langMap[ext];
    if (lang) codeEl.classList.add(`language-${lang}`);
    hljs.highlightElement(codeEl);
  }

  function showChangedSections(path, change) {
    const diffView = document.getElementById("diff-view");
    diffView.style.display = "block";

    const sections = splitDiffSections(change.diff_lines || [], change.hunks || []);
    if (sections.length === 0) {
      diffView.innerHTML = `<div class="diff-empty">${escapeHtml(t("noChangedSections"))}</div>`;
      return;
    }

    const panels = sections.map((section, index) => renderDiffPanel(path, section, index + 1)).join("");
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

    function renderFullFilePanel(path, content, change, ext) {
      const markers = collectDiffMarkers(change.diff_lines || []);
      const lines = content.split(/\r?\n/);
      const languageName = getLanguageFromExt(ext);
      const body = [];

      for (let i = 0; i < lines.length; i++) {
        const lineNumber = i + 1;
        body.push(renderDeletedLines(markers.deletedBefore.get(lineNumber), languageName));
        body.push(renderFullFileLine(lineNumber, lines[i], markers.addedLines.has(lineNumber), languageName));
      }

      body.push(renderDeletedLines(markers.deletedBefore.get(lines.length + 1), languageName));

      return `
        <div class="full-file-panel">
          <div class="full-file-header">
            <code>${escapeHtml(path)}</code>
            ${renderFullFileLegend()}
          </div>
          <div class="full-file-lines">${body.join("")}</div>
        </div>
      `;
    }

    function collectDiffMarkers(diffLines) {
      const addedLines = new Set();
      const deletedBefore = new Map();
      let oldLine = 0;
      let newLine = 0;

      for (const line of diffLines) {
        if (line.startsWith("@@")) {
          const match = line.match(/@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
          if (match) {
            oldLine = Number(match[1]);
            newLine = Number(match[2]);
          }
          continue;
        }

        if (line.startsWith("+")) {
          addedLines.add(newLine);
          newLine += 1;
        } else if (line.startsWith("-")) {
          const bucket = deletedBefore.get(newLine) || [];
          bucket.push(line.substring(1));
          deletedBefore.set(newLine, bucket);
          oldLine += 1;
        } else if (line.startsWith(" ")) {
          oldLine += 1;
          newLine += 1;
        }
      }

      return { addedLines, deletedBefore };
    }

    function renderFullFileLegend() {
      return `
        <div class="full-file-legend">
          <span class="legend-item legend-added">${escapeHtml(t("addedLine"))}</span>
          <span class="legend-item legend-existing">${escapeHtml(t("existingLine"))}</span>
          <span class="legend-item legend-deleted">${escapeHtml(t("deletedLine"))}</span>
        </div>
      `;
    }

    function renderFullFileLine(lineNumber, content, isAdded, languageName) {
      const cls = isAdded ? "full-line-add" : "full-line-existing";
      return `
        <div class="full-file-line ${cls}">
          <span class="full-line-number">${lineNumber}</span>
          <span class="full-line-content">${highlightLine(content, languageName)}</span>
        </div>
      `;
    }

    function renderDeletedLines(lines, languageName) {
      if (!lines || lines.length === 0) return "";
      return lines.map(line => `
        <div class="full-file-line full-line-del">
          <span class="full-line-number">-</span>
          <span class="full-line-content">${highlightLine(line, languageName)}</span>
        </div>
      `).join("");
    }

    function getLanguageFromExt(ext) {
      const langMap = {
        py: "python",
        rs: "rust",
        js: "javascript",
        ts: "typescript",
        yml: "yaml",
        yaml: "yaml",
        sh: "bash",
        md: "markdown",
        html: "xml",
        css: "css",
        json: "json",
        toml: "toml",
        xml: "xml"
      };
      return langMap[ext] || "";
    }

    function highlightLine(content, languageName) {
      if (languageName && window.hljs && hljs.getLanguage(languageName)) {
        try {
          return hljs.highlight(content, { language: languageName, ignoreIllegals: true }).value;
        } catch (e) {
          return escapeHtml(content);
        }
      }
      return escapeHtml(content);
    }

  function splitDiffSections(diffLines, hunks) {
    const sections = [];
    let current = null;
    let hunkIndex = 0;

    for (const line of diffLines) {
      if (line.startsWith("@@")) {
        if (current) sections.push(current);
        const hunk = hunks[hunkIndex] || {};
        current = { header: line, reason: hunk.reason || "", lines: [] };
        hunkIndex += 1;
      } else if (current && (line.startsWith("+") || line.startsWith("-") || line.startsWith(" "))) {
        current.lines.push(line);
      }
    }

    if (current) sections.push(current);
    return sections;
  }

  function renderDiffPanel(path, section, number) {
    const lines = section.lines.map(renderDiffLine).join("");
    const reason = section.reason || t("reasonPending");
    return `
      <section class="diff-panel">
        <header class="diff-panel-header">
          <span class="diff-panel-number">#${number}</span>
          <code>${escapeHtml(section.header || path)}</code>
        </header>
        <div class="diff-panel-reason">
          <strong>${escapeHtml(t("reason"))}:</strong> ${escapeHtml(reason)}
        </div>
        <div class="diff-panel-lines">${lines}</div>
      </section>
    `;
  }

  function renderDiffLine(line) {
    let cls = "diff-line-context";
    if (line.startsWith("+")) cls = "diff-line-add";
    else if (line.startsWith("-")) cls = "diff-line-del";
    return `<div class="diff-line ${cls}"><span class="diff-line-content">${escapeHtml(line)}</span></div>`;
  }

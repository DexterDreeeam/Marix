"use strict";
  async function showCodeSegmentsPopover(title, segments, status = "unchanged") {
    const blocks = [];
    const segmentsByPath = new Map();
    for (const segment of segments) {
      const fileSegments = segmentsByPath.get(segment.sourcePath) || [];
      fileSegments.push(segment);
      segmentsByPath.set(segment.sourcePath, fileSegments);
    }

    for (const [sourcePath, fileSegments] of segmentsByPath) {
      const entry = await ensureFileContent(sourcePath);
      const content = (entry && entry.content) || "";
      const fallbackLanguage = getLanguageFromExt(sourcePath.split(".").pop().toLowerCase());
      for (const segment of fileSegments) {
        blocks.push(renderDesignCodeSegmentPanel(sourcePath, content, segment, segment.language || fallbackLanguage, status));
      }
    }
    showCodePopover(title, blocks.join(""), "", "code-popover-file");
  }

  function renderDesignCodeSegmentPanel(sourcePath, content, segment, languageName, elementStatus) {
    const parts = getDesignCodeSegmentLines(content, segment);
    const publicBody = parts.publicLines.map(line => {
      return renderFullFileLine(line.lineNumber, line.content, getDesignCodeSegmentLineClass(segment, elementStatus, line.lineNumber), languageName);
    }).join("");
    const privateBody = parts.privateLines.map(line => {
      return renderFullFileLine(line.lineNumber, line.content, getDesignCodeSegmentLineClass(segment, elementStatus, line.lineNumber), languageName);
    }).join("");
    if (!publicBody && !privateBody) return "";
    return `
      <section class="code-segment-panel">
        <div class="code-segment-label">${escapeHtml(getDesignCodeSegmentLabel(sourcePath, segment))}</div>
        <div class="full-file-lines full-file-lines-embedded">${publicBody}${renderPrivateCodeReveal(privateBody)}</div>
      </section>
    `;
  }

  function getDesignCodeSegmentLines(content, segment) {
    const lines = String(content || "").split(/\r?\n/);
    const start = Math.max(1, Number(segment.lineStart) || 1);
    const end = Math.min(lines.length, Number(segment.lineEnd) || start);
    const markerIndex = lines.findIndex(isPrivateCodeMarkerLine);
    const markerLine = markerIndex >= 0 ? markerIndex + 1 : 0;
    const publicLines = [];
    const privateLines = [];
    if (end < start) return { publicLines, privateLines };

    for (let lineNumber = start; lineNumber <= end; lineNumber += 1) {
      if (lineNumber === markerLine) continue;
      const line = {
        lineNumber,
        content: lines[lineNumber - 1] || ""
      };
      if (markerLine > 0 && lineNumber > markerLine) privateLines.push(line);
      else publicLines.push(line);
    }
    return { publicLines, privateLines };
  }

  function getDesignCodeSegmentLabel(sourcePath, segment) {
    return `${sourcePath}:${segment.lineStart}-${segment.lineEnd}`;
  }

  function getDesignCodeSegmentLineClass(segment, elementStatus, lineNumber) {
    const status = normalizeStatus(segment.changeStatus || elementStatus);
    if (status === "added" && lineNumber >= segment.lineStart && lineNumber <= segment.lineEnd) return "full-line-add";
    if (isLineInCodeSegmentRanges(lineNumber, segment.addedLines)) return "full-line-add";
    if (isLineInCodeSegmentRanges(lineNumber, segment.modifiedLines)) return "full-line-modified";
    return "full-line-existing";
  }

  function isLineInCodeSegmentRanges(lineNumber, ranges) {
    return (ranges || []).some(range => lineNumber >= range.lineStart && lineNumber <= range.lineEnd);
  }

  function showCodePopover(title, contentHtml, languageName, contentClass = "code-popover-code") {
    const backdrop = document.getElementById("code-popover-backdrop");
    const popover = document.getElementById("code-popover");
    const codeEl = document.getElementById("code-popover-content");
    document.getElementById("code-popover-title").textContent = title;
    codeEl.className = `code-popover-content ${contentClass}`;
    codeEl.removeAttribute("data-highlighted");
    if (languageName) codeEl.classList.add(`language-${languageName}`);
    codeEl.innerHTML = languageName ? renderHighlightedSourceWithPrivateReveal(contentHtml, languageName) : contentHtml;
    backdrop.style.display = "block";
    popover.style.display = "flex";
  }

  async function showFilePopover(path) {
    const entry = await ensureFileContent(path);
    const change = getChange(path) || { diff_lines: [], hunks: [] };
    const ext = path.split(".").pop().toLowerCase();
    const backdrop = document.getElementById("code-popover-backdrop");
    const popover = document.getElementById("code-popover");
    const codeEl = document.getElementById("code-popover-content");
    document.getElementById("code-popover-title").textContent = path;
    codeEl.className = "code-popover-content code-popover-file";
    codeEl.removeAttribute("data-highlighted");
    codeEl.innerHTML = renderFullFilePanel(path, (entry && entry.content) || "", change, ext, { embedded: true, includeDeleted: false });
    backdrop.style.display = "block";
    popover.style.display = "flex";
  }

  function hideCodePopover() {
    document.getElementById("code-popover-backdrop").style.display = "none";
    document.getElementById("code-popover").style.display = "none";
  }

  function isCodePopoverVisible() {
    return document.getElementById("code-popover").style.display !== "none";
  }

  function handleCodePopoverOutsidePointer(evt) {
    if (!isCodePopoverVisible()) return;
    const popover = document.getElementById("code-popover");
    if (popover && popover.contains(evt.target)) return;
    // Exempt the currently-selected file's tree node to avoid a close-open flicker.
    if (isSelectedTreeFilePointerTarget(evt.target)) return;
    if (popover) hideCodePopover();
  }

  function isSelectedTreeFilePointerTarget(target) {
    if (overviewMode === "star") return false;
    if (!target || typeof target.closest !== "function") return false;
    const item = target.closest(".tree-item.file");
    if (!item) return false;
    return isSelectedTreeFile(item.dataset.path);
  }

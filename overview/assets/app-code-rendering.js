"use strict";
    function renderFullFilePanel(path, content, change, ext, options = {}) {
      const markers = collectDiffMarkers(change.diff_lines || []);
      const includeDeleted = options.includeDeleted !== false;
      const lines = content.split(/\r?\n/);
      const languageName = getLanguageFromExt(ext);
      const body = [];

      for (let i = 0; i < lines.length; i++) {
        const lineNumber = i + 1;
        if (includeDeleted) body.push(renderDeletedLines(markers.deletedBefore.get(lineNumber), languageName));
        body.push(renderFullFileLine(lineNumber, lines[i], getFullFileLineClass(markers, lineNumber), languageName));
      }

      if (includeDeleted) body.push(renderDeletedLines(markers.deletedBefore.get(lines.length + 1), languageName));

      if (options.embedded) {
        return `<div class="full-file-lines full-file-lines-embedded">${body.join("")}</div>`;
      }

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
      const modifiedLines = new Set();
      const deletedBefore = new Map();
      let oldLine = 0;
      let newLine = 0;
      let pendingDeletedReplacement = false;

      for (const line of diffLines) {
        if (line.startsWith("@@")) {
          const match = line.match(/@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
          if (match) {
            oldLine = Number(match[1]);
            newLine = Number(match[2]);
          }
          pendingDeletedReplacement = false;
          continue;
        }

        if (line.startsWith("+")) {
          if (pendingDeletedReplacement) {
            modifiedLines.add(newLine);
          } else {
            addedLines.add(newLine);
          }
          newLine += 1;
        } else if (line.startsWith("-")) {
          const bucket = deletedBefore.get(newLine) || [];
          bucket.push(line.substring(1));
          deletedBefore.set(newLine, bucket);
          oldLine += 1;
          pendingDeletedReplacement = true;
        } else if (line.startsWith(" ")) {
          oldLine += 1;
          newLine += 1;
          pendingDeletedReplacement = false;
        }
      }

      return { addedLines, modifiedLines, deletedBefore };
    }

    function renderFullFileLegend() {
      return `
        <div class="full-file-legend">
          <span class="legend-item legend-added">${escapeHtml(t("addedLine"))}</span>
          <span class="legend-item legend-modified">${escapeHtml(t("modifiedLine"))}</span>
          <span class="legend-item legend-existing">${escapeHtml(t("existingLine"))}</span>
          <span class="legend-item legend-deleted">${escapeHtml(t("deletedLine"))}</span>
        </div>
      `;
    }

    function getFullFileLineClass(markers, lineNumber) {
      if (markers.addedLines.has(lineNumber)) return "full-line-add";
      if (markers.modifiedLines.has(lineNumber)) return "full-line-modified";
      return "full-line-existing";
    }

    function renderFullFileLine(lineNumber, content, lineClass, languageName) {
      return `
        <div class="full-file-line ${lineClass}">
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
      return highlightSource(content, languageName);
    }

  function highlightSource(content, languageName) {
    if (languageName === "rust") {
      return String(content || "").split(/\r?\n/).map(highlightRustLine).join("\n");
    }

      if (languageName && window.hljs && hljs.getLanguage(languageName)) {
        try {
          return hljs.highlight(content, { language: languageName, ignoreIllegals: true }).value;
        } catch (e) {
          return escapeHtml(content);
        }
      }
      return escapeHtml(content);
  }

  function highlightRustLine(line) {
    const commentIndex = line.indexOf("//");
    const code = commentIndex >= 0 ? line.slice(0, commentIndex) : line;
    const comment = commentIndex >= 0 ? line.slice(commentIndex) : "";
    const tokens = tokenizeRustCode(code);
    const rendered = tokens.map((token, index) => renderRustToken(token, tokens, index)).join("");
    return comment ? `${rendered}<span class="mx-code-comment">${escapeHtml(comment)}</span>` : rendered;
  }

  function tokenizeRustCode(code) {
    const tokens = [];
    const pattern = /(\s+)|("(?:\\.|[^"\\])*")|('(?:\\.|[^'\\])')|('[A-Za-z_][A-Za-z0-9_]*)|([A-Za-z_][A-Za-z0-9_]*)|(::|->|=>|[{}()[\],.:;<>+\-*/=&|!?])/g;
    let cursor = 0;
    let match;
    while ((match = pattern.exec(code)) !== null) {
      if (match.index > cursor) {
        tokens.push({ kind: "text", value: code.slice(cursor, match.index) });
      }
      const value = match[0];
      if (match[1]) tokens.push({ kind: "space", value });
      else if (match[2] || match[3]) tokens.push({ kind: "string", value });
      else if (match[4]) tokens.push({ kind: "lifetime", value });
      else if (match[5]) tokens.push({ kind: "identifier", value });
      else tokens.push({ kind: "punctuation", value });
      cursor = pattern.lastIndex;
    }
    if (cursor < code.length) {
      tokens.push({ kind: "text", value: code.slice(cursor) });
    }
    return tokens;
  }

  function renderRustToken(token, tokens, index) {
    if (token.kind === "space" || token.kind === "text") return escapeHtml(token.value);
    if (token.kind === "string") return `<span class="mx-code-string">${escapeHtml(token.value)}</span>`;
    if (token.kind === "lifetime") return `<span class="mx-code-lifetime">${escapeHtml(token.value)}</span>`;
    if (token.kind === "punctuation") return `<span class="mx-code-punctuation">${escapeHtml(token.value)}</span>`;
    if (token.kind !== "identifier") return escapeHtml(token.value);

    const value = token.value;
    if (value === "pub") return `<span class="mx-code-visibility">${escapeHtml(value)}</span>`;
    if (RUST_PRIMITIVES.has(value)) return `<span class="mx-code-primitive">${escapeHtml(value)}</span>`;
    if (nextPunctuation(tokens, index) === "!") return `<span class="mx-code-macro">${escapeHtml(value)}</span>`;
    if (RUST_KEYWORDS.has(value)) return `<span class="mx-code-keyword">${escapeHtml(value)}</span>`;
    if (previousIdentifier(tokens, index) === "fn") return `<span class="mx-code-function">${escapeHtml(value)}</span>`;
    if (nextPunctuation(tokens, index) === "(" && !/^[A-Z]/.test(value)) return `<span class="mx-code-function">${escapeHtml(value)}</span>`;
    if (/^[A-Z]/.test(value)) return `<span class="mx-code-type">${escapeHtml(value)}</span>`;
    if (nextPunctuation(tokens, index) === ":") return `<span class="mx-code-variable">${escapeHtml(value)}</span>`;
    return `<span class="mx-code-variable">${escapeHtml(value)}</span>`;
  }

  function previousIdentifier(tokens, index) {
    for (let i = index - 1; i >= 0; i--) {
      if (tokens[i].kind === "space") continue;
      return tokens[i].kind === "identifier" ? tokens[i].value : "";
    }
    return "";
  }

  function nextPunctuation(tokens, index) {
    for (let i = index + 1; i < tokens.length; i++) {
      if (tokens[i].kind === "space") continue;
      return tokens[i].kind === "punctuation" ? tokens[i].value : "";
    }
    return "";
  }

  const RUST_KEYWORDS = new Set([
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
    "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
    "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
    "trait", "true", "type", "unsafe", "use", "where", "while"
  ]);

  const RUST_PRIMITIVES = new Set([
    "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize",
    "str", "u8", "u16", "u32", "u64", "u128", "usize"
  ]);

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

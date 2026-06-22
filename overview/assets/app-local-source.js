"use strict";
  async function fetchLocalRepositoryTree(rootHandle) {
    const files = [];
    await collectLocalFiles(rootHandle, "", files);
    return files;
  }

  async function collectLocalFiles(directoryHandle, prefix, files) {
    for await (const [name, handle] of directoryHandle.entries()) {
      const path = prefix ? `${prefix}/${name}` : name;
      if (handle.kind === "directory") {
        if (isExcludedPathPart(name) || name.startsWith(".")) continue;
        await collectLocalFiles(handle, path, files);
      } else if (handle.kind === "file" && !path.split("/").some(part => isExcludedPathPart(part)) && shouldIncludeManifestPath(path)) {
        const file = await handle.getFile();
        files.push({
          path,
          size: file.size,
          localHandle: handle
        });
      }
    }
  }

  async function fetchManifestFilesFromLocal(tree) {
    const files = {};
    for (const item of tree) {
      const entry = {
        size: item.size || 0,
        localHandle: item.localHandle
      };
      if (isDesignDocumentPathName(item.path)) {
        if ((item.size || 0) > MAX_DYNAMIC_FILE_SIZE) {
          entry.content = `[File too large: ${item.size} bytes]`;
        } else {
          try {
            entry.content = await readLocalFileText(item.localHandle);
          } catch (e) {
            logOverviewError(`local design content load failed: ${item.path}`, e);
            entry.content = "[Unable to read file]";
          }
        }
      }
      files[item.path] = entry;
    }
    logOverview("local file metadata loaded", {
      files: Object.keys(files).length,
      preloadedDesignFiles: Object.keys(files).filter(path => isDesignDocumentPathName(path) && files[path].content).length
    });
    return files;
  }

  async function readLocalFileText(fileHandle) {
    const file = await fileHandle.getFile();
    return await file.text();
  }

  async function buildLocalDiffFromLatestTag(rootHandle, localTree) {
    const gitHandle = await rootHandle.getDirectoryHandle(".git");
    const tags = await fetchLocalProjectTags(gitHandle);
    const diff = { prev_tag: null, latest_tag: "local", changes: {} };
    if (tags.length === 0) return diff;

    const latestTag = tags[tags.length - 1];
    diff.prev_tag = latestTag.name;

    const commit = await peelLocalGitCommit(gitHandle, latestTag.sha);
    const treeSha = getLocalCommitTreeSha(commit.content);
    const baseTree = await readLocalGitTree(gitHandle, treeSha);
    const baseFiles = new Map(baseTree
      .filter(item => shouldIncludeVisibleSourcePath(item.path))
      .map(item => [item.path, item]));

    for (const item of localTree.filter(entry => shouldIncludeVisibleSourcePath(entry.path))) {
      const baseItem = baseFiles.get(item.path);
      if (!baseItem) {
        diff.changes[item.path] = await buildLocalAddedChange(item);
        continue;
      }
      if (await isLocalFileModifiedFromGitBlob(item, baseItem.sha)) {
        diff.changes[item.path] = await buildLocalModifiedChange(gitHandle, item, baseItem);
      }
    }

    return diff;
  }

  // Maximum base*current line product allowed for the O(n*m) LCS diff. Beyond
  // this, fall back to status-only so huge/pathological files stay responsive.
  const MAX_LOCAL_DIFF_LINE_PRODUCT = 4000000;

  async function buildLocalAddedChange(item) {
    if ((item.size || 0) > MAX_DYNAMIC_FILE_SIZE) return createSyntheticDiffChange("A");
    let bytes = null;
    try {
      bytes = await readLocalFileBytes(item.localHandle);
    } catch (e) {
      logOverviewError(`local added diff read failed: ${item.path}`, e);
      return createSyntheticDiffChange("A");
    }
    if (bytes.includes(0)) return createSyntheticDiffChange("A");
    const text = new TextDecoder("utf-8").decode(bytes);
    const built = buildAddedUnifiedDiff(text);
    if (built.diff_lines.length === 0) return createSyntheticDiffChange("A");
    return { status: "A", diff_lines: built.diff_lines, hunks: built.hunks };
  }

  async function buildLocalModifiedChange(gitHandle, item, baseItem) {
    if ((item.size || 0) > MAX_DYNAMIC_FILE_SIZE) return createSyntheticDiffChange("M");
    let baseBytes = null;
    let currentBytes = null;
    try {
      const baseObject = await readLocalGitObject(gitHandle, baseItem.sha);
      if (baseObject.type !== "blob") return createSyntheticDiffChange("M");
      baseBytes = baseObject.content;
      currentBytes = await readLocalFileBytes(item.localHandle);
    } catch (e) {
      logOverviewError(`local modified diff read failed: ${item.path}`, e);
      return createSyntheticDiffChange("M");
    }
    if (baseBytes.length > MAX_DYNAMIC_FILE_SIZE) return createSyntheticDiffChange("M");
    if (baseBytes.includes(0) || currentBytes.includes(0)) return createSyntheticDiffChange("M");
    const baseText = new TextDecoder("utf-8").decode(baseBytes);
    const currentText = new TextDecoder("utf-8").decode(currentBytes);
    const built = buildLocalUnifiedDiff(baseText, currentText);
    if (built.diff_lines.length === 0) return createSyntheticDiffChange("M");
    return { status: "M", diff_lines: built.diff_lines, hunks: built.hunks };
  }

  async function readLocalFileBytes(fileHandle) {
    const file = await fileHandle.getFile();
    return new Uint8Array(await file.arrayBuffer());
  }

  // Split text into logical lines, treating CRLF as LF (line endings are
  // discarded), matching how `content.split(/\r?\n/)` numbers lines in the
  // full-file renderer so added-line highlighting stays aligned.
  function splitTextIntoDiffLines(text) {
    return String(text == null ? "" : text).split(/\r?\n/);
  }

  function buildAddedUnifiedDiff(currentText) {
    const lines = splitTextIntoDiffLines(currentText);
    if (lines.length === 0) return { diff_lines: [], hunks: [] };
    const header = `@@ -0,0 +1,${lines.length} @@`;
    const diff_lines = [header, ...lines.map(line => `+${line}`)];
    return { diff_lines, hunks: [{ header, reason: "" }] };
  }

  function buildLocalUnifiedDiff(baseText, currentText) {
    const baseLines = splitTextIntoDiffLines(baseText);
    const currentLines = splitTextIntoDiffLines(currentText);
    if (baseLines.length * currentLines.length > MAX_LOCAL_DIFF_LINE_PRODUCT) {
      return { diff_lines: [], hunks: [] };
    }
    const ops = computeLineDiff(baseLines, currentLines);
    return buildUnifiedDiffHunks(ops, 3);
  }

  // Classic LCS backtrack producing an ordered edit script of
  // { type: ' ' | '-' | '+', text } entries.
  function computeLineDiff(a, b) {
    const n = a.length;
    const m = b.length;
    const dp = [];
    for (let i = 0; i <= n; i++) dp.push(new Uint32Array(m + 1));
    for (let i = n - 1; i >= 0; i--) {
      const row = dp[i];
      const next = dp[i + 1];
      for (let j = m - 1; j >= 0; j--) {
        row[j] = a[i] === b[j] ? next[j + 1] + 1 : Math.max(next[j], row[j + 1]);
      }
    }
    const ops = [];
    let i = 0;
    let j = 0;
    while (i < n && j < m) {
      if (a[i] === b[j]) {
        ops.push({ type: " ", text: a[i] });
        i++;
        j++;
      } else if (dp[i + 1][j] >= dp[i][j + 1]) {
        ops.push({ type: "-", text: a[i] });
        i++;
      } else {
        ops.push({ type: "+", text: b[j] });
        j++;
      }
    }
    while (i < n) ops.push({ type: "-", text: a[i++] });
    while (j < m) ops.push({ type: "+", text: b[j++] });
    return ops;
  }

  // Group an edit script into unified-diff hunks with `context` surrounding
  // context lines, merging hunks whose context windows touch.
  function buildUnifiedDiffHunks(ops, context) {
    let oldNo = 1;
    let newNo = 1;
    const items = ops.map(op => {
      const entry = { type: op.type, text: op.text, oldNo, newNo };
      if (op.type === " ") {
        oldNo++;
        newNo++;
      } else if (op.type === "-") {
        oldNo++;
      } else {
        newNo++;
      }
      return entry;
    });

    const ranges = [];
    items.forEach((entry, idx) => {
      if (entry.type === " ") return;
      const start = Math.max(0, idx - context);
      const end = Math.min(items.length - 1, idx + context);
      const last = ranges[ranges.length - 1];
      if (last && start <= last.end + 1) {
        last.end = Math.max(last.end, end);
      } else {
        ranges.push({ start, end });
      }
    });

    const diff_lines = [];
    const hunks = [];
    for (const range of ranges) {
      const slice = items.slice(range.start, range.end + 1);
      const header = buildUnifiedHunkHeader(slice);
      diff_lines.push(header);
      hunks.push({ header, reason: "" });
      for (const entry of slice) diff_lines.push(`${entry.type}${entry.text}`);
    }
    return { diff_lines, hunks };
  }

  function buildUnifiedHunkHeader(slice) {
    let oldCount = 0;
    let newCount = 0;
    for (const entry of slice) {
      if (entry.type !== "+") oldCount++;
      if (entry.type !== "-") newCount++;
    }
    // Starts must equal the first slice entry's line numbers so that
    // `collectDiffMarkers` maps added lines and deletion buckets correctly.
    const oldStart = slice[0].oldNo;
    const newStart = slice[0].newNo;
    return `@@ -${oldStart},${oldCount} +${newStart},${newCount} @@`;
  }

  async function fetchLocalProjectTags(gitHandle) {
    const tags = new Map();
    await collectLocalLooseTags(gitHandle, "refs/tags", "", tags);
    await collectLocalPackedTags(gitHandle, tags);
    const tagPrefix = resolveAliases("{{proj_lower}}_tag_");
    return Array.from(tags.entries())
      .filter(([name]) => name.startsWith(tagPrefix))
      .map(([name, sha]) => ({ name, sha }))
      .sort((a, b) => a.name.localeCompare(b.name));
  }

  async function collectLocalLooseTags(gitHandle, refsPath, prefix, tags) {
    let directory = null;
    try {
      directory = await getLocalGitDirectory(gitHandle, refsPath);
    } catch (e) {
      return;
    }

    for await (const [name, handle] of directory.entries()) {
      const tagName = prefix ? `${prefix}/${name}` : name;
      if (handle.kind === "directory") {
        await collectLocalLooseTags(gitHandle, `${refsPath}/${name}`, tagName, tags);
      } else if (handle.kind === "file") {
        const sha = (await readLocalFileText(handle)).trim();
        if (/^[0-9a-f]{40}$/i.test(sha)) tags.set(tagName, sha.toLowerCase());
      }
    }
  }

  async function collectLocalPackedTags(gitHandle, tags) {
    let text = "";
    try {
      text = await readLocalGitText(gitHandle, "packed-refs");
    } catch (e) {
      return;
    }

    let lastTagName = "";
    for (const line of text.split(/\r?\n/)) {
      if (!line || line.startsWith("#")) continue;
      const peeled = line.match(/^\^([0-9a-f]{40})$/i);
      if (peeled && lastTagName) {
        tags.set(lastTagName, peeled[1].toLowerCase());
        continue;
      }
      const match = line.match(/^([0-9a-f]{40})\s+refs\/tags\/(.+)$/i);
      if (match) {
        lastTagName = match[2];
        tags.set(lastTagName, match[1].toLowerCase());
      }
    }
  }

  async function peelLocalGitCommit(gitHandle, sha) {
    let object = await readLocalGitObject(gitHandle, sha);
    while (object.type === "tag") {
      const text = new TextDecoder("utf-8").decode(object.content);
      const nextSha = text.match(/^object ([0-9a-f]{40})$/m);
      if (!nextSha) throw new Error(`local tag object missing target: ${sha}`);
      object = await readLocalGitObject(gitHandle, nextSha[1]);
    }
    if (object.type !== "commit") throw new Error(`local tag target is not a commit: ${sha}`);
    return object;
  }

  function getLocalCommitTreeSha(commitContent) {
    const text = new TextDecoder("utf-8").decode(commitContent);
    const match = text.match(/^tree ([0-9a-f]{40})$/m);
    if (!match) throw new Error("local commit has no tree");
    return match[1];
  }

  async function readLocalGitTree(gitHandle, treeSha, prefix = "") {
    const object = await readLocalGitObject(gitHandle, treeSha);
    if (object.type !== "tree") throw new Error(`local object is not a tree: ${treeSha}`);
    const entries = parseLocalGitTreeEntries(object.content);
    const files = [];
    for (const entry of entries) {
      const path = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (entry.mode === "40000") {
        const childFiles = await readLocalGitTree(gitHandle, entry.sha, path);
        files.push(...childFiles);
      } else {
        files.push({ path, sha: entry.sha });
      }
    }
    return files;
  }

  function parseLocalGitTreeEntries(content) {
    const entries = [];
    const decoder = new TextDecoder("utf-8");
    let offset = 0;
    while (offset < content.length) {
      const modeStart = offset;
      while (content[offset] !== 0x20) offset++;
      const mode = decoder.decode(content.slice(modeStart, offset));
      offset++;

      const nameStart = offset;
      while (content[offset] !== 0x00) offset++;
      const name = decoder.decode(content.slice(nameStart, offset));
      offset++;

      const sha = Array.from(content.slice(offset, offset + 20), byte => byte.toString(16).padStart(2, "0")).join("");
      offset += 20;
      entries.push({ mode, name, sha });
    }
    return entries;
  }

  async function readLocalGitObject(gitHandle, sha) {
    const bytes = await readLocalGitBytes(gitHandle, `objects/${sha.slice(0, 2)}/${sha.slice(2)}`);
    const inflated = await inflateLocalGitObject(bytes);
    let headerEnd = 0;
    while (headerEnd < inflated.length && inflated[headerEnd] !== 0x00) headerEnd++;
    if (headerEnd >= inflated.length) throw new Error(`invalid local git object: ${sha}`);

    const header = new TextDecoder("utf-8").decode(inflated.slice(0, headerEnd));
    const [type] = header.split(" ");
    return {
      type,
      content: inflated.slice(headerEnd + 1)
    };
  }

  async function inflateLocalGitObject(bytes) {
    if (!window.DecompressionStream) {
      throw new Error("DecompressionStream is unavailable for local git objects");
    }
    const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream("deflate"));
    return new Uint8Array(await new Response(stream).arrayBuffer());
  }

  async function isLocalFileModifiedFromGitBlob(item, baseSha) {
    const shas = await calculateLocalGitBlobShaCandidates(item.localHandle);
    return !shas.includes(baseSha);
  }

  async function calculateLocalGitBlobShaCandidates(fileHandle) {
    if (!window.crypto || !window.crypto.subtle) {
      throw new Error("Web Crypto is unavailable for local diff hashing");
    }
    const file = await fileHandle.getFile();
    const content = new Uint8Array(await file.arrayBuffer());
    const shas = [await calculateGitBlobShaForBytes(content)];
    const normalized = normalizeTextBytesForGitHash(content);
    if (normalized) {
      const normalizedSha = await calculateGitBlobShaForBytes(normalized);
      if (!shas.includes(normalizedSha)) shas.push(normalizedSha);
    }
    return shas;
  }

  async function calculateGitBlobShaForBytes(content) {
    const header = new TextEncoder().encode(`blob ${content.byteLength}\0`);
    const blob = new Uint8Array(header.byteLength + content.byteLength);
    blob.set(header, 0);
    blob.set(content, header.byteLength);
    const digest = await window.crypto.subtle.digest("SHA-1", blob);
    return Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, "0")).join("");
  }

  function normalizeTextBytesForGitHash(content) {
    if (content.includes(0)) return null;
    const text = new TextDecoder("utf-8", { fatal: false }).decode(content);
    if (!text.includes("\r\n")) return null;
    return new TextEncoder().encode(text.replace(/\r\n/g, "\n"));
  }

  async function readLocalGitText(gitHandle, path) {
    const file = await getLocalGitFile(gitHandle, path);
    return readLocalFileText(file);
  }

  async function readLocalGitBytes(gitHandle, path) {
    const file = await getLocalGitFile(gitHandle, path);
    const blob = await file.getFile();
    return new Uint8Array(await blob.arrayBuffer());
  }

  async function getLocalGitDirectory(gitHandle, path) {
    const parts = path.split("/").filter(Boolean);
    let current = gitHandle;
    for (const part of parts) {
      current = await current.getDirectoryHandle(part);
    }
    return current;
  }

  async function getLocalGitFile(gitHandle, path) {
    const parts = path.split("/").filter(Boolean);
    let current = gitHandle;
    for (let index = 0; index < parts.length - 1; index++) {
      current = await current.getDirectoryHandle(parts[index]);
    }
    return await current.getFileHandle(parts[parts.length - 1]);
  }

  async function requestLocalReadPermission(handle) {
    const options = { mode: "read" };
    if ((await handle.queryPermission(options)) === "granted") return true;
    return (await handle.requestPermission(options)) === "granted";
  }

  async function assertLocalRootReadable(handle) {
    for await (const _ of handle.entries()) {
      return true;
    }
    throw new Error("local folder is empty");
  }

  async function storeLocalRootHandle(handle) {
    const db = await openLocalSourceDb();
    await writeLocalSourceValue(db, LOCAL_ROOT_HANDLE_KEY, handle);
  }

  async function loadLocalRootHandle() {
    const db = await openLocalSourceDb();
    return await readLocalSourceValue(db, LOCAL_ROOT_HANDLE_KEY);
  }

  async function clearCachedLocalSource() {
    localStorage.removeItem(STORAGE_KEYS.dataSource);
    let db = null;
    try {
      db = await openLocalSourceDb();
      await deleteLocalSourceValue(db, LOCAL_ROOT_HANDLE_KEY);
    } catch (e) {
      logOverviewError("local source cache clear failed", e);
    } finally {
      if (db) db.close();
    }
  }

  function openLocalSourceDb() {
    return new Promise((resolve, reject) => {
      if (!window.indexedDB) {
        reject(new Error("IndexedDB unavailable"));
        return;
      }
      const request = window.indexedDB.open(LOCAL_DB_NAME, 1);
      request.onupgradeneeded = () => {
        request.result.createObjectStore(LOCAL_DB_STORE);
      };
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
  }

  function readLocalSourceValue(db, key) {
    return new Promise((resolve, reject) => {
      const request = db.transaction(LOCAL_DB_STORE, "readonly").objectStore(LOCAL_DB_STORE).get(key);
      request.onsuccess = () => resolve(request.result || null);
      request.onerror = () => reject(request.error);
    });
  }

  function writeLocalSourceValue(db, key, value) {
    return new Promise((resolve, reject) => {
      const request = db.transaction(LOCAL_DB_STORE, "readwrite").objectStore(LOCAL_DB_STORE).put(value, key);
      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  }

  function deleteLocalSourceValue(db, key) {
    return new Promise((resolve, reject) => {
      const request = db.transaction(LOCAL_DB_STORE, "readwrite").objectStore(LOCAL_DB_STORE).delete(key);
      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  }

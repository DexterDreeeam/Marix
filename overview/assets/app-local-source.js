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
    let bytes = null;
    try {
      bytes = await readLocalGitBytes(gitHandle, `objects/${sha.slice(0, 2)}/${sha.slice(2)}`);
    } catch (e) {
      // After a `git gc`/repack, tag/commit/tree/blob objects move from loose
      // storage (`objects/<2>/<38>`) into packfiles, so the loose lookup throws
      // NotFoundError. Fall back to reading the object from any packfile.
      if (e && e.name === "NotFoundError") {
        return await readLocalPackedObject(gitHandle, sha);
      }
      throw e;
    }
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

  // ---------------------------------------------------------------------------
  // Packed git object reading.
  //
  // Loose objects carry a `type size\0` header before their zlib stream, but
  // packed objects do NOT: the inflated pack data is the RAW object content.
  // Pack integers in the .idx are BIG-endian; the pack object header size and
  // delta source/target sizes are LITTLE-endian 7-bit varints; the ofs_delta
  // negative offset uses the special `((off + 1) << 7) | low7` encoding.
  // ---------------------------------------------------------------------------

  const PACK_OBJECT_TYPE_NAMES = { 1: "commit", 2: "tree", 3: "blob", 4: "tag" };
  const MAX_PACK_DELTA_DEPTH = 50;

  // Per-gitHandle cache so each pack index and pack file is parsed/read once.
  const localPackCache = new WeakMap();

  function getLocalPackCache(gitHandle) {
    let cache = localPackCache.get(gitHandle);
    if (!cache) {
      cache = { indices: new Map(), packs: new Map(), packNames: null };
      localPackCache.set(gitHandle, cache);
    }
    return cache;
  }

  async function listLocalPackBasenames(gitHandle, cache) {
    if (cache.packNames) return cache.packNames;
    const names = [];
    let dir = null;
    try {
      dir = await getLocalGitDirectory(gitHandle, "objects/pack");
    } catch (e) {
      cache.packNames = names;
      return names;
    }
    for await (const [name, handle] of dir.entries()) {
      if (handle.kind === "file" && name.endsWith(".idx")) {
        names.push(name.slice(0, -4));
      }
    }
    cache.packNames = names;
    return names;
  }

  async function getParsedLocalPackIndex(gitHandle, cache, basename) {
    if (cache.indices.has(basename)) return cache.indices.get(basename);
    const bytes = await readLocalGitBytes(gitHandle, `objects/pack/${basename}.idx`);
    const parsed = parsePackIndexV2(bytes);
    cache.indices.set(basename, parsed);
    return parsed;
  }

  async function getLocalPackBytes(gitHandle, cache, basename) {
    if (cache.packs.has(basename)) return cache.packs.get(basename);
    const bytes = await readLocalGitBytes(gitHandle, `objects/pack/${basename}.pack`);
    cache.packs.set(basename, bytes);
    return bytes;
  }

  async function readLocalPackedObject(gitHandle, sha) {
    const wanted = sha.toLowerCase();
    const cache = getLocalPackCache(gitHandle);
    const basenames = await listLocalPackBasenames(gitHandle, cache);
    for (const basename of basenames) {
      const index = await getParsedLocalPackIndex(gitHandle, cache, basename);
      const offset = index.lookup(wanted);
      if (offset === null || offset === undefined) continue;
      const packBytes = await getLocalPackBytes(gitHandle, cache, basename);
      return await readPackedObjectFromPack(packBytes, offset, gitHandle);
    }
    throw new Error(`local git object not found in loose or packed storage: ${sha}`);
  }

  // Parse a v2 pack index (.idx). Returns { count, lookup(shaHex) -> offset|null }.
  function parsePackIndexV2(bytes) {
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (!(bytes[0] === 0xff && bytes[1] === 0x74 && bytes[2] === 0x4f && bytes[3] === 0x63)) {
      throw new Error("unsupported pack index: bad magic (expected v2)");
    }
    const version = view.getUint32(4, false);
    if (version !== 2) throw new Error(`unsupported pack index version: ${version}`);

    const fanout = new Uint32Array(256);
    for (let i = 0; i < 256; i++) fanout[i] = view.getUint32(8 + i * 4, false);
    const count = fanout[255];

    const shaTableStart = 8 + 256 * 4;
    const crcTableStart = shaTableStart + count * 20;
    const offsetTableStart = crcTableStart + count * 4;
    const largeOffsetTableStart = offsetTableStart + count * 4;

    function compareShaAt(index, shaHex) {
      const base = shaTableStart + index * 20;
      for (let i = 0; i < 20; i++) {
        const want = parseInt(shaHex.substr(i * 2, 2), 16);
        const have = bytes[base + i];
        if (have !== want) return have - want;
      }
      return 0;
    }

    function offsetAt(index) {
      const raw = view.getUint32(offsetTableStart + index * 4, false);
      if (raw & 0x80000000) {
        const largeIndex = raw & 0x7fffffff;
        const hi = view.getUint32(largeOffsetTableStart + largeIndex * 8, false);
        const lo = view.getUint32(largeOffsetTableStart + largeIndex * 8 + 4, false);
        return hi * 0x100000000 + lo;
      }
      return raw;
    }

    function lookup(shaHex) {
      const hex = String(shaHex).toLowerCase();
      if (!/^[0-9a-f]{40}$/.test(hex)) return null;
      const first = parseInt(hex.substr(0, 2), 16);
      let lo = first === 0 ? 0 : fanout[first - 1];
      let hi = fanout[first];
      while (lo < hi) {
        const mid = (lo + hi) >> 1;
        const cmp = compareShaAt(mid, hex);
        if (cmp === 0) return offsetAt(mid);
        if (cmp < 0) lo = mid + 1;
        else hi = mid;
      }
      return null;
    }

    return { count, lookup };
  }

  // Read and fully resolve a single object at `offset` within a packfile.
  async function readPackedObjectFromPack(packBytes, offset, gitHandle, depth = 0) {
    if (depth > MAX_PACK_DELTA_DEPTH) throw new Error("pack delta chain too deep");

    let pos = offset;
    let b = packBytes[pos++];
    const type = (b >> 4) & 0x7;
    // Object header size varint (little-endian 7-bit), first byte holds 4 bits.
    let size = b & 0x0f;
    let shift = 4;
    while (b & 0x80) {
      b = packBytes[pos++];
      size |= (b & 0x7f) << shift;
      shift += 7;
    }

    if (type >= 1 && type <= 4) {
      const content = await inflatePackedZlib(packBytes, pos, size);
      return { type: PACK_OBJECT_TYPE_NAMES[type], content };
    }

    if (type === 6) {
      // OFS_DELTA: base is at (thisObjectOffset - negativeOffset) in same pack.
      let c = packBytes[pos++];
      let negOffset = c & 0x7f;
      while (c & 0x80) {
        c = packBytes[pos++];
        negOffset = ((negOffset + 1) << 7) | (c & 0x7f);
      }
      const baseOffset = offset - negOffset;
      const base = await readPackedObjectFromPack(packBytes, baseOffset, gitHandle, depth + 1);
      const delta = await inflatePackedZlib(packBytes, pos, size);
      return { type: base.type, content: applyGitDelta(base.content, delta) };
    }

    if (type === 7) {
      // REF_DELTA: base is referenced by its 20-byte sha (loose or packed).
      const baseShaHex = Array.from(packBytes.subarray(pos, pos + 20), byte => byte.toString(16).padStart(2, "0")).join("");
      pos += 20;
      const base = await readLocalGitObject(gitHandle, baseShaHex);
      const delta = await inflatePackedZlib(packBytes, pos, size);
      return { type: base.type, content: applyGitDelta(base.content, delta) };
    }

    throw new Error(`unsupported pack object type: ${type}`);
  }

  // Inflate the zlib stream starting at `start` inside `packBytes`. Pack objects
  // are stored back-to-back, so the input always has trailing bytes (the rest of
  // the pack) after the stream end. Some `DecompressionStream` implementations
  // reject that trailing junk, so we bound output by the known uncompressed
  // `expectedSize` from the pack header and stop as soon as we have enough.
  async function inflatePackedZlib(packBytes, start, expectedSize) {
    const Ctor = (window && window.DecompressionStream) || (typeof DecompressionStream !== "undefined" ? DecompressionStream : null);
    if (!Ctor) throw new Error("DecompressionStream is unavailable for packed git objects");
    if (expectedSize === 0) return new Uint8Array(0);

    const ds = new Ctor("deflate");
    const writer = ds.writable.getWriter();
    const reader = ds.readable.getReader();
    const out = new Uint8Array(expectedSize);
    let outLen = 0;

    const readAll = (async () => {
      try {
        while (outLen < expectedSize) {
          const { value, done } = await reader.read();
          if (done) break;
          if (value && value.length) {
            const take = Math.min(value.length, expectedSize - outLen);
            out.set(value.subarray(0, take), outLen);
            outLen += value.length;
          }
        }
      } catch (e) {
        // Trailing-junk errors surface only after all real output is emitted; if
        // we already have the full object, the error is benign.
        if (outLen < expectedSize) throw e;
      }
    })();

    try {
      await writer.write(packBytes.subarray(start));
      await writer.close();
    } catch (e) {
      // The writable side may also reject on trailing junk after output flushed.
    }
    await readAll;

    if (outLen < expectedSize) throw new Error(`packed object inflate produced ${outLen} of ${expectedSize} bytes`);
    return out;
  }

  // Apply a git delta (`delta`) against a base object (`base`). Both Uint8Array.
  function applyGitDelta(base, delta) {
    let pos = 0;
    function readVarint() {
      let result = 0;
      let shift = 0;
      let byte;
      do {
        byte = delta[pos++];
        result |= (byte & 0x7f) << shift;
        shift += 7;
      } while (byte & 0x80);
      return result >>> 0;
    }

    const sourceSize = readVarint();
    if (sourceSize !== base.length) {
      throw new Error(`git delta source size mismatch: ${sourceSize} != ${base.length}`);
    }
    const targetSize = readVarint();
    const out = new Uint8Array(targetSize);
    let outPos = 0;

    while (pos < delta.length) {
      const op = delta[pos++];
      if (op & 0x80) {
        // COPY from base.
        let copyOffset = 0;
        let copySize = 0;
        if (op & 0x01) copyOffset |= delta[pos++];
        if (op & 0x02) copyOffset |= delta[pos++] << 8;
        if (op & 0x04) copyOffset |= delta[pos++] << 16;
        if (op & 0x08) copyOffset |= delta[pos++] << 24;
        if (op & 0x10) copySize |= delta[pos++];
        if (op & 0x20) copySize |= delta[pos++] << 8;
        if (op & 0x40) copySize |= delta[pos++] << 16;
        copyOffset = copyOffset >>> 0;
        if (copySize === 0) copySize = 0x10000;
        out.set(base.subarray(copyOffset, copyOffset + copySize), outPos);
        outPos += copySize;
      } else if (op !== 0) {
        // INSERT literal bytes from the delta stream.
        out.set(delta.subarray(pos, pos + op), outPos);
        pos += op;
        outPos += op;
      } else {
        throw new Error("invalid git delta opcode 0");
      }
    }

    if (outPos !== targetSize) {
      throw new Error(`git delta produced ${outPos} bytes, expected ${targetSize}`);
    }
    return out;
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

  function getLocalRootHandleKey(localPath) {
    return `root:${normalizeWindowsLocalPath(localPath)}`;
  }

  async function storeLocalRootHandle(localPath, handle) {
    localRouteHandles.set(normalizeWindowsLocalPath(localPath), handle);
    if (!window.indexedDB) return;
    let db = null;
    try {
      db = await openLocalSourceDb();
      await writeLocalSourceValue(db, getLocalRootHandleKey(localPath), handle);
    } catch (e) {
      logOverviewError("local source handle persistence failed", e);
    } finally {
      if (db) db.close();
    }
  }

  async function loadLocalRootHandle(localPath) {
    if (!window.indexedDB) return null;
    let db = null;
    try {
      db = await openLocalSourceDb();
      return await readLocalSourceValue(db, getLocalRootHandleKey(localPath));
    } finally {
      if (db) db.close();
    }
  }

  async function deleteLocalRootHandle(localPath) {
    localRouteHandles.delete(normalizeWindowsLocalPath(localPath));
    if (!window.indexedDB) return;
    let db = null;
    try {
      db = await openLocalSourceDb();
      await deleteLocalSourceValue(db, getLocalRootHandleKey(localPath));
    } catch (e) {
      logOverviewError("local source handle delete failed", e);
    } finally {
      if (db) db.close();
    }
  }

  async function clearCachedLocalSource() {
    localRouteHandles.clear();
    if (!window.indexedDB) return;
    let db = null;
    try {
      db = await openLocalSourceDb();
      await clearLocalSourceValues(db);
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

  function clearLocalSourceValues(db) {
    return new Promise((resolve, reject) => {
      const request = db.transaction(LOCAL_DB_STORE, "readwrite").objectStore(LOCAL_DB_STORE).clear();
      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  }

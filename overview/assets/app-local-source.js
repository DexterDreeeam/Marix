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

"use strict";
  function getChange(path) {
    return ((manifest.diff || {}).changes || {})[path]
      || getDesignStatusChanges().files[path]
      || getDirectAddedFolderDesignChange(path)
      || null;
  }

  function normalizeChangeStatus(status) {
    const key = String(status || "").trim();
    const values = {
      A: "added",
      M: "modified",
      R: "renamed",
      D: "deleted",
      added: "added",
      modified: "modified",
      renamed: "renamed",
      deleted: "deleted",
      removed: "deleted",
      changed: "modified",
      unchanged: "unchanged"
    };
    return values[key] || values[key.toLowerCase()] || "unchanged";
  }

  function getPathChangeStatus(path) {
    const change = getChange(path);
    return change ? normalizeChangeStatus(change.status) : "unchanged";
  }

  function isPathChanged(path) {
    return getPathChangeStatus(path) !== "unchanged";
  }

  function getChangedVisiblePaths() {
    const paths = new Set([
      ...Object.keys(((manifest.diff || {}).changes || {})),
      ...Object.keys(getDesignStatusChanges().files),
      ...getDesignChangedFolderFiles()
    ]);
    return Array.from(paths)
      .filter(path => shouldIncludeVisibleSourcePath(path))
      .filter(path => {
        const status = getPathChangeStatus(path);
        return status !== "unchanged" && status !== "deleted";
      });
  }

  function getDesignStatusChanges() {
    if (designStatusChanges) return designStatusChanges;

    const changes = { files: {}, folders: {} };
    for (const [designPath, data] of Object.entries(manifest.files || {})) {
      if (!designPath.endsWith("/.design.json") || !data || !data.content) continue;

      let document = null;
      try {
        document = JSON.parse(String(data.content || "").trim());
      } catch (error) {
        logOverviewError(`design status load failed: ${designPath}`, error);
        continue;
      }

      const modulePath = (document.module && document.module.path) || designPath.replace(/\/\.design\.json$/i, "");
      collectDesignStatusEntries(changes, modulePath, document, "added");
      collectDesignStatusEntries(changes, modulePath, document, "modified");
      collectDesignStatusEntries(changes, modulePath, document, "deleted");
      collectDesignStatusEntries(changes, modulePath, document, "renamed");
    }

    designStatusChanges = changes;
    return changes;
  }

  function collectDesignStatusEntries(changes, modulePath, document, status) {
    const entries = Array.isArray(document[status]) ? document[status] : [];
    for (const entry of entries) {
      const name = String(entry || "").trim();
      if (!name) continue;
      if (name === ".") {
        changes.folders[modulePath] = createSyntheticDesignChange(status);
      } else if (!name.includes("/") && !name.includes("\\")) {
        changes.files[`${modulePath}/${name}`] = createSyntheticDesignChange(status);
      }
    }
  }

  function getDirectAddedFolderDesignChange(path) {
    const parentPath = getPathParent(path);
    const change = getDesignStatusChanges().folders[parentPath] || null;
    return normalizeChangeStatus(change && change.status) === "added" ? change : null;
  }

  function getDesignChangedFolderFiles() {
    const files = [];
    for (const [folderPath, change] of Object.entries(getDesignStatusChanges().folders)) {
      const status = normalizeChangeStatus(change.status);
      if (status !== "added") continue;
      for (const path of Object.keys(manifest.files || {})) {
        if (shouldIncludeVisibleSourcePath(path) && getPathParent(path) === folderPath) {
          files.push(path);
        }
      }
    }
    return files;
  }

  function getFolderChangeStatus(path) {
    const change = getDesignStatusChanges().folders[path] || null;
    return normalizeChangeStatus(change && change.status);
  }

  function getPathParent(path) {
    const index = String(path || "").lastIndexOf("/");
    return index >= 0 ? path.slice(0, index) : "";
  }

  function createSyntheticDesignChange(status) {
    return {
      status,
      diff_lines: [],
      hunks: []
    };
  }

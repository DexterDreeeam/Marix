"use strict";
  const c_telemetryCredentialDirectory = ".credential";
  const c_telemetryHostFilename = "SERVER_IP.txt";
  const c_telemetryPortFilename = "SERVER_PORT_TELEMETRY_HTTP.txt";
  const c_telemetryStateIdle = "idle";
  const c_telemetryStateLoading = "loading";
  const c_telemetryStateLocalOnly = "local-only";
  const c_telemetryStateUnreadable = "unreadable";
  const c_telemetryStateReady = "ready";
  let s_telemetryPortalUrl = null;
  let s_telemetryPortalState = c_telemetryStateIdle;
  let s_telemetryPortalGeneration = 0;

  function clearTelemetryPortalState(sourceKind = "") {
    s_telemetryPortalGeneration += 1;
    s_telemetryPortalUrl = null;
    if (sourceKind === DATA_SOURCE_LOCAL) {
      s_telemetryPortalState = c_telemetryStateLoading;
    } else if (sourceKind === DATA_SOURCE_GITHUB) {
      s_telemetryPortalState = c_telemetryStateLocalOnly;
    } else {
      s_telemetryPortalState = c_telemetryStateIdle;
    }
    updateTelemetryPortalButton();
  }

  async function prepareTelemetryPortal(sourceKind, rootHandle) {
    const _generation = ++s_telemetryPortalGeneration;
    s_telemetryPortalUrl = null;
    s_telemetryPortalState = sourceKind === DATA_SOURCE_LOCAL
      ? c_telemetryStateLoading
      : c_telemetryStateLocalOnly;
    updateTelemetryPortalButton();

    if (sourceKind !== DATA_SOURCE_LOCAL) return;
    if (!rootHandle) {
      setTelemetryPortalUnavailable(_generation);
      return;
    }

    let _url = null;
    try {
      _url = await readTelemetryPortalUrl(rootHandle);
    } catch {
      setTelemetryPortalUnavailable(_generation);
      return;
    }
    if (_generation !== s_telemetryPortalGeneration) return;

    s_telemetryPortalUrl = _url;
    s_telemetryPortalState = c_telemetryStateReady;
    updateTelemetryPortalButton();
  }

  function setTelemetryPortalUnavailable(generation) {
    if (generation !== s_telemetryPortalGeneration) return;
    s_telemetryPortalUrl = null;
    s_telemetryPortalState = c_telemetryStateUnreadable;
    updateTelemetryPortalButton();
  }

  async function readTelemetryPortalUrl(rootHandle) {
    const _credentialDirectory = await rootHandle.getDirectoryHandle(c_telemetryCredentialDirectory);
    const [_hostHandle, _portHandle] = await Promise.all([
      _credentialDirectory.getFileHandle(c_telemetryHostFilename),
      _credentialDirectory.getFileHandle(c_telemetryPortFilename)
    ]);
    const [_hostFile, _portFile] = await Promise.all([
      _hostHandle.getFile(),
      _portHandle.getFile()
    ]);
    const [_hostText, _portText] = await Promise.all([
      _hostFile.text(),
      _portFile.text()
    ]);
    return buildTelemetryPortalUrl(_hostText, _portText);
  }

  function buildTelemetryPortalUrl(hostText, portText) {
    const _host = String(hostText || "").trim();
    const _portText = String(portText || "").trim();
    if (!_host || /[\s/\\?#@]/.test(_host)) {
      throw new Error("telemetry host is invalid");
    }
    if (!/^\d+$/.test(_portText)) {
      throw new Error("telemetry port is invalid");
    }

    const _port = Number(_portText);
    if (!Number.isInteger(_port) || _port < 1 || _port > 65535) {
      throw new Error("telemetry port is out of range");
    }

    let _formattedHost = _host;
    if (_host.startsWith("[") || _host.endsWith("]")) {
      if (!/^\[[^\[\]]+\]$/.test(_host)) {
        throw new Error("telemetry host brackets are invalid");
      }
    } else if (_host.includes(":")) {
      _formattedHost = `[${_host}]`;
    }

    const _url = `http://${_formattedHost}:${_port}/`;
    return _url;
  }

  function getTelemetryPortalLabelKey() {
    if (s_telemetryPortalState === c_telemetryStateLoading) return "telemetryPortalLoading";
    if (s_telemetryPortalState === c_telemetryStateLocalOnly) return "telemetryPortalLocalOnly";
    if (s_telemetryPortalState === c_telemetryStateUnreadable) return "telemetryPortalCredentialUnreadable";
    return "telemetryPortalTool";
  }

  function updateTelemetryPortalButton() {
    const telemetryButton = document.getElementById("btn-telemetry-portal");
    if (!telemetryButton) return;
    const _labelKey = getTelemetryPortalLabelKey();
    if (typeof updateActionButton === "function") {
      updateActionButton("btn-telemetry-portal", _labelKey);
    } else if (typeof t === "function") {
      telemetryButton.dataset.tooltip = t(_labelKey);
      telemetryButton.setAttribute("aria-label", t(_labelKey));
    }
    telemetryButton.disabled = !s_telemetryPortalUrl;
  }

  function openTelemetryPortal() {
    if (!s_telemetryPortalUrl) return;
    window.open(s_telemetryPortalUrl, "_blank", "noopener,noreferrer");
  }

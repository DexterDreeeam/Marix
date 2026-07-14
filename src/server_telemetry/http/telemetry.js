import {
  buildLogsUrl,
  c_defaultLimit,
  fetchJson,
  mergeSummaries,
  visibleRange,
} from "./telemetry-data.js";
import { createMessageActions } from "./telemetry-message.js";

const c_keywordDebounceMs = 400;
const c_logRefreshMs = 2000;
const c_sessionRefreshMs = 10000;

const s_state = {
  sessions: [],
  selectedSession: undefined,
  tag: "",
  keyword: "",
  summaries: [],
  nextCursor: null,
  latestRecordId: null,
  sessionItems: new Map(),
  rowElements: new Map(),
  sessionsRequest: null,
  logRequest: null,
  generation: 0,
  initialized: false,
};

const appEl = document.getElementById("app");
const sessionListEl = document.getElementById("session-list");
const tagFilterEl = document.getElementById("tag-filter");
const keywordFilterEl = document.getElementById("keyword-filter");
const statusBannerEl = document.getElementById("status-banner");
const logAreaEl = document.getElementById("log-area");
const logTableEl = document.getElementById("log-table");
const logBodyEl = document.getElementById("log-body");
const emptyStateEl = document.getElementById("empty-state");
const initialLoadingEl = document.getElementById("initial-loading");
const noSessionsEl = document.getElementById("no-sessions-state");

const s_messageActions = createMessageActions(
  {
    app: appEl,
    contextMenu: document.getElementById("log-context-menu"),
    copyAction: document.getElementById("copy-message-action"),
    formatAction: document.getElementById("format-message-action"),
    copyToast: document.getElementById("copy-toast"),
    modalBackdrop: document.getElementById("format-message-backdrop"),
    modal: document.getElementById("format-message-modal"),
    modalClose: document.getElementById("format-message-close"),
    modalEditor: document.getElementById("format-message-editor"),
  },
  {
    fetchRecord: function (_id) {
      return fetchJson("/api/logs/" + encodeURIComponent(_id));
    },
    showError: showError,
    onModalChange: function (_open) {
      if (!_open) {
        requestLogs("incremental");
      }
    },
  }
);

function setLogAreaState(_mode) {
  initialLoadingEl.hidden = _mode !== "loading";
  noSessionsEl.hidden = _mode !== "no-sessions";
  emptyStateEl.hidden = _mode !== "empty";
  logTableEl.hidden = _mode !== "table";
}

function showError(_message) {
  statusBannerEl.textContent = _message;
  statusBannerEl.className = "error";
}

function clearError() {
  statusBannerEl.textContent = "";
  statusBannerEl.className = "";
}

function sessionKey(_sessionId) {
  return _sessionId === null ? "unknown" : _sessionId;
}

function fallbackSessionId(_sessions) {
  var _ordinary = _sessions.find(function (_session) {
    return _session.id !== null;
  });
  return (_ordinary || _sessions[0]).id;
}

function formatTimestamp(_milliseconds) {
  if (_milliseconds === null || _milliseconds === undefined) {
    return "–";
  }
  var _date = new Date(_milliseconds);
  return Number.isNaN(_date.getTime())
    ? String(_milliseconds)
    : _date.toLocaleString();
}

function createSessionItem(_sessionId) {
  var _item = document.createElement("li");
  _item.tabIndex = 0;
  _item.setAttribute("role", "button");
  var _label = document.createElement("span");
  _label.className = "session-label";
  _item.appendChild(_label);
  _item.addEventListener("click", function () {
    selectSession(_sessionId);
  });
  _item.addEventListener("keydown", function (_event) {
    if (_event.key === "Enter" || _event.key === " ") {
      _event.preventDefault();
      selectSession(_sessionId);
    }
  });
  return _item;
}

function renderSessions() {
  var _nextItems = new Map();
  s_state.sessions.forEach(function (_session) {
    var _key = sessionKey(_session.id);
    var _item =
      s_state.sessionItems.get(_key) || createSessionItem(_session.id);
    _item.classList.toggle(
      "selected",
      _key === sessionKey(s_state.selectedSession)
    );
    var _label = _item.querySelector(".session-label");
    var _labelText =
      _session.id === null ? "Unknown" : formatTimestamp(_session.emit_ts);
    _label.textContent = _labelText;
    _item.title = _labelText;
    _nextItems.set(_key, _item);
  });
  Array.from(_nextItems.values()).forEach(function (_item, _index) {
    var _current = sessionListEl.children[_index] || null;
    if (_current !== _item) {
      sessionListEl.insertBefore(_item, _current);
    }
  });
  s_state.sessionItems.forEach(function (_item, _key) {
    if (!_nextItems.has(_key)) {
      _item.remove();
    }
  });
  s_state.sessionItems = _nextItems;
}

function normalizedSource(_source) {
  return ["Host", "Client", "Server"].includes(_source) ? _source : "Server";
}

function tagBadgeClass(_tag) {
  return ["Info", "Warning", "Error", "Debug"].includes(_tag)
    ? "tag-" + _tag.toLowerCase()
    : "tag-debug";
}

function createLogRow(_summary) {
  var _row = document.createElement("tr");
  _row.className = "log-row";
  var _emitCell = document.createElement("td");
  _emitCell.className = "time-cell";
  _emitCell.textContent = formatTimestamp(_summary.emit_ts);
  _row.appendChild(_emitCell);

  var _source = normalizedSource(_summary.source);
  var _sourceCell = document.createElement("td");
  var _sourceBadge = document.createElement("span");
  _sourceBadge.className = "source-badge source-" + _source.toLowerCase();
  _sourceBadge.textContent = _source.charAt(0);
  _sourceCell.appendChild(_sourceBadge);
  _row.appendChild(_sourceCell);

  var _tagCell = document.createElement("td");
  var _tagBadge = document.createElement("span");
  _tagBadge.className = "tag-badge " + tagBadgeClass(_summary.tag);
  _tagBadge.textContent = _summary.tag;
  _tagCell.appendChild(_tagBadge);
  _row.appendChild(_tagCell);

  var _messageCell = document.createElement("td");
  _messageCell.className = "message-cell";
  _messageCell.textContent =
    _summary.message_preview + (_summary.truncated ? "…" : "");
  _row.appendChild(_messageCell);
  _row.addEventListener("contextmenu", function (_event) {
    s_messageActions.openContextMenu(_event, _summary, _row);
  });
  return _row;
}

function spacerRow(_height) {
  var _row = document.createElement("tr");
  _row.className = "virtual-spacer";
  var _cell = document.createElement("td");
  _cell.colSpan = 4;
  _cell.style.height = _height + "px";
  _row.appendChild(_cell);
  return _row;
}

function renderVirtualRows() {
  if (s_state.summaries.length === 0) {
    s_state.rowElements = new Map();
    logBodyEl.replaceChildren();
    setLogAreaState(s_state.initialized ? "empty" : "loading");
    return;
  }
  setLogAreaState("table");
  var _range = visibleRange(
    s_state.summaries.length,
    logAreaEl.scrollTop,
    logAreaEl.clientHeight
  );
  var _nextRows = new Map();
  var _fragment = document.createDocumentFragment();
  _fragment.appendChild(spacerRow(_range.top));
  s_state.summaries.slice(_range.start, _range.end).forEach(function (_summary) {
    var _row =
      s_state.rowElements.get(_summary.id) || createLogRow(_summary);
    _nextRows.set(_summary.id, _row);
    _fragment.appendChild(_row);
  });
  _fragment.appendChild(spacerRow(_range.bottom));
  logBodyEl.replaceChildren(_fragment);
  s_state.rowElements = _nextRows;
}

function abortLogRequest() {
  if (s_state.logRequest) {
    s_state.logRequest.abort();
    s_state.logRequest = null;
  }
}

function resetLogs() {
  abortLogRequest();
  s_state.generation += 1;
  s_state.summaries = [];
  s_state.nextCursor = null;
  s_state.latestRecordId = null;
  s_state.rowElements = new Map();
  logAreaEl.scrollTop = 0;
  setLogAreaState("loading");
  requestLogs("initial");
}

function selectSession(_sessionId) {
  if (
    s_messageActions.isModalOpen() ||
    (s_state.selectedSession !== undefined &&
      sessionKey(s_state.selectedSession) === sessionKey(_sessionId))
  ) {
    return;
  }
  s_state.selectedSession = _sessionId;
  renderSessions();
  resetLogs();
}

async function loadSessions() {
  if (s_state.sessionsRequest || s_messageActions.isModalOpen()) {
    return;
  }
  var _controller = new AbortController();
  s_state.sessionsRequest = _controller;
  try {
    var _sessions = (await fetchJson("/api/sessions", _controller.signal)) || [];
    if (s_state.sessionsRequest !== _controller) {
      return;
    }
    s_state.sessions = _sessions;
    if (_sessions.length === 0) {
      abortLogRequest();
      s_state.selectedSession = undefined;
      s_state.summaries = [];
      renderSessions();
      renderVirtualRows();
      setLogAreaState("no-sessions");
      s_state.initialized = true;
      return;
    }
    var _present =
      s_state.selectedSession !== undefined &&
      _sessions.some(function (_session) {
        return sessionKey(_session.id) === sessionKey(s_state.selectedSession);
      });
    if (!_present) {
      s_state.selectedSession = fallbackSessionId(_sessions);
      renderSessions();
      resetLogs();
    } else {
      renderSessions();
    }
    clearError();
  } catch (_error) {
    if (_error.name !== "AbortError") {
      showError("Failed to load sessions: " + _error.message);
    }
  } finally {
    if (s_state.sessionsRequest === _controller) {
      s_state.sessionsRequest = null;
    }
  }
}

async function requestLogs(_mode) {
  if (
    s_state.logRequest ||
    s_state.selectedSession === undefined ||
    s_messageActions.isModalOpen()
  ) {
    return;
  }
  if (_mode === "before" && !s_state.nextCursor) {
    return;
  }
  if (_mode === "incremental" && s_state.latestRecordId === null) {
    return;
  }
  var _generation = s_state.generation;
  var _controller = new AbortController();
  s_state.logRequest = _controller;
  var _url = buildLogsUrl(
    {
      sessionId: s_state.selectedSession,
      tag: s_state.tag,
      keyword: s_state.keyword,
      limit: c_defaultLimit,
      before: s_state.nextCursor,
      afterId: s_state.latestRecordId,
    },
    _mode
  );
  try {
    var _page = await fetchJson(_url, _controller.signal);
    if (
      s_state.logRequest !== _controller ||
      _generation !== s_state.generation
    ) {
      return;
    }
    if (_mode === "initial") {
      s_state.summaries = (_page && _page.items) || [];
      s_state.nextCursor = _page ? _page.next_cursor : null;
    } else if (_mode === "before") {
      s_state.summaries = mergeSummaries(
        s_state.summaries,
        (_page && _page.items) || []
      );
      s_state.nextCursor = _page ? _page.next_cursor : null;
    } else {
      s_state.summaries = mergeSummaries(
        s_state.summaries,
        (_page && _page.items) || []
      );
    }
    if (
      _mode !== "before" &&
      _page &&
      _page.latest_record_id !== null
    ) {
      s_state.latestRecordId = Math.max(
        s_state.latestRecordId === null ? 0 : s_state.latestRecordId,
        _page.latest_record_id
      );
    }
    s_state.initialized = true;
    clearError();
    renderVirtualRows();
  } catch (_error) {
    if (_error.name !== "AbortError") {
      showError("Failed to load logs: " + _error.message);
    }
  } finally {
    if (s_state.logRequest === _controller) {
      s_state.logRequest = null;
    }
  }
}

var s_keywordTimer = null;
keywordFilterEl.addEventListener("input", function () {
  if (s_keywordTimer !== null) {
    clearTimeout(s_keywordTimer);
  }
  s_keywordTimer = setTimeout(function () {
    s_keywordTimer = null;
    s_state.keyword = keywordFilterEl.value.trim();
    resetLogs();
  }, c_keywordDebounceMs);
});
tagFilterEl.addEventListener("change", function () {
  s_state.tag = tagFilterEl.value;
  resetLogs();
});
logAreaEl.addEventListener("scroll", function () {
  renderVirtualRows();
  var _remaining =
    logAreaEl.scrollHeight - logAreaEl.scrollTop - logAreaEl.clientHeight;
  if (_remaining < 12 * 44) {
    requestLogs("before");
  }
});
window.addEventListener("resize", renderVirtualRows);

setLogAreaState("loading");
loadSessions();
setInterval(loadSessions, c_sessionRefreshMs);
setInterval(function () {
  requestLogs("incremental");
}, c_logRefreshMs);

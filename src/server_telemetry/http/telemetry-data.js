export const c_defaultLimit = 200;
export const c_maxVisibleRows = 120;
export const c_rowHeight = 44;

export function compareSummaries(_left, _right) {
  if (_left.emit_ts !== _right.emit_ts) {
    return _right.emit_ts - _left.emit_ts;
  }
  return _right.id - _left.id;
}

export function mergeSummaries(_current, _incoming) {
  var _byId = new Map();
  _current.forEach(function (_item) {
    _byId.set(_item.id, _item);
  });
  _incoming.forEach(function (_item) {
    _byId.set(_item.id, _item);
  });
  return Array.from(_byId.values()).sort(compareSummaries);
}

export function visibleRange(
  _total,
  _scrollTop,
  _viewportHeight,
  _rowHeight = c_rowHeight
) {
  if (_total === 0) {
    return { start: 0, end: 0, top: 0, bottom: 0 };
  }
  var _overscan = 18;
  var _first = Math.max(0, Math.floor(_scrollTop / _rowHeight) - _overscan);
  var _visible = Math.ceil(_viewportHeight / _rowHeight) + _overscan * 2;
  var _count = Math.min(c_maxVisibleRows, Math.max(1, _visible));
  var _start = Math.min(_first, Math.max(0, _total - _count));
  var _end = Math.min(_total, _start + _count);
  return {
    start: _start,
    end: _end,
    top: _start * _rowHeight,
    bottom: (_total - _end) * _rowHeight,
  };
}

export function buildLogsUrl(_filters, _mode) {
  var _parameters = new URLSearchParams();
  _parameters.set(
    "session_id",
    _filters.sessionId === null ? "unknown" : _filters.sessionId
  );
  _parameters.set("limit", String(_filters.limit || c_defaultLimit));
  if (_filters.tag) {
    _parameters.set("tag", _filters.tag);
  }
  if (_filters.keyword && _filters.keyword.trim()) {
    _parameters.set("keyword", _filters.keyword.trim());
  }
  if (_mode === "before" && _filters.before) {
    _parameters.set("before", _filters.before);
  }
  if (_mode === "incremental" && _filters.afterId !== null) {
    _parameters.set("after_id", String(_filters.afterId));
  }
  return "/api/logs?" + _parameters.toString();
}

export function fetchJson(_url, _signal) {
  return fetch(_url, {
    headers: { Accept: "application/json" },
    signal: _signal,
  }).then(function (_response) {
    return _response.text().then(function (_text) {
      var _payload = null;
      try {
        _payload = _text ? JSON.parse(_text) : null;
      } catch (_parseError) {
        _payload = null;
      }
      if (!_response.ok) {
        var _message =
          (_payload && _payload.error) ||
          "request failed (" + _response.status + ")";
        var _error = new Error(_message);
        _error.status = _response.status;
        throw _error;
      }
      return _payload;
    });
  });
}

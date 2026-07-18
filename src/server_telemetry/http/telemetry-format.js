const c_maxNestedJsonDepth = 32;

export function renderFormattedMessage(_container, _message) {
  _container.textContent = "";
  extractFormattedSegments(_message).forEach(function (_segment) {
    if (_segment.kind === "text") {
      var _textElement = document.createElement("span");
      _textElement.className = "formatted-text";
      _textElement.textContent = _segment.text;
      _container.appendChild(_textElement);
      return;
    }
    _container.appendChild(createJsonSection(_segment.value, 0, false));
  });
}

function countPrecedingBackslashes(_text, _index) {
  var _count = 0;
  for (
    var _cursor = _index - 1;
    _cursor >= 0 && _text[_cursor] === "\\";
    _cursor -= 1
  ) {
    _count += 1;
  }
  return _count;
}

function findBalancedJsonCandidate(_message, _start) {
  var _opening = _message[_start];
  var _expectedClose = _opening === "{" ? "}" : _opening === "[" ? "]" : null;
  if (!_expectedClose) {
    return null;
  }
  var _stack = [_expectedClose];
  var _stringMode = null;
  var _inString = false;
  for (var _index = _start + 1; _index < _message.length; _index += 1) {
    var _character = _message[_index];
    if (_character === "\"") {
      var _slashCount = countPrecedingBackslashes(_message, _index);
      if (_stringMode === null) {
        _stringMode = _slashCount % 2 === 1 ? "escaped" : "direct";
        _inString = true;
      } else if (_stringMode === "direct" && _slashCount % 2 === 0) {
        _inString = !_inString;
      } else if (_stringMode === "escaped" && _slashCount % 4 === 1) {
        _inString = !_inString;
      }
      continue;
    }
    if (_inString) {
      continue;
    }
    if (_character === "{" || _character === "[") {
      _stack.push(_character === "{" ? "}" : "]");
      continue;
    }
    if (_character !== "}" && _character !== "]") {
      continue;
    }
    if (_stack[_stack.length - 1] !== _character) {
      return {
        candidate: _message.slice(_start, _index + 1),
        end: _index,
      };
    }
    _stack.pop();
    if (_stack.length === 0) {
      return {
        candidate: _message.slice(_start, _index + 1),
        end: _index,
      };
    }
  }
  return null;
}

function isJsonContainer(_value) {
  return _value !== null && typeof _value === "object";
}

function decodeEscapedJsonBody(_value) {
  var _body = "";
  for (var _index = 0; _index < _value.length; _index += 1) {
    var _character = _value[_index];
    if (
      _character === "\"" &&
      countPrecedingBackslashes(_value, _index) % 2 === 0
    ) {
      _body += "\\\"";
    } else if (_character.charCodeAt(0) < 0x20) {
      _body += JSON.stringify(_character).slice(1, -1);
    } else {
      _body += _character;
    }
  }
  try {
    return JSON.parse("\"" + _body + "\"");
  } catch (_decodeError) {
    return null;
  }
}

function parseNestedJsonString(_value) {
  if (typeof _value !== "string") {
    return null;
  }
  var _current = _value;
  for (var _depth = 0; _depth < c_maxNestedJsonDepth; _depth += 1) {
    var _candidate = _current.trim();
    if (_candidate.length === 0) {
      return null;
    }
    var _parsed;
    try {
      _parsed = JSON.parse(_candidate);
    } catch (_parseError) {
      _parsed = decodeEscapedJsonBody(_candidate);
      if (_parsed === null) {
        return null;
      }
    }
    if (isJsonContainer(_parsed)) {
      return _parsed;
    }
    if (typeof _parsed !== "string" || _parsed === _candidate) {
      return null;
    }
    _current = _parsed;
  }
  return null;
}

function extractFormattedSegments(_message) {
  var _segments = [];
  var _textStart = 0;
  var _searchStart = 0;
  while (_searchStart < _message.length) {
    var _objectStart = _message.indexOf("{", _searchStart);
    var _arrayStart = _message.indexOf("[", _searchStart);
    var _candidateStart;
    if (_objectStart === -1) {
      _candidateStart = _arrayStart;
    } else if (_arrayStart === -1) {
      _candidateStart = _objectStart;
    } else {
      _candidateStart = Math.min(_objectStart, _arrayStart);
    }
    if (_candidateStart === -1) {
      break;
    }
    var _balanced = findBalancedJsonCandidate(_message, _candidateStart);
    if (!_balanced) {
      _searchStart = _candidateStart + 1;
      continue;
    }
    var _value = parseNestedJsonString(_balanced.candidate);
    if (!_value) {
      _searchStart = _candidateStart + 1;
      continue;
    }
    if (_candidateStart > _textStart) {
      _segments.push({
        kind: "text",
        text: _message.slice(_textStart, _candidateStart),
      });
    }
    _segments.push({ kind: "json", value: _value });
    _textStart = _balanced.end + 1;
    _searchStart = _balanced.end + 1;
  }
  if (_textStart < _message.length || _segments.length === 0) {
    _segments.push({ kind: "text", text: _message.slice(_textStart) });
  }
  return _segments;
}

function appendJsonToken(_container, _text, _className) {
  var _token = document.createElement("span");
  _token.className = _className;
  _token.textContent = _text;
  _container.appendChild(_token);
}

function appendJsonLine(_container, _indent) {
  var _line = document.createElement("div");
  _line.className = "json-line";
  _line.appendChild(document.createTextNode("  ".repeat(_indent)));
  _container.appendChild(_line);
  return _line;
}

function formatJsonString(_value) {
  var _formatted = "\"";
  for (var _index = 0; _index < _value.length; _index += 1) {
    var _character = _value[_index];
    if (_character === "\r") {
      if (_value[_index + 1] === "\n") {
        _index += 1;
      }
      _formatted += "\n";
    } else if (_character === "\n") {
      _formatted += "\n";
    } else {
      _formatted += JSON.stringify(_character).slice(1, -1);
    }
  }
  return _formatted + "\"";
}

function appendJsonScalar(_container, _value) {
  var _className;
  if (typeof _value === "string") {
    _className = "json-string";
  } else if (typeof _value === "number") {
    _className = "json-number";
  } else if (typeof _value === "boolean") {
    _className = "json-boolean";
  } else {
    _className = "json-null";
  }
  appendJsonToken(
    _container,
    typeof _value === "string"
      ? formatJsonString(_value)
      : JSON.stringify(_value),
    _className
  );
}

function appendJsonPrefix(_container, _key) {
  if (_key === null) {
    return;
  }
  appendJsonToken(_container, JSON.stringify(_key), "json-key");
  _container.appendChild(document.createTextNode(": "));
}

function appendJsonValue(
  _container,
  _value,
  _indent,
  _key,
  _trailingComma,
  _sectionDepth
) {
  var _nestedValue =
    typeof _value === "string" && _sectionDepth < c_maxNestedJsonDepth
      ? parseNestedJsonString(_value)
      : null;
  if (_nestedValue) {
    if (_key !== null) {
      var _keyLine = appendJsonLine(_container, _indent);
      appendJsonPrefix(_keyLine, _key);
    }
    _container.appendChild(
      createJsonSection(_nestedValue, _sectionDepth + 1, _trailingComma)
    );
    return;
  }
  var _line = appendJsonLine(_container, _indent);
  appendJsonPrefix(_line, _key);
  if (!isJsonContainer(_value)) {
    appendJsonScalar(_line, _value);
    if (_trailingComma) {
      _line.appendChild(document.createTextNode(","));
    }
    return;
  }
  var _isArray = Array.isArray(_value);
  _line.appendChild(document.createTextNode(_isArray ? "[" : "{"));
  var _entries = _isArray
    ? _value.map(function (_item) {
        return [null, _item];
      })
    : Object.keys(_value).map(function (_entryKey) {
        return [_entryKey, _value[_entryKey]];
      });
  _entries.forEach(function (_entry, _index) {
    appendJsonValue(
      _container,
      _entry[1],
      _indent + 1,
      _entry[0],
      _index < _entries.length - 1,
      _sectionDepth
    );
  });
  var _closingLine = appendJsonLine(_container, _indent);
  _closingLine.appendChild(document.createTextNode(_isArray ? "]" : "}"));
  if (_trailingComma) {
    _closingLine.appendChild(document.createTextNode(","));
  }
}

function createJsonSection(_value, _depth, _trailingComma) {
  var _section = document.createElement("div");
  _section.className = "formatted-json";
  _section.dataset.jsonDepth = String(_depth);
  if (_depth > 0) {
    _section.classList.add("nested-json");
    _section.classList.add(
      _depth % 2 === 0 ? "nested-depth-even" : "nested-depth-odd"
    );
  }
  var _code = document.createElement("div");
  _code.className = "json-code";
  appendJsonValue(_code, _value, 0, null, _trailingComma, _depth);
  _section.appendChild(_code);
  return _section;
}

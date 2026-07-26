"use strict";
  function splitFileViewDiffSections(path, change) {
    const _diffLines = change.diff_lines || [];
    const _sections = splitDiffSections(_diffLines, change.hunks || []);
    if (!String(path || "").toLowerCase().endsWith(".rs")) return _sections;

    const _entry = (manifest.files || {})[path];
    const _currentContent = _entry && typeof _entry.content === "string"
      ? _entry.content
      : null;
    return filterRustModuleWiringSections(_sections, _currentContent);
  }

  function filterRustModuleWiringSections(sections, currentContent) {
    const _sources = buildRustDiffSideSources(sections, currentContent);
    const _oldWiringLines = detectRustModuleWiringLines(_sources.oldSource);
    const _newWiringLines = detectRustModuleWiringLines(_sources.newSource);

    return sections.map(section => {
      return filterRustModuleWiringSection(section, _oldWiringLines, _newWiringLines);
    }).filter(Boolean);
  }

  function buildRustDiffSideSources(sections, currentContent) {
    if (currentContent !== null) {
      return {
        oldSource: reconstructOldRustSource(sections, currentContent),
        newSource: currentContent
      };
    }
    return {
      oldSource: buildKnownRustDiffPrefix(sections, "old"),
      newSource: buildKnownRustDiffPrefix(sections, "new")
    };
  }

  function reconstructOldRustSource(sections, currentContent) {
    const _oldLines = String(currentContent).split(/\r?\n/);
    const _orderedSections = [...sections].sort((left, right) => {
      const _rightHeader = parseRustDiffHeader(right.header);
      const _leftHeader = parseRustDiffHeader(left.header);
      return (_rightHeader ? _rightHeader.newStart : 0) - (_leftHeader ? _leftHeader.newStart : 0);
    });

    for (const _section of _orderedSections) {
      const _header = parseRustDiffHeader(_section.header);
      if (!_header) continue;
      const _replacement = [];
      let _removeCount = 0;
      for (const _line of _section.lines || []) {
        const _marker = _line.slice(0, 1);
        if (_marker === " " || _marker === "-") _replacement.push(_line.slice(1));
        if (_marker === " " || _marker === "+") _removeCount += 1;
      }
      const _index = Math.max(0, _header.newStart - 1);
      _oldLines.splice(_index, _removeCount, ..._replacement);
    }
    return _oldLines.join("\n");
  }

  function buildKnownRustDiffPrefix(sections, side) {
    const _lines = [];
    let _expectedLine = 1;
    for (const _section of sections) {
      const _header = parseRustDiffHeader(_section.header);
      if (!_header) continue;
      const _start = side === "old" ? _header.oldStart : _header.newStart;
      const _sideLines = (_section.lines || [])
        .filter(line => side === "old" ? !line.startsWith("+") : !line.startsWith("-"))
        .map(line => line.slice(1));
      if (_sideLines.length === 0) continue;
      if (_start > _expectedLine) break;
      const _overlap = Math.max(0, _expectedLine - _start);
      _lines.push(..._sideLines.slice(_overlap));
      _expectedLine += Math.max(0, _sideLines.length - _overlap);
    }
    return _lines.join("\n");
  }

  function filterRustModuleWiringSection(section, oldWiringLines, newWiringLines) {
    const _header = parseRustDiffHeader(section.header);
    if (!_header) return section;
    let _oldLine = _header.oldStart;
    let _newLine = _header.newStart;
    const _lines = [];
    const _linePositions = [];

    for (const _line of section.lines || []) {
      const _marker = _line.slice(0, 1);
      const _isOldWiring = _marker !== "+" && oldWiringLines.has(_oldLine);
      const _isNewWiring = _marker !== "-" && newWiringLines.has(_newLine);
      if (!_isOldWiring && !_isNewWiring) {
        _lines.push(_line);
        _linePositions.push({ oldLine: _oldLine, newLine: _newLine });
      }
      if (_marker === " " || _marker === "-") _oldLine += 1;
      if (_marker === " " || _marker === "+") _newLine += 1;
    }

    if (!_lines.some(line => line.startsWith("+") || line.startsWith("-"))) return null;
    return { ...section, lines: _lines, linePositions: _linePositions };
  }

  function parseRustDiffHeader(header) {
    const _match = String(header || "").match(/@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/);
    if (!_match) return null;
    return {
      oldStart: Number(_match[1]),
      oldCount: _match[2] === undefined ? 1 : Number(_match[2]),
      newStart: Number(_match[3]),
      newCount: _match[4] === undefined ? 1 : Number(_match[4])
    };
  }

  function detectRustModuleWiringLines(source) {
    if (!source) return new Set();
    const _lexed = maskRustNonCode(source);
    const _lineStarts = collectRustLineStarts(source);
    const _lines = new Set();
    const _frames = [{ kind: "module", itemStart: 0 }];
    let _parenthesisDepth = 0;
    let _bracketDepth = 0;

    for (let _index = 0; _index < _lexed.masked.length; _index += 1) {
      const _character = _lexed.masked[_index];
      const _frame = _frames[_frames.length - 1];
      if (_character === "(") _parenthesisDepth += 1;
      else if (_character === ")") _parenthesisDepth = Math.max(0, _parenthesisDepth - 1);
      else if (_character === "[") _bracketDepth += 1;
      else if (_character === "]") _bracketDepth = Math.max(0, _bracketDepth - 1);
      else if (_character === "{") {
        const _prefix = _frame.kind === "module"
          ? _lexed.masked.slice(_frame.itemStart, _index)
          : "";
        const _parsed = _frame.kind === "module" && _parenthesisDepth === 0 && _bracketDepth === 0
          ? parseRustWiringPrefix(_prefix, false)
          : null;
        if (_frame.kind === "module" && (_parenthesisDepth > 0 || _bracketDepth > 0)) {
          _frames.push({ kind: "group", itemStart: 0 });
        } else if (_parsed && _parsed.kind === "use") _frames.push({ kind: "group", itemStart: 0 });
        else if (_parsed && _parsed.kind === "mod") _frames.push({ kind: "module", itemStart: _index + 1 });
        else _frames.push({ kind: _frame.kind === "group" ? "group" : "other", itemStart: 0 });
      } else if (_character === "}" && _frames.length > 1) {
        const _closed = _frames.pop();
        const _parent = _frames[_frames.length - 1];
        if (_closed.kind !== "group" && _parent.kind === "module") _parent.itemStart = _index + 1;
      } else if (_character === ";" && _frame.kind === "module"
        && _parenthesisDepth === 0 && _bracketDepth === 0) {
        const _item = _lexed.masked.slice(_frame.itemStart, _index);
        const _parsed = parseRustWiringPrefix(_item, true);
        if (_parsed) {
          const _codeStart = _frame.itemStart + _parsed.startOffset;
          const _rangeStart = findAttachedRustCommentStart(source, _lexed.comments, _codeStart);
          addRustLineRange(_lines, _lineStarts, _rangeStart, _index);
        }
        _frame.itemStart = _index + 1;
      }
    }
    return _lines;
  }

  function parseRustWiringPrefix(text, complete) {
    let _index = 0;
    let _outerAttributeStart = -1;
    while (true) {
      _index = skipRustWhitespace(text, _index);
      const _isInner = text.startsWith("#![", _index);
      const _isOuter = !_isInner && text.startsWith("#[", _index);
      if (!_isInner && !_isOuter) break;
      if (_isOuter && _outerAttributeStart < 0) _outerAttributeStart = _index;
      const _openBracket = text.indexOf("[", _index);
      const _attributeEnd = findRustBalancedEnd(text, _openBracket, "[", "]");
      if (_attributeEnd < 0) return null;
      _index = _attributeEnd + 1;
    }

    _index = skipRustWhitespace(text, _index);
    const _declarationStart = _index;
    if (hasRustWordAt(text, _index, "pub")) {
      _index += 3;
      _index = skipRustWhitespace(text, _index);
      if (text[_index] === "(") {
        const _visibilityEnd = findRustBalancedEnd(text, _index, "(", ")");
        if (_visibilityEnd < 0) return null;
        _index = skipRustWhitespace(text, _visibilityEnd + 1);
      }
    }

    let _kind = "";
    if (hasRustWordAt(text, _index, "use")) {
      _kind = "use";
      _index += 3;
    } else if (hasRustWordAt(text, _index, "mod")) {
      _kind = "mod";
      _index += 3;
    } else {
      return null;
    }

    const _remainder = text.slice(_index).trim();
    if (!_remainder && (complete || _kind !== "use")) return null;
    if (_kind === "mod" && !/^[A-Za-z_][A-Za-z0-9_]*$/.test(_remainder)) return null;
    if (!complete && _kind === "use" && _remainder && !/::$/.test(_remainder)) return null;
    return {
      kind: _kind,
      startOffset: _outerAttributeStart >= 0 ? _outerAttributeStart : _declarationStart
    };
  }

  function skipRustWhitespace(text, index) {
    let _index = index;
    while (_index < text.length && /\s/.test(text[_index])) _index += 1;
    return _index;
  }

  function hasRustWordAt(text, index, word) {
    if (!text.startsWith(word, index)) return false;
    const _before = index > 0 ? text[index - 1] : "";
    const _after = text[index + word.length] || "";
    return !/[A-Za-z0-9_]/.test(_before) && !/[A-Za-z0-9_]/.test(_after);
  }

  function findRustBalancedEnd(text, start, open, close) {
    let _depth = 0;
    for (let _index = start; _index < text.length; _index += 1) {
      if (text[_index] === open) _depth += 1;
      else if (text[_index] === close) {
        _depth -= 1;
        if (_depth === 0) return _index;
      }
    }
    return -1;
  }

  function findAttachedRustCommentStart(source, comments, codeStart) {
    let _start = codeStart;
    for (let _index = comments.length - 1; _index >= 0; _index -= 1) {
      const _comment = comments[_index];
      if (_comment.end > _start) continue;
      const _gap = source.slice(_comment.end, _start);
      if (_gap.trim() || countRustNewlines(_gap) > 1) break;
      const _commentText = source.slice(_comment.start, _comment.end);
      if (_commentText.startsWith("//!") || _commentText.startsWith("/*!")) break;
      const _lineStart = source.lastIndexOf("\n", _comment.start - 1) + 1;
      if (source.slice(_lineStart, _comment.start).trim()) break;
      _start = _comment.start;
    }
    return _start;
  }

  function countRustNewlines(text) {
    return (text.match(/\n/g) || []).length;
  }

  function collectRustLineStarts(source) {
    const _starts = [0];
    for (let _index = 0; _index < source.length; _index += 1) {
      if (source[_index] === "\n") _starts.push(_index + 1);
    }
    return _starts;
  }

  function addRustLineRange(lines, lineStarts, start, end) {
    const _startLine = findRustLineNumber(lineStarts, start);
    const _endLine = findRustLineNumber(lineStarts, end);
    for (let _line = _startLine; _line <= _endLine; _line += 1) lines.add(_line);
  }

  function findRustLineNumber(lineStarts, offset) {
    let _low = 0;
    let _high = lineStarts.length;
    while (_low < _high) {
      const _middle = Math.floor((_low + _high) / 2);
      if (lineStarts[_middle] <= offset) _low = _middle + 1;
      else _high = _middle;
    }
    return Math.max(1, _low);
  }

  function maskRustNonCode(source) {
    const _masked = String(source).split("");
    const _comments = [];
    let _index = 0;
    while (_index < source.length) {
      if (source.startsWith("//", _index)) {
        const _end = source.indexOf("\n", _index);
        const _commentEnd = _end < 0 ? source.length : _end;
        _comments.push({ start: _index, end: _commentEnd });
        maskRustRange(_masked, source, _index, _commentEnd);
        _index = _commentEnd;
        continue;
      }
      if (source.startsWith("/*", _index)) {
        const _commentEnd = findRustBlockCommentEnd(source, _index);
        _comments.push({ start: _index, end: _commentEnd });
        maskRustRange(_masked, source, _index, _commentEnd);
        _index = _commentEnd;
        continue;
      }
      const _rawStringEnd = findRustRawStringEnd(source, _index);
      if (_rawStringEnd > _index) {
        maskRustRange(_masked, source, _index, _rawStringEnd);
        _index = _rawStringEnd;
        continue;
      }
      if (source[_index] === "\"") {
        const _stringEnd = findRustQuotedEnd(source, _index, "\"");
        maskRustRange(_masked, source, _index, _stringEnd);
        _index = _stringEnd;
        continue;
      }
      if (source[_index] === "'") {
        const _characterEnd = findRustCharacterEnd(source, _index);
        if (_characterEnd > _index) {
          maskRustRange(_masked, source, _index, _characterEnd);
          _index = _characterEnd;
          continue;
        }
      }
      _index += 1;
    }
    return { masked: _masked.join(""), comments: _comments };
  }

  function findRustBlockCommentEnd(source, start) {
    let _depth = 1;
    let _index = start + 2;
    while (_index < source.length && _depth > 0) {
      if (source.startsWith("/*", _index)) {
        _depth += 1;
        _index += 2;
      } else if (source.startsWith("*/", _index)) {
        _depth -= 1;
        _index += 2;
      } else {
        _index += 1;
      }
    }
    return _index;
  }

  function findRustRawStringEnd(source, start) {
    const _before = start > 0 ? source[start - 1] : "";
    if (/[A-Za-z0-9_]/.test(_before)) return -1;
    let _index = start;
    if (source.startsWith("br", _index) || source.startsWith("cr", _index)) _index += 2;
    else if (source[_index] === "r") _index += 1;
    else return -1;
    let _hashes = "";
    while (source[_index] === "#") {
      _hashes += "#";
      _index += 1;
    }
    if (source[_index] !== "\"") return -1;
    const _terminator = `"${_hashes}`;
    const _end = source.indexOf(_terminator, _index + 1);
    return _end < 0 ? source.length : _end + _terminator.length;
  }

  function findRustQuotedEnd(source, start, quote) {
    let _index = start + 1;
    while (_index < source.length) {
      if (source[_index] === "\\") _index += 2;
      else if (source[_index] === quote) return _index + 1;
      else _index += 1;
    }
    return source.length;
  }

  function findRustCharacterEnd(source, start) {
    const _end = findRustQuotedEnd(source, start, "'");
    if (_end >= source.length && source[_end - 1] !== "'") return -1;
    const _content = source.slice(start + 1, _end - 1);
    if (_content.startsWith("\\")) return _end;
    return [..._content].length === 1 ? _end : -1;
  }

  function maskRustRange(masked, source, start, end) {
    for (let _index = start; _index < end; _index += 1) {
      if (source[_index] !== "\n" && source[_index] !== "\r") masked[_index] = " ";
    }
  }

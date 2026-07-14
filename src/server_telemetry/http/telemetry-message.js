import { renderFormattedMessage } from "./telemetry-format.js";

const c_cacheLimit = 100;

export function createMessageActions(_elements, _callbacks) {
  var _context = null;
  var _modalOpen = false;
  var _modalPreviousFocus = null;
  var _copyToastTimer = null;
  var _recordCache = new Map();

  function exactMessage(_id) {
    if (_recordCache.has(_id)) {
      return Promise.resolve(_recordCache.get(_id));
    }
    return _callbacks.fetchRecord(_id).then(function (_record) {
      _recordCache.set(_id, _record.message);
      if (_recordCache.size > c_cacheLimit) {
        _recordCache.delete(_recordCache.keys().next().value);
      }
      return _record.message;
    });
  }

  function closeContextMenu(_restoreFocus) {
    if (!_context) {
      return;
    }
    var _previousFocus = _context.previousFocus;
    _context = null;
    _elements.contextMenu.hidden = true;
    _elements.contextMenu.style.left = "";
    _elements.contextMenu.style.top = "";
    if (_restoreFocus && _previousFocus && _previousFocus.isConnected) {
      _previousFocus.focus({ preventScroll: true });
    }
  }

  function openContextMenu(_event, _summary, _row) {
    _event.preventDefault();
    if (_modalOpen) {
      return;
    }
    closeContextMenu(false);
    _context = {
      id: _summary.id,
      row: _row,
      previousFocus: document.activeElement,
    };
    _elements.contextMenu.hidden = false;
    _elements.contextMenu.style.visibility = "hidden";
    _elements.contextMenu.style.left = "0";
    _elements.contextMenu.style.top = "0";
    var _bounds = _elements.contextMenu.getBoundingClientRect();
    var _margin = 8;
    var _left = Math.max(
      _margin,
      Math.min(_event.clientX, window.innerWidth - _bounds.width - _margin)
    );
    var _top = Math.max(
      _margin,
      Math.min(_event.clientY, window.innerHeight - _bounds.height - _margin)
    );
    _elements.contextMenu.style.left = _left + "px";
    _elements.contextMenu.style.top = _top + "px";
    _elements.contextMenu.style.visibility = "";
    _elements.copyAction.focus({ preventScroll: true });
  }

  function showCopyToast() {
    if (_copyToastTimer !== null) {
      clearTimeout(_copyToastTimer);
    }
    _elements.copyToast.classList.add("visible");
    _copyToastTimer = setTimeout(function () {
      _elements.copyToast.classList.remove("visible");
      _copyToastTimer = null;
    }, 1050);
  }

  function copyTextWithExecCommand(_message) {
    var _previousFocus = document.activeElement;
    var _textarea = document.createElement("textarea");
    _textarea.value = _message;
    _textarea.readOnly = true;
    _textarea.style.position = "fixed";
    _textarea.style.left = "-9999px";
    _textarea.style.top = "0";
    _textarea.style.opacity = "0";
    document.body.appendChild(_textarea);
    try {
      _textarea.select();
      _textarea.setSelectionRange(0, _textarea.value.length);
      return document.execCommand("copy") === true;
    } catch (_copyError) {
      return false;
    } finally {
      _textarea.remove();
      if (_previousFocus && typeof _previousFocus.focus === "function") {
        try {
          _previousFocus.focus({ preventScroll: true });
        } catch (_focusError) {
          try {
            _previousFocus.focus();
          } catch (_fallbackFocusError) {
            // Focus restoration is best-effort and does not change copy success.
          }
        }
      }
    }
  }

  function writeClipboardText(_message) {
    if (navigator.clipboard && typeof navigator.clipboard.writeText === "function") {
      try {
        return Promise.resolve(navigator.clipboard.writeText(_message)).then(
          function () {
            return true;
          },
          function () {
            return copyTextWithExecCommand(_message);
          }
        );
      } catch (_clipboardError) {
        return Promise.resolve(copyTextWithExecCommand(_message));
      }
    }
    return Promise.resolve(copyTextWithExecCommand(_message));
  }

  function activateCopyMessage() {
    if (!_context) {
      return;
    }
    var _id = _context.id;
    exactMessage(_id)
      .then(writeClipboardText)
      .then(function (_copied) {
        if (_copied) {
          showCopyToast();
        } else {
          _callbacks.showError("Failed to copy message.");
        }
      })
      .catch(function (_error) {
        _callbacks.showError("Failed to copy message: " + _error.message);
      });
    closeContextMenu(true);
  }

  function openFormatMessage(_message, _row, _previousFocus) {
    _modalOpen = true;
    _modalPreviousFocus =
      _previousFocus || _row.querySelector(".message-cell");
    closeContextMenu(false);
    renderFormattedMessage(_elements.modalEditor, _message);
    _elements.app.setAttribute("inert", "");
    _elements.app.setAttribute("aria-hidden", "true");
    document.body.classList.add("modal-open");
    _elements.modalBackdrop.hidden = false;
    _elements.modalBackdrop.setAttribute("aria-hidden", "false");
    _elements.modalClose.focus({ preventScroll: true });
    _callbacks.onModalChange(true);
  }

  function activateFormatMessage() {
    if (!_context) {
      return;
    }
    var _contextAtRequest = _context;
    exactMessage(_contextAtRequest.id)
      .then(function (_message) {
        openFormatMessage(
          _message,
          _contextAtRequest.row,
          _contextAtRequest.previousFocus
        );
      })
      .catch(function (_error) {
        _callbacks.showError("Failed to load full message: " + _error.message);
        closeContextMenu(true);
      });
  }

  function closeFormatMessage() {
    if (!_modalOpen) {
      return;
    }
    _modalOpen = false;
    _elements.modalBackdrop.hidden = true;
    _elements.modalBackdrop.setAttribute("aria-hidden", "true");
    document.body.classList.remove("modal-open");
    _elements.app.removeAttribute("inert");
    _elements.app.removeAttribute("aria-hidden");
    _elements.modalEditor.textContent = "";
    var _previousFocus = _modalPreviousFocus;
    _modalPreviousFocus = null;
    if (_previousFocus && _previousFocus.isConnected) {
      _previousFocus.focus({ preventScroll: true });
    }
    _callbacks.onModalChange(false);
  }

  function bindAction(_element, _action) {
    _element.addEventListener("click", _action);
    _element.addEventListener("keydown", function (_event) {
      if (_event.key === "Enter" || _event.key === " ") {
        _event.preventDefault();
        _action();
      }
    });
  }

  bindAction(_elements.copyAction, activateCopyMessage);
  bindAction(_elements.formatAction, activateFormatMessage);
  _elements.modalClose.addEventListener("click", closeFormatMessage);
  _elements.modal.addEventListener("keydown", function (_event) {
    if (_event.key !== "Tab") {
      return;
    }
    var _focusable = [_elements.modalClose, _elements.modalEditor];
    var _index = _focusable.indexOf(document.activeElement);
    if (_event.shiftKey && _index <= 0) {
      _event.preventDefault();
      _elements.modalEditor.focus();
    } else if (!_event.shiftKey && _index === _focusable.length - 1) {
      _event.preventDefault();
      _elements.modalClose.focus();
    } else if (_index === -1) {
      _event.preventDefault();
      _elements.modalClose.focus();
    }
  });
  document.addEventListener("pointerdown", function (_event) {
    if (_context && !_elements.contextMenu.contains(_event.target)) {
      closeContextMenu(false);
    }
  });
  document.addEventListener("keydown", function (_event) {
    if (_event.key !== "Escape") {
      return;
    }
    if (_modalOpen) {
      _event.preventDefault();
      closeFormatMessage();
    } else if (_context) {
      _event.preventDefault();
      closeContextMenu(true);
    }
  });
  window.addEventListener(
    "scroll",
    function () {
      closeContextMenu(false);
    },
    true
  );
  window.addEventListener("resize", function () {
    closeContextMenu(false);
  });

  return {
    openContextMenu: openContextMenu,
    isModalOpen: function () {
      return _modalOpen;
    },
  };
}

// Reusable popup mechanics for the toolbar's custom dropdowns (Level, Tags).
// Self-contained here, no longer modelled on any other module's popup API:
// hidden-attribute toggling, viewport-clamped absolute positioning measured
// via a temporary visibility:hidden pass, outside-click/Escape/scroll/resize
// dismissal. Selection semantics (single-select vs multi-select, closing or
// not closing on pick) are left entirely to the caller.
// _onOpen runs once the dropdown is flagged open but before it is positioned,
// so a caller that deferred a rebuild can commit it and still have the popup
// measured against its final content.
export function createDropdown(_button, _popup, _onOpen) {
  var _open = false;

  function position() {
    var _margin = 8;
    var _gap = 4;
    _popup.style.visibility = "hidden";
    _popup.hidden = false;
    _popup.style.maxHeight =
      Math.max(0, Math.min(320, window.innerHeight - _margin * 2)) + "px";
    var _buttonBounds = _button.getBoundingClientRect();
    var _popupBounds = _popup.getBoundingClientRect();
    var _left = Math.max(
      _margin,
      Math.min(
        _buttonBounds.left,
        window.innerWidth - _popupBounds.width - _margin
      )
    );
    var _spaceBelow =
      window.innerHeight - _buttonBounds.bottom - _margin - _gap;
    var _spaceAbove = _buttonBounds.top - _margin - _gap;
    var _opensAbove =
      _spaceBelow < _popupBounds.height && _spaceAbove > _spaceBelow;
    var _preferredTop = _opensAbove
      ? _buttonBounds.top - _popupBounds.height - _gap
      : _buttonBounds.bottom + _gap;
    var _top = Math.max(
      _margin,
      Math.min(
        _preferredTop,
        window.innerHeight - _popupBounds.height - _margin
      )
    );
    _popup.style.left = _left + "px";
    _popup.style.top = _top + "px";
    _popup.style.visibility = "";
  }

  function close() {
    if (!_open) {
      return;
    }
    _open = false;
    _popup.hidden = true;
    _popup.style.left = "";
    _popup.style.top = "";
    _popup.style.maxHeight = "";
    _button.setAttribute("aria-expanded", "false");
  }

  function open() {
    if (_open) {
      return;
    }
    _open = true;
    _button.setAttribute("aria-expanded", "true");
    if (typeof _onOpen === "function") {
      _onOpen();
    }
    position();
  }

  function toggle() {
    if (_open) {
      close();
    } else {
      open();
    }
  }

  _button.addEventListener("click", toggle);
  document.addEventListener("pointerdown", function (_event) {
    if (
      _open &&
      !_popup.contains(_event.target) &&
      !_button.contains(_event.target)
    ) {
      close();
    }
  });
  document.addEventListener("keydown", function (_event) {
    if (_open && _event.key === "Escape") {
      _event.preventDefault();
      close();
    }
  });
  // Scroll events never bubble, but a capture-phase listener on the window still
  // observes every scrollable element on the page. The popup is positioned
  // against the button, so only a scroll that can move the button invalidates
  // it. Scrolling an unrelated container - the log area being re-rendered and
  // pinned to the bottom by the two-second refresh, or the popup's own option
  // list - must leave the dropdown alone.
  function movesAnchor(_target) {
    if (_target === null || _target === undefined) {
      return true;
    }
    if (typeof _target.contains !== "function") {
      return true;
    }
    return _target.contains(_button);
  }

  window.addEventListener(
    "scroll",
    function (_event) {
      if (_open && movesAnchor(_event.target)) {
        close();
      }
    },
    true
  );
  window.addEventListener("resize", function () {
    close();
  });

  return {
    close: close,
    isOpen: function () {
      return _open;
    },
    reposition: function () {
      if (_open) {
        position();
      }
    },
  };
}

function bindOptionActivation(_element, _action) {
  _element.tabIndex = 0;
  _element.addEventListener("click", _action);
  _element.addEventListener("keydown", function (_event) {
    if (_event.key === "Enter" || _event.key === " ") {
      _event.preventDefault();
      _action();
    }
  });
}

export function createLogFilters(_elements, _state, _actions) {
  var _levelDropdown = createDropdown(
    _elements.levelButton,
    _elements.levelPopup
  );
  var _tagsRenderPending = false;
  var _tagsDropdown = createDropdown(
    _elements.tagsButton,
    _elements.tagsPopup,
    function () {
      if (_tagsRenderPending) {
        renderTagsPopupContent();
      }
    }
  );
  var _tagsRequest = null;

  function updateLevelSelection() {
    _elements.levelButton.textContent = _state.level || "All levels";
    Array.from(_elements.levelPopup.children).forEach(function (_item) {
      var _selected = _item.dataset.value === _state.level;
      _item.classList.toggle("selected", _selected);
      _item.setAttribute("aria-selected", String(_selected));
    });
  }

  function selectLevel(_level) {
    _levelDropdown.close();
    if (_state.level === _level) {
      return;
    }
    _state.level = _level;
    updateLevelSelection();
    _actions.resetLogs();
  }

  function tagsLabel() {
    var _count = _state.selectedTags.size;
    if (_count === 0) {
      return "All tags";
    }
    return _count + (_count === 1 ? " tag selected" : " tags selected");
  }

  // Rebuilds every option node, so it is only safe while the popup is closed:
  // replaceChildren discards the node the user is currently interacting with.
  // Also reached from the dropdown's open callback to flush a deferred refresh
  // before the popup is measured and shown.
  function renderTagsPopupContent() {
    _tagsRenderPending = false;
    _elements.tagsButton.textContent = tagsLabel();
    if (_state.availableTags.length === 0) {
      var _empty = document.createElement("li");
      _empty.className = "dropdown-empty";
      _empty.textContent = "No tags for this session";
      _elements.tagsPopup.replaceChildren(_empty);
      _tagsDropdown.reposition();
      return;
    }
    var _fragment = document.createDocumentFragment();
    _state.availableTags.forEach(function (_tag) {
      var _item = document.createElement("li");
      _item.setAttribute("role", "option");
      _item.dataset.tag = _tag;
      _item.textContent = _tag;
      var _selected = _state.selectedTags.has(_tag);
      _item.classList.toggle("selected", _selected);
      _item.setAttribute("aria-selected", String(_selected));
      bindOptionActivation(_item, function () {
        toggleTag(_tag);
      });
      _fragment.appendChild(_item);
    });
    _elements.tagsPopup.replaceChildren(_fragment);
    _tagsDropdown.reposition();
  }

  // Entry point for background refreshes driven by log polling. While the user
  // has the popup open its option nodes stay untouched and the rebuild is
  // deferred to the next open; the button label lives outside the popup and is
  // never interacted with, so it keeps tracking the selection immediately.
  function renderTagsPopup() {
    if (!_tagsDropdown.isOpen()) {
      renderTagsPopupContent();
      return;
    }
    _tagsRenderPending = true;
    _elements.tagsButton.textContent = tagsLabel();
  }

  function updateTagSelection(_tag) {
    var _selected = _state.selectedTags.has(_tag);
    Array.from(_elements.tagsPopup.children).forEach(function (_item) {
      if (_item.dataset.tag === _tag) {
        _item.classList.toggle("selected", _selected);
        _item.setAttribute("aria-selected", String(_selected));
      }
    });
  }

  function toggleTag(_tag) {
    if (_state.selectedTags.has(_tag)) {
      _state.selectedTags.delete(_tag);
    } else {
      _state.selectedTags.add(_tag);
    }
    // The click came from inside the open popup, so update the clicked node in
    // place rather than rebuilding the list out from under the user.
    _elements.tagsButton.textContent = tagsLabel();
    updateTagSelection(_tag);
    _actions.resetLogs();
  }

  async function loadAvailableTags(_sessionId) {
    if (_sessionId === undefined) {
      return;
    }
    if (_tagsRequest !== null) {
      _tagsRequest.abort();
    }
    var _controller = new AbortController();
    _tagsRequest = _controller;
    try {
      var _tags =
        (await _actions.fetchTags(_sessionId, _controller.signal)) || [];
      if (
        _tagsRequest !== _controller ||
        !_actions.isCurrentSession(_sessionId)
      ) {
        return;
      }
      _state.availableTags = _tags;
      renderTagsPopup();
    } catch (_error) {
      if (
        _error.name !== "AbortError" &&
        _tagsRequest === _controller &&
        _actions.isCurrentSession(_sessionId)
      ) {
        _actions.showError("Failed to load tags: " + _error.message);
      }
    } finally {
      if (_tagsRequest === _controller) {
        _tagsRequest = null;
      }
    }
  }

  function resetTagsForSession() {
    if (_tagsRequest !== null) {
      _tagsRequest.abort();
      _tagsRequest = null;
    }
    _state.selectedTags.clear();
    _state.availableTags = [];
    _tagsDropdown.close();
    _tagsRenderPending = false;
    renderTagsPopup();
  }

  Array.from(_elements.levelPopup.children).forEach(function (_item) {
    bindOptionActivation(_item, function () {
      selectLevel(_item.dataset.value);
    });
  });
  updateLevelSelection();

  return {
    loadAvailableTags: loadAvailableTags,
    resetTagsForSession: resetTagsForSession,
  };
}

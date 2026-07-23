// Reusable popup mechanics for the toolbar's custom dropdowns (Level, Tags).
// Mirrors the openContextMenu/closeContextMenu shape in telemetry-message.js:
// hidden-attribute toggling, viewport-clamped absolute positioning measured
// via a temporary visibility:hidden pass, outside-click/Escape/scroll/resize
// dismissal. Selection semantics (single-select vs multi-select, closing or
// not closing on pick) are left entirely to the caller.
export function createDropdown(_button, _popup) {
  var _open = false;

  function position() {
    _popup.style.visibility = "hidden";
    _popup.hidden = false;
    var _buttonBounds = _button.getBoundingClientRect();
    var _popupBounds = _popup.getBoundingClientRect();
    var _margin = 8;
    var _left = Math.max(
      _margin,
      Math.min(
        _buttonBounds.left,
        window.innerWidth - _popupBounds.width - _margin
      )
    );
    var _top = Math.max(
      _margin,
      Math.min(
        _buttonBounds.bottom + 4,
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
    _button.setAttribute("aria-expanded", "false");
  }

  function open() {
    if (_open) {
      return;
    }
    _open = true;
    _button.setAttribute("aria-expanded", "true");
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
  window.addEventListener(
    "scroll",
    function () {
      close();
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
  var _tagsDropdown = createDropdown(_elements.tagsButton, _elements.tagsPopup);

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

  function renderTagsPopup() {
    _elements.tagsButton.textContent = tagsLabel();
    if (_state.availableTags.length === 0) {
      var _empty = document.createElement("li");
      _empty.className = "dropdown-empty";
      _empty.textContent = "No tags for this session";
      _elements.tagsPopup.replaceChildren(_empty);
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
  }

  function toggleTag(_tag) {
    if (_state.selectedTags.has(_tag)) {
      _state.selectedTags.delete(_tag);
    } else {
      _state.selectedTags.add(_tag);
    }
    renderTagsPopup();
    _actions.resetLogs();
  }

  async function loadAvailableTags(_sessionId) {
    if (_sessionId === undefined) {
      return;
    }
    try {
      var _tags = (await _actions.fetchTags(_sessionId)) || [];
      if (!_actions.isCurrentSession(_sessionId)) {
        return;
      }
      _state.availableTags = _tags;
      renderTagsPopup();
    } catch (_error) {
      if (_error.name !== "AbortError") {
        _actions.showError("Failed to load tags: " + _error.message);
      }
    }
  }

  function resetTagsForSession() {
    _state.selectedTags.clear();
    _state.availableTags = [];
    _tagsDropdown.close();
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

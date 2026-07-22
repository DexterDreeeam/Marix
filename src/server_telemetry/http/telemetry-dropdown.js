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

// Scheme init and the segmented switch (spec 13.B as amended by 14.B).
// Synchronous in head so a stored scheme applies before first paint.
(function () {
  var root = document.documentElement, S = "ulpia-scheme", v = null;
  try { v = localStorage.getItem(S); } catch (e) {}
  if (v === "light" || v === "dark") root.dataset.theme = v;
  root.dataset.js = "";
  document.addEventListener("DOMContentLoaded", function () {
    var group = document.querySelector(".scheme-switch");
    if (!group) return;
    var radios = [].slice.call(group.querySelectorAll("[role=radio]"));
    function cur() {
      var t = root.dataset.theme;
      return t === "light" || t === "dark" ? t : "auto";
    }
    function apply(state) {
      if (state === "auto") { delete root.dataset.theme; } else { root.dataset.theme = state; }
      try {
        state === "auto" ? localStorage.removeItem(S) : localStorage.setItem(S, state);
      } catch (e) {}
      paint();
    }
    // aria-checked carries the state; the active segment is the tab stop.
    function paint() {
      var c = cur();
      radios.forEach(function (r) {
        var on = r.dataset.scheme === c;
        r.setAttribute("aria-checked", on ? "true" : "false");
        r.tabIndex = on ? 0 : -1;
      });
    }
    radios.forEach(function (r, i) {
      r.addEventListener("click", function () { apply(r.dataset.scheme); });
      // Standard radio keyboard: arrows move the selection and apply it.
      r.addEventListener("keydown", function (e) {
        var d = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1
              : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
        if (!d) return;
        e.preventDefault();
        var n = radios[(i + d + radios.length) % radios.length];
        apply(n.dataset.scheme);
        n.focus();
      });
    });
    paint();
  });
})();

// Scheme init and the lamp toggle (spec 13.B as amended by 15.C).
// Synchronous in head so a stored scheme applies before first paint. The lamp
// itself is pure CSS driven by the html attribute; this script only cycles
// the choice, stores it, and keeps the accessible name current-state-first.
(function () {
  var root = document.documentElement, S = "ulpia-scheme", v = null;
  try { v = localStorage.getItem(S); } catch (e) {}
  if (v === "light" || v === "dark") root.dataset.theme = v;
  root.dataset.js = "";
  document.addEventListener("DOMContentLoaded", function () {
    var btn = document.querySelector(".lamp-toggle");
    if (!btn) return;
    var cycle = ["auto", "light", "dark"];
    function cur() {
      var t = root.dataset.theme;
      return t === "light" || t === "dark" ? t : "auto";
    }
    function paint() {
      btn.setAttribute("aria-label", "Color scheme: " + cur() + ", press to change");
    }
    btn.addEventListener("click", function () {
      var n = cycle[(cycle.indexOf(cur()) + 1) % 3];
      if (n === "auto") { delete root.dataset.theme; } else { root.dataset.theme = n; }
      try {
        n === "auto" ? localStorage.removeItem(S) : localStorage.setItem(S, n);
      } catch (e) {}
      paint();
    });
    paint();
  });
})();

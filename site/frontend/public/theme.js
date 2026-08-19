// The page's one script: apply a stored color scheme before first paint,
// reveal the switch, cycle auto -> light -> dark. Nothing else.
(function () {
  var root = document.documentElement, S = "ulpia-scheme", v = null;
  try { v = localStorage.getItem(S); } catch (e) {}
  if (v === "light" || v === "dark") root.dataset.theme = v;
  root.dataset.js = "";
  document.addEventListener("DOMContentLoaded", function () {
    var btn = document.querySelector(".scheme-switch");
    if (!btn) return;
    var cycle = ["auto", "light", "dark"];
    function cur() {
      var t = root.dataset.theme;
      return t === "light" || t === "dark" ? t : "auto";
    }
    function next() { return cycle[(cycle.indexOf(cur()) + 1) % 3]; }
    function paint() {
      // The label is the state a press will apply; the page shows the current one.
      btn.textContent = next();
      btn.setAttribute("aria-label", "Color scheme: " + cur() + ". Press for " + next() + ".");
    }
    btn.addEventListener("click", function () {
      var n = next();
      if (n === "auto") { delete root.dataset.theme; } else { root.dataset.theme = n; }
      try { n === "auto" ? localStorage.removeItem(S) : localStorage.setItem(S, n); } catch (e) {}
      paint();
    });
    paint();
  });
})();

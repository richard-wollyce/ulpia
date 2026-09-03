// Scheme init and the lamp toggle (spec 18.D). Two states, no auto: at load
// the choice resolves to the stored one, or to the device's scheme; every
// press flips light and dark and stores. Synchronous in head so the attribute
// is set before first paint (no flash: the resolved value is what the media
// query would have painted anyway). The lamp is pure CSS driven by the
// attribute; this script cycles, stores, and keeps the name current.
(function () {
  var root = document.documentElement, S = "ulpia-scheme", v = null;
  try { v = localStorage.getItem(S); } catch (e) {}
  if (v !== "light" && v !== "dark") {
    v = matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  root.dataset.theme = v;
  root.dataset.js = "";
  document.addEventListener("DOMContentLoaded", function () {
    var btn = document.querySelector(".lamp-toggle");
    if (!btn) return;
    function paint() {
      btn.setAttribute("aria-label", "Color scheme: " + root.dataset.theme + ", press to change");
    }
    btn.addEventListener("click", function () {
      // The .pressed gate (spec 17.B): animations exist only after a real
      // press, so a stored-state restore never performs at page load.
      btn.classList.add("pressed");
      var n = root.dataset.theme === "dark" ? "light" : "dark";
      root.dataset.theme = n;
      try { localStorage.setItem(S, n); } catch (e) {}
      paint();
    });
    paint();
  });
})();

// The ambient field loads from here rather than from a tag in every page.
// theme.js is the one script all sixteen pages already carry, and it is
// deliberately synchronous in <head> so the stored scheme applies before first
// paint. So this appends a deferred tag instead of doing the work inline: the
// field never blocks the paint that theme.js exists to protect, and adding a
// page does not mean remembering a second script tag.
(function () {
  var s = document.createElement("script");
  s.src = "/field.js";
  s.defer = true;
  (document.head || document.documentElement).appendChild(s);
})();

// The animation system, spec 14.C. Typing on the hero standfirst, a rise for
// the CTA row after it, scroll-triggered rises for section content. Floors:
// reduced motion exits before any work (the CSS gate already hides nothing),
// screen readers always hold complete text (the typed stream is aria-hidden
// over transparent real text), only opacity and transform move, no
// element.style writes, and a 6s failsafe force-shows everything.
(function () {
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;

  var TYPE_START_MS = 250, TYPE_CHAR_MS = 20, TYPE_BEAT_MS = 120;
  var FAILSAFE_MS = 6000;

  function show(el, instant) {
    el.classList.add("is-shown");
    if (instant) el.classList.add("instant");
  }

  document.addEventListener("DOMContentLoaded", function () {
    var heroEls = [].slice.call(document.querySelectorAll("[data-anim=hero]"));
    var riseEls = [].slice.call(document.querySelectorAll("[data-anim=rise]"));
    var typeP = document.querySelector("[data-anim=type]");
    var real = typeP && typeP.querySelector(".type-real");

    // The typing twin. The real text keeps its box and its place in the
    // accessibility tree; the live span is a visual double and nothing more.
    function typeStandfirst(done) {
      if (!typeP || !real) return done();
      var text = real.textContent, i = 0, timer = null, finished = false;
      var live = document.createElement("span");
      live.className = "type-live";
      live.setAttribute("aria-hidden", "true");
      // Every character exists from the first frame, hidden, so the line is
      // laid out in its final shape and typing never moves a glyph.
      var chars = [];
      for (var k = 0; k < text.length; k++) {
        var ch = document.createElement("span");
        ch.className = "ch";
        ch.textContent = text.charAt(k);
        live.appendChild(ch);
        chars.push(ch);
      }
      function finish() {
        if (finished) return;
        finished = true;
        clearInterval(timer);
        typeP.classList.remove("is-typing");
        if (live.parentNode) live.remove();
        document.removeEventListener("visibilitychange", onHide);
        done();
      }
      function onHide() { if (document.hidden) finish(); }
      // Typing to an empty room is waste; a hidden tab completes instantly.
      if (document.hidden) return done();
      typeP.classList.add("is-typing");
      typeP.appendChild(live);
      document.addEventListener("visibilitychange", onHide);
      // Any interaction with the hero skips to the end.
      var hero = typeP.closest("section") || typeP;
      hero.addEventListener("keydown", finish, { once: true });
      hero.addEventListener("pointerdown", finish, { once: true });
      setTimeout(function () {
        timer = setInterval(function () {
          if (i > 0) chars[i - 1].classList.remove("cur");
          if (i >= chars.length) return finish();
          chars[i].className = "ch on cur";
          i++;
        }, TYPE_CHAR_MS);
      }, TYPE_START_MS);
    }

    typeStandfirst(function () {
      setTimeout(function () {
        heroEls.forEach(function (el) { show(el); });
      }, TYPE_BEAT_MS);
    });

    // Scroll rises: once only, activation when a block's top crosses the line
    // 65% down the viewport. Blocks the line can never reach fall back to a
    // plain viewport-entry observer; nothing may be unreachable by scroll.
    var lineObs = new IntersectionObserver(onEnter, { rootMargin: "0px 0px -35% 0px" });
    var edgeObs = new IntersectionObserver(onEnter, {});
    function onEnter(entries, obs) {
      entries.forEach(function (e) {
        if (!e.isIntersecting) return;
        obs.unobserve(e.target);
        show(e.target);
      });
    }
    var bySection = new Map();
    riseEls.forEach(function (el) {
      // Stagger index within the section, capped at two followers.
      var section = el.closest("section");
      var n = (bySection.get(section) || 0) + 1;
      bySection.set(section, n);
      if (n === 2) el.classList.add("delay-1");
      if (n >= 3) el.classList.add("delay-2");
      var reachable = el.offsetTop <= document.documentElement.scrollHeight - innerHeight * 0.65;
      (reachable ? lineObs : edgeObs).observe(el);
    });

    // A keyboard user must never focus into an invisible block.
    document.addEventListener("focusin", function (e) {
      var box = e.target.closest("[data-anim]:not(.is-shown)");
      if (box) show(box, true);
    });

    // No script failure may leave content hidden.
    setTimeout(function () {
      heroEls.concat(riseEls).forEach(function (el) {
        if (!el.classList.contains("is-shown")) show(el, true);
      });
    }, FAILSAFE_MS);
  });
})();

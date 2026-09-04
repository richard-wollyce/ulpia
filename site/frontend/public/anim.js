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

    // ---- the latency chart plays only to somebody who is looking at it.
    //
    // REVEALING AND PERFORMING ARE DIFFERENT JOBS AND THEY NEEDED DIFFERENT
    // CLASSES. `is-shown` means the block is visible, and the 6s failsafe below
    // adds it to everything on the page whether or not it was ever scrolled to,
    // because no script failure may leave content hidden. That is correct for a
    // paragraph and wrong for a chart: the figure was getting `is-shown` at the
    // six second mark while still far below the fold, spending its four seconds
    // of fill on an empty screen, and by the time a reader arrived it had
    // finished. Measured, not guessed: at t=6500ms the figure had the class with
    // its bounding box entirely outside the viewport.
    //
    // So `is-playing` is a second, narrower class with its own observer, and
    // NOTHING else may add it: not the failsafe, not the focus guard. If the
    // figure is never scrolled to, it simply never plays, and what renders is
    // the finished chart, which is the true one. That is why this needs no
    // failsafe of its own.
    //
    // The line is 55% down the viewport, deliberately BELOW the 65% line the
    // rise uses. Scrolling down, an element crosses 65% first and 55% second, so
    // the reveal always starts before the performance and the bars never fill
    // while the figure is still faded out.
    var chartObs = new IntersectionObserver(function (entries, obs) {
      entries.forEach(function (e) {
        if (!e.isIntersecting) return;
        obs.unobserve(e.target);
        e.target.classList.add("is-playing");
      });
    }, { rootMargin: "0px 0px -45% 0px" });
    [].slice.call(document.querySelectorAll(".cmp")).forEach(function (fig) {
      chartObs.observe(fig);
    });

    // ---- the latency chart's one control (motion spec, 2026-09-04).
    // At 13.333x the figure's whole sequence runs about 7.2s, past the five
    // seconds at which WCAG 2.2.2 requires automatically starting motion to be
    // pausable, stoppable or hidable. This button is that mechanism, and it is
    // not optional furniture: without it the chart is non-conforming.
    //
    // Built here and never in markup, so it cannot exist without JS and cannot
    // exist under reduced motion, where this file returns at line 8 and there
    // is no motion to pause. Nothing is stored: a reader's pause belongs to
    // their visit, not to the next one.
    //
    // The label does NOT change with state. A toggle button that swaps its
    // accessible name while also setting aria-pressed announces two opposite
    // things at once ("Play, pressed"), which is worse than either alone.
    // aria-pressed carries the state and the name stays the action.
    [].slice.call(document.querySelectorAll(".cmp")).forEach(function (fig) {
      var axis = fig.querySelector(".cmp-axis");
      if (!axis) return;
      var p = document.createElement("p");
      var b = document.createElement("button");
      p.className = "cmp-ctl";
      b.type = "button";
      b.className = "cmp-pause";
      b.setAttribute("aria-pressed", "false");
      b.textContent = "Pause the animation";
      b.addEventListener("click", function () {
        var paused = fig.classList.toggle("is-paused");
        b.setAttribute("aria-pressed", paused ? "true" : "false");
      });
      p.appendChild(b);
      axis.parentNode.insertBefore(p, axis.nextSibling);
    });

    // No script failure may leave content hidden.
    setTimeout(function () {
      heroEls.concat(riseEls).forEach(function (el) {
        if (!el.classList.contains("is-shown")) show(el, true);
      });
    }, FAILSAFE_MS);
  });
})();

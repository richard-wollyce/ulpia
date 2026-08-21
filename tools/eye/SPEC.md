# `eye`, a specification

**One line contract.** `eye` renders a surface in headless Chromium across the whole review matrix Aldus's constitution names, samples computed styles at every interaction state, diffs each state against rest, and reports a state that changed nothing as a failure. Pixels are the by-product for Richard. The diff is the product for Aldus.

**Verification tiers used below:** *ran it* (I executed it against this machine's Playwright/Chromium during this session), *read the source* (Ulpia files or Playwright's API surface), *read the docs*, *guessing*. Everything unmarked in a mechanism claim is *guessing* and should be treated as such.

---

## 0. What I verified before specifying anything

I wrote two throwaway probes in `tools/eye/`, ran them against the installed Playwright (node v25.0.0, `playwright` 1.56 resolved, Chromium launched headless), and deleted them. Results that decide the spec:

| Question | Result | Tier |
|---|---|---|
| Does headless Chromium screenshot with no visible pane | Yes, a 200x60 clip returned a 990-byte PNG | ran it |
| Is the stale `getComputedStyle` problem present here | No. Writing `background-color: rgb(1,2,3) !important` inline and reading back in the same `evaluate` returned `rgb(1, 2, 3)`. The staleness lives in the other pipe, not in CDP | ran it |
| Does `element.focus()` produce `:focus-visible` | Yes, `a.matches(":focus-visible")` was `true`, and still `true` after a real mouse click elsewhere first | ran it |
| Is Tab-walking a safe way to reach a target | No. One `Tab` from the last link put `document.activeElement` on `BODY` with `:focus-visible` false. Focus leaves the document and comes back as body | ran it |
| Does `mouse.down()` hold `:active` | Yes, `matches(":active")` true while held | ran it |
| **Is an immediate sample after entering a state safe** | **No.** While `:active` was held, `transform` still read `matrix(1,0,0,1,0,0)`, the rest value, because the 90ms transition had not run. Sampling without settling manufactures false "changed nothing" failures | ran it |
| Does `getAnimations().finished` settle a CSS transition deterministically | Yes. Immediately after `hover`, `getAnimations({subtree:true}).length` was 1 and colour still read the start value; after awaiting `.finished`, length 0 and colour was the end value | ran it |
| Does the drain hang on an infinite animation | Not if filtered. With a `spin 1s linear infinite` on the page, filtering to finite `iterations` in-page left exactly the one transition to await and returned | ran it |
| Can it see a pseudo-element state signal | Yes. `getComputedStyle(a, "::after").transform` went `matrix(0,0,0,1,0,0)` to `matrix(1,0,0,1,0,0)` on hover, while `color` stayed `rgb(201,168,96)` on both sides. **That is bug (a)'s exact shape reproduced: a colour no-op with a surviving second signal** | ran it |
| Touch: what do the media queries say | On a `hasTouch/isMobile` 360px context: `(hover:hover)` false, `(pointer:coarse)` true, `(any-hover:hover)` false | ran it |
| Touch: does a synthetic `touchstart` produce `:active` | No. `dispatchEvent("touchstart")` left `:active` false. Untrusted events do not drive UA state | ran it |
| Touch: does a CDP-held touch point produce `:active` | Not while held. After `Input.dispatchTouchEvent{touchStart}` plus 150ms, `:active` was false and `transform` was `none`. **After `touchEnd` plus 150ms, `transform` read `matrix(1,0,0,1,0,1)`, the 1px press.** Chromium defers the active state until the gesture is recognised as a tap | ran it |
| Under `reducedMotion: "reduce"` | `matchMedia` reports reduce, and the hover end value was readable immediately with no wait | ran it |
| `emulateMedia` flips scheme and motion live | Yes, both, mid-page | ran it |

Two source readings that decide the scheme axis (*read the source*): `site/frontend/public/theme.js` resolves the scheme **once, synchronously in `<head>`**, from `localStorage["ulpia-scheme"]` or the media query, and writes `data-theme`. `site/frontend/src/styles.css` writes every dark value twice, once behind `:root[data-theme="dark"]` and once behind `@media (prefers-color-scheme: dark) :root:not([data-theme="light"])`. Consequence: with JS running, the attribute branch always wins and the media branch is **never exercised**, so flipping `emulateMedia` after load does not change the resolved scheme at all.

---

## 1. File layout

```
tools/eye/
  package.json            # exists; playwright ^1.56, type: module, private
  eye.mjs                 # CLI, argument parsing, orchestration, exit codes
  lib/
    color.mjs             # parse, composite, relative luminance, WCAG ratio
    backdrop.mjs          # resolve the real background behind an element
    settle.mjs            # the animation drain, injected as a page function
    sample.mjs            # the property set, pseudo-elements, geometry, state entry
    checks.mjs            # every check id, its severity, its evidence shape
    discover.mjs          # the orphan sweep and the fingerprint
    report.mjs            # report.json, findings.json, index.html contact sheet
    sheet.mjs             # sheet loader and validator (rejects a waiver with no reason)
  test/
    color.test.mjs        # node --test, no dependency; WCAG ratios against known pairs
    backdrop.test.mjs     # compositing alpha stacks
  out/                    # every artifact of every run. gitignored
  .gitignore
site/frontend/eye.sheet.mjs   # the review sheet, tracked, lives beside the surface
```

`node --test` is built into Node 25 (*ran it*: `node -e "console.log(process.version)"` printed `v25.0.0`), so the colour maths gets unit tests without adding a dependency. The colour maths is the one part that can be wrong silently, because a wrong ratio still prints a plausible number.

**Delete `verify-band.mjs` when `eye.mjs` lands.** It is the throwaway that proved the approach, its header says so, and two tools that answer the same question will disagree within a month.

---

## 2. CLI signature

```
node tools/eye/eye.mjs <sheet.mjs> [flags]     # the matrix. The default verb.
node tools/eye/eye.mjs shot <url> [flags]      # one cell, rendered, for a human
node tools/eye/eye.mjs tokens <url> [flags]    # the resolved token table, per scheme
```

Matrix flags. **Every flag subtracts. None adds.**

| Flag | Default | Effect |
|---|---|---|
| `--base <url>` | from sheet | Override the base URL |
| `--pages a,b` | all | Subset by page name. Stamps `PARTIAL` |
| `--targets a,b` | all | Subset by target name. Stamps `PARTIAL` |
| `--widths 360,1280` | sheet's | Stamps `PARTIAL` unless a superset of the sheet's |
| `--schemes light,dark` | both | Stamps `PARTIAL` if one |
| `--motion no-preference,reduce` | both | Stamps `PARTIAL` if one |
| `--states rest,hover,focus-visible,active,touch-press` | all | Stamps `PARTIAL` if fewer |
| `--no-nojs` | nojs on | Skip the JavaScript-disabled pass. Stamps `PARTIAL` |
| `--quick` | off | Preset: widths 360 and 1280, motion no-preference, no nojs. Stamps `PARTIAL` |
| `--shots off\|fail\|context\|all` | `context` | See section 7 |
| `--dpr n` | 2 | Device pixel ratio for captures |
| `--settle-ms n` | 1200 | Settle deadline per state |
| `--timeout n` | 15000 | Per navigation |
| `--concurrency n` | 4 | Parallel contexts |
| `--out dir` | `tools/eye/out` | Artifact root |
| `--json -` | off | Stream the full report to stdout instead of the summary |
| `--fail-on error\|warn` | `error` | Which severity sets exit 1 |
| `--headed` | off | For a human watching it work |

`shot` flags: `--at <width>`, `--scheme`, `--motion`, `--state`, `--clip <selector>`, `--out <file>`.

**Exit codes.** `0` ran and nothing at or above the fail level. `1` findings at the fail level. `2` **the run could not be trusted**: base unreachable, the design system's stylesheet did not load, a sheet selector matched nothing on any page, the sheet failed validation, a waiver carried no reason.

Separating 2 from 1 is the point. A tool that exits 1 when it never actually looked teaches the reader to argue with red runs. Exit 2 means "you learned nothing", and the summary line says so in those words.

**`PARTIAL` is the anti-narrowing mechanism.** Any trim sets `run.complete=false` in the JSON, prints `PARTIAL: trimmed motion=reduce, widths` on the first line of the summary, and prefixes the contact sheet's title. Aldus's constitution can then read one boolean: **a review with `complete=false` is not a review**, which is the machine form of "a design reviewed only at desktop width in light mode was not reviewed".

---

## 3. The sheet

### D1. How targets are chosen: CLI selectors, a sheet, or auto-discovery

- **CLI selector list.** Zero indirection, instant to start. Consequence: the invocation is long, nobody retypes it identically, and the matrix quietly narrows to whatever the person could be bothered to type. That is the same failure as reviewing only desktop light, arriving by a different road.
- **A sheet file.** The review's definition is versioned beside the surface, so a run at commit N and a run at commit N+20 cover the same targets and a regression is a diff rather than a memory. Consequence: a component that exists on the page but not in the sheet is invisible, which is how a new control ships unreviewed.
- **Auto-discovery of every interactive element.** Nothing is invisible. Consequence: the report drowns. Every anchor in the body is interactive, most share one rule, and a long report gets skimmed, which is not looking.

**Picked: the sheet as the matrix's source of truth, plus a mandatory orphan sweep.** The sweep runs auto-discovery, subtracts everything a sheet selector already matches, groups the remainder by a computed-style fingerprint, and reports the groups as `ORPHANS` (warning) with a count and one example path each. The sheet keeps the report short; the sweep closes the sheet's blind spot without paying its noise. The fingerprint is `tag | role | color | backgroundColor | textDecorationLine | fontWeight | hasAfterContent`, so forty links sharing one rule collapse to one line reading `40 x a.doc-link`, and a genuinely new component is a new group and cannot hide inside an old one.

### D2. Sheet format: JSON or an ES module

- **JSON.** Parseable by anything, diffs cleanly. Consequence: no comments, so a target cannot carry the reason it is in the sheet, and a list of selectors with no reasons is a list nobody dares prune.
- **`.mjs` default export.** Carries comments, can compute selectors, can extend another sheet. Consequence: it is executable code loaded from disk, which for a local tool run by the repository's owner is not a real risk, and it matches what is already here (`verify-band.mjs`, `tools/build-posts.mjs`).

**Picked: `.mjs`.** In this house a target without its `why` is exactly the sort of value that "gets rounded by the next person who needs a round number".

### The sheet, concretely, for the live surface

```js
// site/frontend/eye.sheet.mjs
export default {
  name: "ulpia-site",
  base: "http://localhost:5173",          // vite dev. Use 4173 for `npm run preview`.
  pages: [
    { name: "front",   path: "/" },
    { name: "terms",   path: "/terms/" }, // the wordmark is a link home here
    { name: "writing", path: "/blog/" },  // the nav item carries aria-current
  ],

  // px are computed as rem * the page's rendered root font-size, not * 16,
  // so a user font-size preference moves the straddle with the breakpoint.
  breakpoints: [34, 56],                  // styles.css:1183 and :617

  matrix: {
    stateWidths:  [360, 768, 1280],       // the constitution's three; full state matrix
    layoutWidths: "breakpoint-straddles", // 544/545 and 896/897; rest state plus geometry
    touchAt:      [360],                  // widths that get hasTouch + isMobile + coarse
    schemes:      ["light", "dark"],
    motion:       ["no-preference", "reduce"],
    nojs:         { widths: [360, 1280], schemes: ["light", "dark"] },
  },

  // The canonical table, transcribed from design-system.md section 2. Any
  // difference in what the page resolves is a fork, and a fork is how two
  // Ulpia windows come to disagree about what red means.
  tokens: {
    light: { "--ground": "#f6f1e7", "--ink": "#241d16", "--ink-2": "#5a5044",
             "--accent": "#a63325", "--rule": "#dacfc0", "--chrome-ink": "#241d16",
             "--chrome-rule": "#dacfc0", "--plate": "#ede5d5" },
    dark:  { "--ground": "#12100d", "--ink": "#e7ddcc", "--ink-2": "#ab9e89",
             "--accent": "#c9a860", "--rule": "#3d3320", "--chrome-ink": "#c9a860",
             "--chrome-rule": "#55452b", "--plate": "#211b16" },
  },

  // The two ground laws, as exact colour equality. No heuristics.
  forbidden: {
    light: ["#c9a860", "#d8be82", "#a88c4e"],   // gold is cover material only
    dark:  ["#a63325"],                          // dark mode contains no red
    exempt: ["--lamp-glow"],                     // law 1's first amendment
  },

  floors: { text: 4.5, largeText: 3.0, nonText: 3.0,
            weakSignal: 1.5, targetMin: 24, targetPreferred: 44 },

  targets: [
    { name: "wordmark", selector: ".wordmark a", pages: ["terms", "writing"],
      why: "Identity. It has no underline marker by design, so colour is its only " +
           "hover signal and a colour no-op leaves it with nothing.",
      states: ["hover", "focus-visible", "active"] },

    { name: "nav-item", selector: ".nav a:not([aria-current])",
      why: "Furniture. Its ::after underline is the second signal the wordmark lacks.",
      states: ["hover", "focus-visible", "active", "touch-press"] },

    { name: "nav-current", selector: ".nav a[aria-current]", pages: ["writing"],
      why: "Never colour alone: weight plus a 2px rule.",
      expect: { rest: { fontWeight: "600", "after.transform": "matrix(1, 0, 0, 1, 0, 0)" } } },

    { name: "lamp", selector: ".lamp-toggle", stateful: true,
      why: "The band's one performer, and the only control. Pressing it flips the " +
           "scheme, so :active must not complete a click." },

    { name: "body-link", selector: "main a", sample: "first",
      why: "The underline carries the affordance; the accent is quiet rubrication." },
  ],

  // A waiver without a reason throws at load. Waived findings still print,
  // in their own section, so they stay visible instead of disappearing.
  waivers: [
    { check: "WEAK_SIGNAL", target: "wordmark", scheme: "dark",
      because: "gold to gold-sheen is 1.25:1, a lightness step inside one hue family " +
               "by law 2, and it is deliberately paired with the 1px :active press. " +
               "design-system.md section 7." },
  ],
};
```

`expect` turns the design system's written rules into executable assertions per surface. `sample: "first"` versus the default `"all"` decides whether a selector matching many nodes reviews the first or every one, capped at 20 with a warning when the cap bites.

---

## 4. The matrix

**Axes and their default values.**

| Axis | Values | Why these |
|---|---|---|
| page | every sheet page | The band differs per page: the wordmark is a link only on inner pages, `aria-current` exists only on `/blog/` |
| width | 360, 768, 1280 for states; 544, 545, 896, 897 for layout | The constitution's three, plus both sides of each declared breakpoint. A breakpoint tested from one side is a breakpoint tested only by users |
| pointer | coarse at 360 (`hasTouch`, `isMobile`, dSF 3), fine above | `(hover:hover)` is false on the coarse profile (*ran it*), which is what makes a hover-only signal invisible |
| scheme | light, dark, each as its own context load | See D3 |
| motion | no-preference, reduce | See D7 |
| js | on, plus a no-JS pass at 360 and 1280 | See D3 |
| state | rest, hover, focus-visible, active, touch-press | The constitution's interaction states, plus the one a phone actually has |

Cell budget for the sheet above: 3 pages x 3 widths x 2 schemes x 2 motion = 36 page loads for the state matrix, 4 straddle widths x 2 schemes x 3 pages = 24 loads for layout, 4 no-JS loads. Roughly 60 loads and about 600 state samples. **Runtime not measured. Estimate low single-digit minutes on this machine, and that is a guess.** `--quick` exists for the iteration loop and stamps `PARTIAL` so it can never be mistaken for a review.

### D3. How the scheme is set

- **`emulateMedia({colorScheme})` only.** One context, flip live, cheap. Consequence on this surface: it flips nothing that matters. `theme.js` resolved the scheme once in `<head>` and wrote `data-theme`, and the attribute branch of every dark rule outranks the media branch, so the page keeps painting the scheme it loaded with. The tool would report light values under a dark label, which is worse than not testing.
- **Seed `localStorage["ulpia-scheme"]` via `context.addInitScript` before the first load, and set `colorScheme` on the context so both branches agree, one context per scheme.** Consequence: contexts double, and the media-query half of every dark rule is still never exercised while JS runs.

**Picked: seed plus context colour scheme, one load per scheme, and a separate no-JS pass** (`javaScriptEnabled: false`) at 360 and 1280 in both schemes. The no-JS pass is the only thing that reaches `@media (prefers-color-scheme: dark) :root:not([data-theme="light"])`, which is half of every dark rule this system writes. It also answers what the page looks like when the lamp cannot exist, and progressive enhancement is in Aldus's domain list. The no-JS pass runs rest, hover and focus-visible only, no touch, no screenshots beyond one full-page per cell.

---

## 5. Entering a state, and proving you entered it

Every sample carries `achieved: boolean`, read back from the DOM. **A cell that did not achieve its state is never allowed to produce an error.** It produces `STATE_NOT_ACHIEVED` (warning) instead. This kills the largest class of false failure a tool like this can invent.

| State | Entry | Assert | Tier |
|---|---|---|---|
| rest | `page.mouse.move(0,0)`, `document.activeElement?.blur()`, settle | `!matches(":hover") && !matches(":active")` | ran it |
| hover | `locator.hover()`, settle | `matches(":hover")` returned true | ran it |
| focus-visible | `el.focus()`, settle | `matches(":focus-visible")` returned true, and stayed true even after a prior real mouse click | ran it |
| active | `mouse.move(centre)`, `mouse.down()`, settle, sample, `mouse.up()` | `matches(":active")` true while held | ran it |
| touch-press | CDP `Input.dispatchTouchEvent` touchStart, settle, sample S1; touchEnd, sample S2; +150ms, sample S3 | union of S1..S3 versus rest | ran it |

### D4. Reaching focus: `el.focus()` or a Tab walk

- **Tab walk.** Closest to a real keyboard user, and `:focus-visible` is a UA heuristic that keyboard input is guaranteed to satisfy. Consequence: it is order-dependent and it falls out of the document. One `Tab` from the last link landed on `BODY` with `:focus-visible` false (*ran it*), so a walk to reach the fifth target needs a known tab order, which is the thing being tested.
- **`el.focus()`.** Deterministic, one call, any target. Consequence: it depends on the UA heuristic granting `:focus-visible` to programmatic focus, which is not guaranteed by spec and could change between Chromium versions.

**Picked: `el.focus()` with the assertion, and a bounded Tab walk as fallback.** If `matches(":focus-visible")` comes back false, press Tab up to 60 times looking for the element, stopping early if `activeElement` becomes `BODY` (focus left the document). If it is still false, the cell is `achieved:false` and warns. The assertion is what makes the fast path safe: the heuristic is allowed to change, because the tool checks rather than assumes.

### D5. Knowing a transition has settled

- **Fixed sleep.** `waitForTimeout(300)`, which is what the throwaway did. Consequence: it encodes a magic number that is wrong in both directions. Too short after a future 400ms transition and every state reads as a no-op; too long times 600 cells and the run is minutes of sleeping.
- **Drain the Web Animations queue.** Consequence: it is exact, it is fast, and it needs three guards (infinite animations, cancelled transitions, animations that start after the list was taken).

**Picked: the drain**, injected as:

```js
async function settle(deadlineMs) {
  const t0 = performance.now();
  for (let pass = 0; pass < 5; pass++) {
    const finite = document.getAnimations().filter(a => {
      const t = a.effect?.getComputedTiming?.();
      return t && Number.isFinite(t.iterations) && Number.isFinite(t.activeDuration);
    });
    if (!finite.length) break;
    await Promise.race([
      Promise.all(finite.map(a => a.finished.catch(() => {}))),
      new Promise(r => setTimeout(r, Math.max(0, deadlineMs - (performance.now() - t0)))),
    ]);
    if (performance.now() - t0 > deadlineMs) return { settled: false };
  }
  await new Promise(requestAnimationFrame);
  await new Promise(requestAnimationFrame);
  return { settled: true };
}
```

Three mechanisms, each earning its line. **The finite filter**: `getComputedTiming().iterations` is `Infinity` for a looping animation and `a.finished` for one never resolves, so an unfiltered drain hangs on any spinner (*ran it*: with a `spin ... infinite` on the page, the filtered drain awaited exactly one transition and returned). **The `.catch`**: a transition that is reversed mid-flight is cancelled, and a cancelled animation's `finished` rejects. **The loop**: page script can start a new animation inside the frame the previous one finished, so the list is re-taken up to five times. **The two `requestAnimationFrame` ticks**: `anim.js` adds classes in DOMContentLoaded and in an IntersectionObserver callback, and one tick guarantees the style recalc for a class added in the previous frame.

`--settle-ms` defaults to 1200 because the longest declared sequence in this system is the lamp's 360ms lighting and the details block-size at 240ms, so 1200 is over three times the longest thing that exists. **If the system ever animates longer than 1200ms this number must move**, and until it does the tool reports `settled: false` rather than lying about the value.

Once per page load, before any sampling: `await document.fonts.ready` and record `document.fonts.check('1rem "EB Garamond"')`. A screenshot taken before the webfont lands shows fallback type, and Aldus would be reviewing a typeface this system does not use.

### D6. The touch press, and why one sample is not enough

- **Sample once while the touch point is held.** The obvious design. Consequence: it reports a false failure. With a CDP touch point held for 150ms, `:active` was false and `transform` was `none` (*ran it*). Chromium withholds the active state until it has decided the gesture is a tap and not the start of a scroll.
- **Sample at three points and take the union: held, immediately after release, and release plus 150ms.** Consequence: three extra reads per touch cell, and the answer becomes the honest one. At release plus 150ms the element read `matrix(1, 0, 0, 1, 0, 1)`, the 1px press, so the change a phone user sees does exist and arrives after the finger lifts (*ran it*).

**Picked: the three-point union**, and the report says which of the three carried the change, because "the press only appears after release" is itself a design finding.

---

## 6. What is sampled, and the checks

### The property set

Per state, for the element and for `::before` and `::after`:

`color, backgroundColor, backgroundImage, borderTop/Right/Bottom/LeftColor, borderTop/Right/Bottom/LeftWidth, outlineColor, outlineStyle, outlineWidth, outlineOffset, textDecorationLine, textDecorationColor, textDecorationThickness, textUnderlineOffset, fontWeight, fontSize, fontStyle, letterSpacing, opacity, transform, translate, scale, rotate, boxShadow, filter, backdropFilter, visibility`, plus `content` on the pseudo-elements, plus the `DOMRect` rounded to 0.01px.

`cursor` is recorded but **excluded from the signal set**: it is a pointer affordance, it changes nothing on the page, and it does not exist on a phone. Counting it would let a hover with no visible effect pass.

The pseudo-elements are not optional. The nav's entire hover signal lives on `::after` (*ran it*: colour identical on both sides of hover, `::after` transform `matrix(0,0,0,1,0,0)` to `matrix(1,0,0,1,0,0)`). A tool that samples only the element reports the nav as broken and is ignored within a week.

### The backdrop, for contrast

**D11. Composite the CSS ancestors, or sample a pixel.**

- **Ancestor walk.** Exact when it applies, needs no image decoding, names which element supplied the background. Consequence: it is defeated by a `background-image`, including a gradient, and by an ancestor `opacity`.
- **Pixel sample from the screenshot.** Answers "what colour is actually behind this" with no CSS reasoning at all. Consequence: it picks up whatever glyph, hairline or ornament happens to sit at the sampled pixel, so it is confidently wrong in exactly the busy places where contrast matters.

**Picked: the walk as primary, the pixel as a marked fallback.** Walk from the element up, compositing `rgba` layers front to back, stopping at the first fully opaque one; multiply the text colour's alpha by the product of ancestor `opacity` values; if the walk reaches the root with nothing opaque, use `getComputedStyle(document.documentElement).backgroundColor`, which is what actually paints the canvas. If any ancestor has a `background-image` other than `none`, do not emit a pass or a fail: emit `CONTRAST_UNRELIABLE` (warning) naming the element that carries the image, and attach a 1px pixel sample taken 2px outside the element's border box as the fallback reading, labelled `reliable: false`. **A contrast number the tool cannot stand behind is worse than no number**, because it gets quoted later.

For the focus ring, the walk starts at `el.parentElement`, not at the element: `outline-offset: 3px` puts the ring outside the element's own box, so the element's own background is not what it sits on.

Ratio maths is WCAG 2.x sRGB relative luminance, `(L1 + 0.05) / (L2 + 0.05)`, the formula already in `verify-band.mjs` (*read the source*). Large-text relaxation is 24px, or 18.66px at weight 700 and above (*read the docs*). **In this system the bold branch is dead**: weights 400 and 600 are the only legal values, so the only relaxation is size, and the tool says so in the report rather than silently applying a branch that cannot fire.

### The checks

| id | Severity | What it catches | Mechanism |
|---|---|---|---|
| `PREFLIGHT_BASE` | exit 2 | The server is not running | `fetch(base)`; the message prints `npm --prefix site/frontend run dev` |
| `PREFLIGHT_STYLESHEET` | exit 2 | The design system did not load | `getComputedStyle(root).getPropertyValue("--ground")` is empty. A page with no CSS passes most checks trivially |
| `PREFLIGHT_SELECTOR` | exit 2 | The sheet is stale | A target selector matched zero elements on every page. A stale sheet reviews nothing while reporting green |
| `PREFLIGHT_FONTS` | warn | Reviewing the wrong typeface | `document.fonts.check('1rem "EB Garamond"')` false; screenshots get a `fallback-type` marker in the contact sheet |
| **`NO_OP`** | **error** | **Bug (a)** | The state's diff set against rest is empty, element and both pseudo-elements included. Per scheme cell, because the real bug existed in dark only |
| `WEAK_SIGNAL` | warn | A same-family shift with nothing beside it | Diff set is exactly `{color}` and the rest-to-state colour ratio is below `floors.weakSignal` (1.5). Gold to gold-sheen is 1.25:1, which the system itself calls the honest ceiling and pairs with a second signal |
| **`TOUCH_DEAD`** | **error** | **Bug (b)** | On the coarse-pointer profile, the touch-press union equals rest **and** focus-visible equals rest. A phone user gets no feedback from this control at all |
| `HOVER_ONLY` | warn | A signal that exists only where hover exists | The desktop profile shows a change, the coarse profile shows a change of strictly fewer properties, and `(hover:hover)` is false there (*ran it*: it is) |
| `REDUCE_KILLS_STATE` | error | A reduce rule that removed the feedback instead of the motion | A state that changed at no-preference has an empty diff under reduce. Reduce removes motion, never information |
| `REDUCE_LEAK` | error | Motion surviving the gate | Under reduce, any finite animation with active duration above 100ms within 2s of load. 100ms sits above the stylesheet's own `0.01ms` collapse (`styles.css:886`, *read the source*) and below the 90ms press, so neither is mistaken for a leak |
| `MOTION_5S` | error | WCAG 2.2.2 | Time from load until the animation queue is empty and a MutationObserver has been quiet for 500ms. Over 5000ms with no pause control fails |
| `CONTRAST_TEXT` | error | Below the floor | Sampled `color` versus the composited backdrop, per state, per scheme |
| `CONTRAST_NONTEXT` | error | 1.4.11 | Focus ring against the backdrop behind the offset gap; and a `::after` underline against the band when it is the state's only signal. Floor 3.0 |
| `CONTRAST_UNRELIABLE` | warn | An honest gap | A `background-image` in the ancestor chain |
| `TOKEN_FORK` | error | A surface redefining a token | Resolved `:root` custom properties versus the sheet's canonical table, per scheme |
| `LAW_RED_IN_DARK` | error | Ground law 2 | Any sampled colour in a dark cell equal to light `--accent` |
| `LAW_GOLD_ON_PAPER` | error | Ground law 1 | Any sampled colour in a light cell equal to a gold token, `--lamp-glow` exempted by the sheet |
| `TARGET_SIZE` | error under 24, warn under 44 | Hit areas | Bounding box on the coarse profile. 24x24 is the WCAG 2.5.8 AA floor; 44 is this system's own stated bar (*read the docs* for 24, *read the source* for 44) |
| `KEYBOARD_UNREACHABLE` | error | A target no keyboard can reach | Tab walk of up to 60 stops, stopping when `activeElement` becomes `BODY`; a sheet target never reached fails |
| `FOCUS_ORDER` | warn | Tab order against reading order | Tab sequence versus DOM order at each width |
| `OVERFLOW_X` | error | A horizontal scrollbar on a phone | `documentElement.scrollWidth > clientWidth` at any width |
| `EXPECT` | error | A sheet assertion | `expect` clause mismatch, with expected and actual |
| `ORPHANS` | warn | Something new that nobody put in the sheet | The deduped sweep |
| `STATE_NOT_ACHIEVED` | warn | The tool failing, not the design | `achieved:false`. Suppresses every error derived from that cell |
| `SETTLE_TIMEOUT` | warn | An unsettled sample | `settled:false`. Downgrades any `NO_OP` from that cell to a warning, because an unsettled sample looks exactly like a no-op (*ran it*: the held `:active` transform read as the rest value before the 90ms transition ran) |

### How the two shipped bugs surface

**(a) The dark-only hover no-op.** Cell `terms / wordmark / 1280 / dark / no-preference / hover`. Diff set against rest is empty: `color` is `--chrome-ink` gold at rest and `--accent` gold on hover, the same value, and `.wordmark a` has no `::after`. `NO_OP`, error. The same cell in `light` passes, so the finding reads "dark only", which is the shape of the actual bug. The report's per-scheme token table sits beside it showing `--chrome-ink` and `--accent` resolving to the same `rgb(201, 168, 96)`, which is the explanation without any rule-provenance machinery. The neighbouring `nav-item` cell in the same scheme passes on `after.transform` changing while colour does not, which I reproduced synthetically end to end (*ran it*).

**(b) The state with no effect on touch.** Cell `writing / nav-item / 360 / coarse / dark / touch-press`. If the union of the three touch samples equals rest and focus-visible also equals rest, `TOUCH_DEAD`, error, with the media facts attached: `(hover:hover) false`, `(pointer:coarse) true` (*ran it*). Neither finding is a picture anyone has to squint at.

---

## 7. Output

### Where

```
tools/eye/out/<sheet-name>/<runid>/report.json
                                   findings.json
                                   index.html
                                   shots/<page>__<target>__w<width>__<scheme>__<motion>__<state>.png
tools/eye/out/<sheet-name>/latest/  (a copy of the newest run)
```

`runid` is `20260820-231455-68cd83c-dirty`, timestamp plus short SHA plus a dirty marker, so a report can be tied to the tree it reviewed. `latest/` is a **copy, not a symlink**: creating a symlink on Windows needs developer mode or elevation, so a symlink turns a clean run into a permissions error on the machine this repository lives on (*read the docs*).

**Yes, gitignore it.** `tools/eye/.gitignore` currently ignores `node_modules/` and `shots/` (*read the source*, 2026-08-20). Replace `shots/` with `out/`, keeping the existing comment's reasoning, which is already correct and already ADR-0003's rule: **a render is evidence for one review at one commit and is reproducible from the sheet plus that commit, so it is not history.** The sheet itself is tracked, in `site/frontend/`, because the sheet is the definition of the review and a review nobody can rerun identically is an anecdote.

Two file names, three artifacts, deliberately:

- **`report.json`**, everything, one object per cell. Can reach a few hundred kilobytes.
- **`findings.json`**, only the findings plus the run header, typically under 10KB. **This is the file the agent reads.** Loading a full matrix into a context window to learn that one cell failed is the same waste as a screenshot nobody looks at.
- **`index.html`**, the contact sheet for Richard.

### Shapes

```jsonc
// findings.json
{
  "eye": "0.1.0",
  "run": { "id": "20260820-231455-68cd83c-dirty", "base": "http://localhost:5173",
           "complete": true, "trimmed": [], "durationMs": 118433,
           "cells": 612, "unsettled": 0, "notAchieved": 0 },
  "summary": { "errors": 1, "warnings": 3, "waived": 1,
               "byCheck": { "NO_OP": 1, "ORPHANS": 2, "TARGET_SIZE": 1 } },
  "findings": [
    {
      "check": "NO_OP", "severity": "error",
      "cell": "terms/wordmark/w1280/dark/no-preference/hover",
      "message": "hover changed nothing: every sampled property equals rest, including ::before and ::after.",
      "evidence": {
        "rest":  { "color": "rgb(201, 168, 96)", "after.content": "none" },
        "state": { "color": "rgb(201, 168, 96)", "after.content": "none" },
        "tokens": { "--chrome-ink": "rgb(201, 168, 96)", "--accent": "rgb(201, 168, 96)" },
        "alsoFailsAt": ["w360/dark", "w768/dark"],
        "passesAt":    ["w1280/light", "w768/light", "w360/light"]
      },
      "shots": ["shots/terms__wordmark__w1280__dark__no-preference__rest.png",
                "shots/terms__wordmark__w1280__dark__no-preference__hover.png"]
    }
  ],
  "waived": [ { "check": "WEAK_SIGNAL", "cell": "...", "because": "..." } ]
}
```

```jsonc
// report.json, one cell
{
  "id": "writing/nav-item/w360/dark/no-preference/touch-press",
  "page": "/blog/", "target": "nav-item", "selector": ".nav a:not([aria-current])",
  "width": 360, "pointer": "coarse", "scheme": "dark", "motion": "no-preference",
  "js": true, "state": "touch-press", "achieved": true, "settled": true,
  "styles": { "color": "rgb(201, 168, 96)", "transform": "matrix(1, 0, 0, 1, 0, 1)", "...": "..." },
  "pseudo": { "before": null, "after": { "transform": "matrix(0, 0, 0, 1, 0, 0)", "...": "..." } },
  "box": { "x": 92.5, "y": 18, "width": 54.7, "height": 35 },
  "backdrop": { "rgb": "rgb(18, 16, 13)", "from": "header.band", "reliable": true },
  "contrast": { "ratio": 8.37, "floor": 4.5, "isLargeText": false, "pass": true },
  "diffVsRest": { "transform": ["none", "matrix(1, 0, 0, 1, 0, 1)"] },
  "signalClasses": ["geometry"],
  "touchSampleThatCarriedIt": "afterRelease+150ms",
  "media": { "hoverHover": false, "pointerCoarse": true, "reduce": false },
  "shot": null
}
```

### D8. The screenshot budget

- **Every cell.** Nothing is unseen. Consequence: about 600 PNGs per run, tens of megabytes, and a folder that size is a folder nobody opens, which is the same outcome as capturing none.
- **Failures only.** Small and pointed. Consequence: Richard cannot look at the page unless something is already broken, and half the value of an eye is looking at a thing that passes.

**Picked: `context`, the default.** One full-page capture per `(page, width, scheme)` at rest, which is 3 x 5 x 2 = 30 for this sheet, plus a before/after element crop for every cell that produced a finding at any severity. `--shots all` and `--shots off` exist for the two extremes.

Element crops clip the bounding box padded by **8px**. Derivation: `outline-offset: 3px` plus `outline-width: 2px` puts the focus ring 5px outside the box, so 8 keeps the ring inside the crop with room. **If the system's focus ring grows, this number moves with it**, otherwise crops start cutting the ring off exactly when the ring is the thing being judged.

### D14. Device pixel ratio

- **1.** Smallest files, matches nothing anybody uses.
- **3.** Matches the 360px phone profile's real rendering. Consequence: full-page phone captures get large fast, and every crop pays for detail that only matters on hairlines.

**Picked: 2 by default, `--dpr 3` when judging hairlines.** The mechanism that rules out 1: a 1px hairline at dSF 1 can land on a half pixel and round away, and this system's separators, underlines and rules are all 1px, so a capture at 1 can show a hairline that does not exist or hide one that does.

### The contact sheet

`index.html`, no JavaScript, images by relative path, one table per page: rows are targets, columns are `(width x scheme)`, each cell shows the rest crop beside each state crop with a verdict chip under it, red for error, amber for warning, plain for pass. Light and dark sit side by side in the same row, because the bug that shipped was a difference between them and a difference is seen by adjacency.

**The contact sheet is styled in browser defaults plus a neutral grey, and imports nothing from the design system.** A review tool wearing the thing it reviews cannot show a result plainly: the token that is wrong is the token colouring the verdict.

---

## 8. The remaining decisions

### D7. Reduced motion: a spot check or a full axis

- **One reduce cell per scheme, rest and hover only.** Cheap, confirms the gate exists. Consequence: it misses the class of bug that matters, which is a reduce block that writes `transform: none` and removes the press itself rather than its transition. That bug is invisible unless the full state set runs under reduce.
- **Full axis: every state, every width, under reduce as well.** Consequence: double the state samples.

**Picked: the full axis, with screenshots off under reduce.** The cost argument is thin in the direction that matters: under reduce there is nothing to wait for, so those cells are the cheapest in the run. I sampled the hover end value immediately under `reducedMotion: "reduce"` with no settle at all and it was already final (*ran it*). Contrast does not depend on motion, so it is computed once at no-preference and reused, and screenshots at reduce would be duplicates of the no-preference rest frames.

### D9. Who starts the server

- **`eye` spawns vite and waits for the port.** One command, no "connection refused" confusion. Consequence: the tool owns a child process tree on Windows, where killing a tree is unreliable, and one orphaned vite holding 5173 breaks every later run in a way that looks like a bug in the eye.
- **`eye` requires a server and refuses to guess.** Consequence: a second terminal.

**Picked: require it.** `PREFLIGHT_BASE` exits 2 and prints the exact command. And say this in the sheet's comment, because it is a real trap: **`npm run dev` serves `/src/styles.css` unbundled; `npm run preview` serves what actually ships.** Iterate against dev, review against preview before anything goes out, because the built stylesheet is a different file (`dist/assets/styles-Ci6Rzl6J.css`, *read the source*) and the review that counts is the review of the artifact.

### D10. Naming the rule that lost

- **Values only.** The report says `hover == rest` and the agent greps the stylesheet for why. Consequence: one grep, and the tool stays small.
- **Rule provenance through CDP** (`DOM.getDocument`, `DOM.querySelector`, `CSS.getMatchedStylesForNode`), so the finding names file and line. Consequence: it binds the eye to Chromium's CSS domain forever, and on the built site the provenance points at the bundled stylesheet rather than the source, so the number it prints is a line in a file nobody edits.

**Picked: values only, plus the per-scheme resolved token table in every report.** In the real bug, the token table *is* the explanation: `--chrome-ink` and `--accent` both resolving to `rgb(201, 168, 96)` says the whole thing. The token table also does double duty as `TOKEN_FORK`. Provenance through CDP is the named upgrade path if a finding ever survives that a grep could not explain.

### D13. Waivers

- **An ignore list.** Two lines, keeps the run green. Consequence: it rots. Entries outlive their reasons and the list becomes the place findings go to die.
- **Reasoned waivers, validated at load, printed in their own section of every report.** Consequence: writing one takes a sentence.

**Picked: reasoned.** The loader throws if `because` is missing or shorter than 20 characters, which is exit 2, not a warning. The waived finding still appears in `findings.json` under `waived` with its reason, so `WEAK_SIGNAL` on the dark wordmark stays visible with the argument that settles it attached, exactly as `design-system.md` section 7 already argues it.

### Stateful targets

The lamp is a target whose `:active` state, released over itself, fires a click, flips `data-theme`, writes `localStorage`, and poisons every later cell in that context. Two mechanisms, both applied, because either alone is a bet:

1. **Release off the element.** A `click` only fires when press and release land on the same element, so `mouse.down()` on the lamp, sample, `mouse.move(0,0)`, `mouse.up()` produces `:active` and no click. *Read the docs, not run in this build.*
2. **Reload after.** Any target marked `stateful: true` runs last within its page context and is followed by `page.reload()`, which restores the scheme from the seeded storage regardless.

---

## 9. What it does not do, stated so nobody expects it

No pixel diffing against golden images: a golden-image suite fails on every intentional change and trains its owner to approve diffs unread, which is worse than not having it. No cross-browser: Firefox and WebKit are in the Playwright package, and the day a WebKit-only bug matters the axis is one context option, but a matrix that already has six axes does not get a seventh for free. No screen reader output: the accessibility tree is readable through CDP and announcement order is not, so a check here would be theatre. No judgement of hierarchy, rhythm or proportion. **The eye reports what is on the screen. What it means is Aldus's job**, and a tool that claimed otherwise would be the fashion mistake his constitution warns about.

---

## 10. Honest status

**What has been run.** `eye.mjs` exists and implements the matrix, the preflight, `NO_OP`,
`TOUCH_DEAD`, `HOVER_ONLY`, `WEAK_SIGNAL`, `CONTRAST_TEXT`, `UNMEASURED` and the per-page
overflow check. It has been run against the band on all three pages, at 320, 360, 768 and 1280,
at root font sizes 16, 20 and 24, in both schemes, on a fine and a coarse pointer.

The mechanisms this specification was built on, and which the implementation then confirmed by
failing without them: sampling after the transition settles rather than during it,
pseudo-element sampling, `:focus-visible` not matching programmatic focus so the walk has to be
real Tab presses, the coarse-pointer media facts, and headless capture with no visible pane.

**What is designed and not implemented.** The tool prints this list on every run rather than
letting a green result imply it: the reduced-motion axis, `REDUCE_KILLS_STATE`, `REDUCE_LEAK`,
`MOTION_5S`, `CONTRAST_NONTEXT`, the no-JavaScript pass, waivers and the contact sheet. The
runtime estimate in section 4 is still a guess.

**A limit found by a reviewer and not by the tool, which is the honest place to record it.**
This tool diffs computed styles, and a computed value can change while nothing renders: a
`transform` on a non-replaced inline box resolves to a matrix and moves no pixels. A state can
therefore pass `NO_OP` while being invisible to a person. Sampling the bounding box alongside
the computed set would close it, and until that exists a passing `NO_OP` means the declaration
changed, not that the reader saw anything.

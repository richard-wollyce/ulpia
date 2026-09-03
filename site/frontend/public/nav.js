// The two things <details> does not do, and nothing else.
//
// The door itself is a native disclosure: click, Enter, Space and touch all
// open it with no script at all, which is why it is <details> and not a button
// wearing a costume. What the element genuinely lacks is the pair this system's
// menu rules require: Escape closes and returns focus to the door, and a click
// anywhere outside closes.
//
// Both are additions, never preconditions. With this file blocked, missing or
// still in flight, the menu opens and navigates exactly as it should; the two
// dismissals are the part a reader loses, and losing them costs a second click
// rather than a destination.
(function () {
  "use strict";

  document.addEventListener("DOMContentLoaded", function () {
    var menu = document.querySelector(".nav-menu");
    if (!menu) return;
    var door = menu.querySelector(".nav-door");
    if (!door) return;

    // Escape closes and returns focus to the door. Returning focus is the half
    // people skip: without it the caret is dismissed and the keyboard is left
    // at the top of the document, so the next Tab restarts the page instead of
    // continuing from the control that was just used.
    document.addEventListener("keydown", function (event) {
      if (event.key !== "Escape" || !menu.open) return;
      menu.open = false;
      door.focus();
    });

    // A click outside closes. `composedPath` rather than `event.target` so a
    // click landing inside the caret, which is a pseudo-element on the door,
    // still counts as inside.
    document.addEventListener("click", function (event) {
      if (!menu.open) return;
      var path = event.composedPath ? event.composedPath() : [event.target];
      if (path.indexOf(menu) === -1) menu.open = false;
    });

    // Focus leaving the menu closes it, which is the keyboard's equivalent of
    // the click above. `focusout` fires before focus lands, so the new target
    // is read from relatedTarget rather than from the document.
    menu.addEventListener("focusout", function (event) {
      if (!menu.open) return;
      var next = event.relatedTarget;
      if (next && menu.contains(next)) return;
      // A focusout with no relatedTarget is the window losing focus, not a move
      // inside the page, and closing on it would shut the menu every time the
      // reader switched applications.
      if (!next) return;
      menu.open = false;
    });
  });
})();

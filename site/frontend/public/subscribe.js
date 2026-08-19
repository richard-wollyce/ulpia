// The write flow (spec 22.F): one click hands the visitor our address and then
// offers the list. The dialog never waits on the clipboard, and the visitor ends
// better off than before the click whether the copy succeeded or not, because the
// address is on screen, selectable, with a Copy button to retry.
(function () {
  var ADDRESS = "hello@ulpia.io";

  // No toast: the dialog body states the outcome in permanent text, and the
  // address row with its Copy button is right there. A floating receipt saying
  // what the card already says is redundancy, not reassurance.
  var BODY_FAIL =
    "Our address is " + ADDRESS + ", and your browser would not let us copy it " +
    "for you. This is the other direction: leave yours and we will tell you when " +
    "there is something you can install. Rarely otherwise, and never more than " +
    "once a month.";
  var SUCCESS_FAIL_TAIL =
    "We will write when there is something you can install. If you have " +
    "something to say before then, we are at " + ADDRESS;

  document.addEventListener("DOMContentLoaded", function () {
    var cta = document.querySelector(".cta-primary");
    var dialog = document.getElementById("write-dialog");
    if (!cta || !dialog || typeof dialog.showModal !== "function") return;

    var card = dialog.querySelector(".dialog-card");
    var form = document.getElementById("newsletter");
    var email = document.getElementById("email");
    var consent = document.getElementById("consent");
    var submit = form.querySelector(".submit");
    var addressNode = document.getElementById("write-address");
    var submitLabel = submit.textContent;

    function selectAddress() {
      var range = document.createRange();
      range.selectNodeContents(addressNode);
      var sel = window.getSelection();
      sel.removeAllRanges();
      sel.addRange(range);
    }

    function copyAddress(announce) {
      // Never awaited before the dialog opens: a control must not be slower than
      // its effect, and the clipboard API can hang on permission prompts.
      if (!navigator.clipboard || !navigator.clipboard.writeText) {
        if (announce) failed();
        return;
      }
      navigator.clipboard.writeText(ADDRESS).then(function () {}, function () {
        if (announce) failed();
      });
    }

    // The body text carries the failure, because the visitor must end better off
    // than before the click either way.
    function failed() {
      var body = document.getElementById("dialog-body");
      if (body) body.textContent = BODY_FAIL;
      var success = document.getElementById("success-body");
      if (success) success.textContent = SUCCESS_FAIL_TAIL;
      selectAddress();
    }

    cta.addEventListener("click", function (event) {
      // A modified click asked for the mail client, not for our dialog.
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      if (typeof event.button === "number" && event.button !== 0) return;
      event.preventDefault();
      dialog.showModal();
      card.focus();
      copyAddress(true);
    });

    document.querySelector(".addr-copy").addEventListener("click", function () {
      copyAddress(true);
    });

    dialog.querySelector(".dialog-close").addEventListener("click", function () {
      dialog.close();
    });

    // Three exits, because a visitor whose primary goal already succeeded must
    // never feel held.
    dialog.querySelector(".dismiss").addEventListener("click", function () {
      dialog.close();
    });

    // A click that lands on the dialog element itself is a click on the backdrop:
    // the card covers everything else.
    dialog.addEventListener("click", function (event) {
      if (event.target === dialog) dialog.close();
    });

    dialog.addEventListener("close", function () {
      cta.focus();
    });

    // The links inside the label must not toggle the box on their way out.
    Array.prototype.forEach.call(document.querySelectorAll(".consent a"), function (link) {
      link.addEventListener("click", function (event) {
        event.stopPropagation();
      });
    });

    function clearInvalid() {
      form.classList.remove("invalid");
      email.removeAttribute("aria-invalid");
    }

    email.addEventListener("input", clearInvalid);
    consent.addEventListener("change", function () {
      form.classList.remove("no-consent");
    });

    form.addEventListener("submit", function (event) {
      event.preventDefault();
      card.classList.remove("state-error");

      // Deliberately permissive: the mail server is the authority on whether an
      // address exists, so this rejects only the obviously malformed.
      var typed = email.value.trim();
      if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(typed)) {
        document.getElementById("email-msg").textContent = typed
          ? "That does not look like an email address."
          : "We need an address to write to.";
        form.classList.add("invalid");
        email.setAttribute("aria-invalid", "true");
        email.focus();
        return;
      }
      clearInvalid();
      if (!consent.checked) {
        form.classList.add("no-consent");
        consent.focus();
        return;
      }
      form.classList.remove("no-consent");

      submit.disabled = true;
      submit.textContent = "Sending";
      form.setAttribute("aria-busy", "true");

      fetch("/api/subscribe", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ email: email.value.trim(), consent: true }),
      })
        .then(function (response) {
          return response.json().then(function (body) {
            return { ok: response.ok, body: body };
          });
        })
        .then(function (result) {
          if (!result.ok) throw new Error(result.body.code || "server_error");
          card.classList.add("state-success");
          var heading = card.querySelector(".success h3");
          if (heading) heading.focus();
        })
        .catch(function () {
          // The typed address and the consent state survive the failure: losing
          // someone's typing to show them an error is how a retry stops happening.
          card.classList.add("state-error");
        })
        .then(function () {
          submit.disabled = false;
          submit.textContent = submitLabel;
          form.removeAttribute("aria-busy");
        });
    });
  });
})();

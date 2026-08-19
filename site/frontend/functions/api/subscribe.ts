// POST /api/subscribe
//
// The list is ours: this writes to our own D1 database and nothing leaves it.
// Sending is deliberately not built yet, because there is no newsletter to send
// until there is something to say, and sending is the only decision here that is
// expensive to reverse (IP reputation is accumulated, not configured). Capturing
// first keeps the list portable and defers that decision to the day it matters.
//
// Single opt-in with an explicit consent checkbox, recorded with the version of
// the text that was agreed to. Double opt-in arrives with the sending path, since
// a confirmation email cannot be sent by a system that cannot send.

interface Env {
  DB: D1Database;
}

// Bumped whenever the consent sentence changes, so a stored consent can always be
// traced to the exact words it was given for.
const CONSENT_VERSION = "2026-08-19";

// Deliberately permissive: the authority on whether an address exists is the mail
// server, not a regex. This rejects the obviously malformed and nothing else,
// because a validator stricter than the specification rejects real people.
const LOOKS_LIKE_EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

const json = (status: number, body: Record<string, unknown>) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json; charset=utf-8" },
  });

export const onRequestPost: PagesFunction<Env> = async ({ request, env }) => {
  let email = "";
  let consent = false;
  let honeypot = "";

  // Accepts JSON from the modal and form encoding from a native submit, so the
  // endpoint does not depend on the client having JavaScript.
  const type = request.headers.get("content-type") ?? "";
  if (type.includes("application/json")) {
    const body = (await request.json().catch(() => ({}))) as Record<string, unknown>;
    email = String(body.email ?? "").trim();
    consent = body.consent === true || body.consent === "on";
    honeypot = String(body.website ?? "");
  } else {
    const form = await request.formData().catch(() => null);
    email = String(form?.get("email") ?? "").trim();
    consent = form?.get("consent") != null;
    honeypot = String(form?.get("website") ?? "");
  }

  // The honeypot is a field no human sees and no human fills. Answering 201 to a
  // bot that filled it costs nothing and teaches it nothing, which is the point:
  // a bot told it failed comes back differently.
  if (honeypot) return json(201, { ok: true, code: "subscribed" });

  if (!email || email.length > 254 || !LOOKS_LIKE_EMAIL.test(email)) {
    return json(400, { ok: false, code: "invalid_email" });
  }
  if (!consent) {
    return json(400, { ok: false, code: "consent_required" });
  }

  // Addresses are stored lowercased so the primary key does the deduplicating.
  // No IP address is recorded: abuse is handled at the edge, and collecting a
  // second piece of personal data to protect the first is a bad trade.
  const normalized = email.toLowerCase();

  try {
    const result = await env.DB.prepare(
      "INSERT INTO subscribers (email, created_at, consent_version) VALUES (?, ?, ?) ON CONFLICT(email) DO NOTHING",
    )
      .bind(normalized, new Date().toISOString(), CONSENT_VERSION)
      .run();

    const created = (result.meta?.changes ?? 0) > 0;
    return json(created ? 201 : 200, {
      ok: true,
      code: created ? "subscribed" : "already_subscribed",
    });
  } catch {
    // The visitor gets a state they can act on; the detail stays server side.
    return json(500, { ok: false, code: "server_error" });
  }
};

// Anything that is not a POST is answered honestly rather than falling through to
// the static 404, which would be a lie about what lives at this path.
export const onRequest: PagesFunction<Env> = async ({ request, next }) => {
  if (request.method === "POST") return next();
  return json(405, { ok: false, code: "method_not_allowed" });
};

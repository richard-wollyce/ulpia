const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { open } = window.__TAURI__.dialog;
const { getCurrentWindow } = window.__TAURI__.window;

const $ = (id) => document.getElementById(id);
const note = (text, bad) => {
  $("note").textContent = text || "";
  $("note").className = bad ? "note bad" : "note";
};

// ---------------------------------------------------------------------------
// Progress
//
// The backend owns this state and the panel only draws it. That is what lets a
// drop survive the panel being dismissed: reopening asks for the current state
// rather than starting from an empty screen while the tray still shows work.
// ---------------------------------------------------------------------------

let shownStage = null;

function drawProgress(p) {
  const running = Boolean(p.stage) || Boolean(p.problem);
  $("progress").className = running ? "progress" : "progress idle";

  if (p.problem) {
    note(p.problem, true);
    shownStage = null;
    $("stage").innerHTML = "";
    $("bar").className = "bar hidden";
    return;
  }

  if (p.stage !== shownStage) {
    shownStage = p.stage;
    const old = $("stage").lastElementChild;
    if (old) {
      // Let the outgoing line finish leaving before it is removed, or the swap
      // reads as a flicker rather than as one thing replacing another.
      old.classList.add("leaving");
      old.addEventListener("animationend", () => old.remove(), { once: true });
    }
    if (p.stage) {
      const span = document.createElement("span");
      span.textContent = p.stage;
      $("stage").append(span);
    }
  }

  // A bar only where there is a real denominator. Generation has none: tokens per
  // second is knowable and the total is not, and a percentage that guesses lies.
  if (p.total > 0) {
    $("bar").className = "bar";
    $("fill").style.width = `${Math.round((p.done / p.total) * 100)}%`;
  } else {
    $("bar").className = "bar hidden";
  }
}

listen("progress", ({ payload }) => drawProgress(payload));

// ---------------------------------------------------------------------------
// Fleet
// ---------------------------------------------------------------------------

async function refresh() {
  const s = await invoke("status");
  if (!s.root) {
    $("sub").textContent = "sem frota";
    note("Nenhuma frota escolhida. Clique aqui para escolher a pasta.", true);
    $("note").onclick = chooseFleet;
    return;
  }
  $("note").onclick = null;
  $("sub").textContent = s.agents.length
    ? `${s.agents.join(", ")} · ${s.entries} entradas`
    : "frota vazia";
  note(s.problem || "", Boolean(s.problem));
}

async function chooseFleet() {
  const dir = await open({ directory: true, title: "Onde fica a frota?" });
  if (dir) {
    await invoke("set_fleet_root", { path: dir });
    refresh();
  }
}

async function sendFiles(paths) {
  try {
    note(await invoke("accept_files", { paths }));
  } catch (e) {
    note(String(e), true);
  }
}

async function pickFiles() {
  const files = await open({ multiple: true, title: "Arquivos para a inbox" });
  if (files) await sendFiles([].concat(files));
}

// ---------------------------------------------------------------------------
// Answers
// ---------------------------------------------------------------------------

function render(answers) {
  $("results").innerHTML = "";
  for (const a of answers) {
    const el = document.createElement("div");
    el.className = "hit";
    const h = document.createElement("h3");
    h.textContent = a.title || a.path;
    const meta = document.createElement("div");
    meta.className = "meta";
    meta.textContent = `${a.agent}/${a.path} · ${a.why}`;
    el.append(h, meta);
    // Only the first passage. The panel is 360px wide; the rest belongs in a
    // window with room for it rather than in a scroll nobody reads.
    if (a.passages[0]) {
      const p = document.createElement("p");
      p.textContent = a.passages[0].text;
      el.append(p);
    }
    $("results").append(el);
  }
}

async function ask(question) {
  try {
    render(await invoke("ask", { question }));
    note("");
  } catch (e) {
    $("results").innerHTML = "";
    note(String(e), true);
  }
}

listen("answer", ({ payload }) => ask(payload));

// ---------------------------------------------------------------------------
// Wiring
// ---------------------------------------------------------------------------

$("pick").onclick = pickFiles;
$("write").onclick = () => invoke("open_compose");
$("drop").ondragover = (e) => e.preventDefault();

// Tauri owns the OS drag and drop when dragDropEnabled is set, so the HTML5
// events never fire and these are the ones that do.
getCurrentWindow().onDragDropEvent(({ payload }) => {
  $("drop").classList.toggle("over", payload.type === "over");
  if (payload.type === "drop") sendFiles(payload.paths);
});

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") getCurrentWindow().hide();
  if (e.ctrlKey && e.key === "o") { e.preventDefault(); pickFiles(); }
  if (e.ctrlKey && e.key === "k") { e.preventDefault(); invoke("open_compose"); }
});

refresh();
invoke("progress_now").then(drawProgress);

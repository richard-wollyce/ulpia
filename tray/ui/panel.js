const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { open } = window.__TAURI__.dialog;
const { getCurrentWindow } = window.__TAURI__.window;

const $ = (id) => document.getElementById(id);
const note = (text, bad) => {
  $("note").textContent = text || "";
  $("note").className = bad ? "note bad" : "note";
};

async function refresh() {
  const s = await invoke("status");
  if (!s.root) {
    $("sub").textContent = "sem frota";
    note("Nenhuma frota escolhida. Clique para escolher a pasta.", true);
    $("note").onclick = chooseFleet;
    return;
  }
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

async function pickFiles() {
  const files = await open({ multiple: true, title: "Arquivos para a inbox" });
  if (files) await invoke("accept_files", { paths: [].concat(files) });
}

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
    // window that has room for it rather than in a scroll nobody reads.
    if (a.passages[0]) {
      const p = document.createElement("p");
      p.textContent = a.passages[0].text;
      el.append(p);
    }
    $("results").append(el);
  }
}

window.ask = async function (question) {
  try {
    note("procurando…");
    render(await invoke("ask", { question }));
    note("");
  } catch (e) {
    $("results").innerHTML = "";
    note(String(e), true);
  }
};

$("pick").onclick = pickFiles;
$("write").onclick = () => invoke("open_compose").catch(() => {});
$("drop").ondragover = (e) => e.preventDefault();

// Tauri owns the OS drag and drop when dragDropEnabled is set, so the HTML5
// events never fire and these are the events that do.
getCurrentWindow().onDragDropEvent(({ payload }) => {
  if (payload.type === "over") $("drop").classList.add("over");
  else $("drop").classList.remove("over");
  if (payload.type === "drop") {
    invoke("accept_files", { paths: payload.paths }).then((r) => note(r));
  }
});

listen("progress", ({ payload }) => {
  if (payload < 100) note(`processando ${payload}%`);
});

listen("answer", ({ payload }) => window.ask(payload));

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") getCurrentWindow().hide();
  if (e.ctrlKey && e.key === "o") { e.preventDefault(); pickFiles(); }
  if (e.ctrlKey && e.key === "k") { e.preventDefault(); invoke("open_compose").catch(() => {}); }
});

refresh();

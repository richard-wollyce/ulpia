const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;
const win = getCurrentWindow();

const text = document.getElementById("text");
const send = document.getElementById("send");

async function submit() {
  const question = text.value.trim();
  if (!question) {
    // Do not close on an empty send. A window that vanishes without doing anything
    // reads as a crash.
    text.placeholder = "Escreva algo primeiro…";
    return;
  }
  text.value = "";
  await invoke("ask_from_compose", { question });
  win.hide();
}

send.onclick = submit;

text.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); submit(); }
  if (e.key === "Escape") win.hide();
});

win.onFocusChanged(({ payload }) => { if (payload) text.focus(); });
text.focus();

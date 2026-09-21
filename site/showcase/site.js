const tree = document.querySelector("#explanation-tree");
const buttons = [...document.querySelectorAll("[data-mode]")];

const data = window.__BELIEF_DEMO__;
if (!data) throw new Error("Belief Lab demo data is missing.");

document.querySelector("#claim-label").textContent = data.claim.label;
document.querySelector("#claim-score").textContent = data.claim.score.toFixed(2);

function render(mode) {
  const rows = data[mode];
  tree.replaceChildren(
    ...rows.map((row) => {
      const node = document.createElement("div");
      node.className = "tree-node";
      node.style.setProperty("--depth", row.depth);

      const title = document.createElement("strong");
      title.textContent = row.label;

      const meta = document.createElement("div");
      meta.className = "tree-meta";
      [row.kind, row.score, row.producer, row.correlation && `group: ${row.correlation}`]
        .filter(Boolean)
        .forEach((value) => {
          const chip = document.createElement("span");
          chip.textContent = value;
          meta.append(chip);
        });

      node.append(title, meta);
      return node;
    }),
  );
  buttons.forEach((button) =>
    button.classList.toggle("active", button.dataset.mode === mode),
  );
}

buttons.forEach((button) =>
  button.addEventListener("click", () => render(button.dataset.mode)),
);
render("summary");

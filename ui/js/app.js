// Application bootstrap: mode toggle, trees, preview, task display.
(function () {
  let mode = "manage"; // 'manage' | 'preview'
  let trees = {}; // id -> FileTree
  let currentTask = null;
  let taskTimer = null;
  let clearTaskTimer = null;

  function $id(id) { return document.getElementById(id); }

  function updateModeUI() {
    const btn = $id("mode-toggle");
    if (mode === "manage") {
      $id("pane-manage").classList.remove("d-none");
      $id("pane-preview").classList.add("d-none");
      btn.textContent = "切换到预览模式";
    } else {
      $id("pane-manage").classList.add("d-none");
      $id("pane-preview").classList.remove("d-none");
      btn.textContent = "切换到管理模式";
    }
  }

  function initTrees() {
    document.querySelectorAll(".qf-tree-container").forEach((container) => {
      const id = container.dataset.treeId;
      const tree = new FileTree(container, {
        id,
        onSelect: (t, entry) => {
          if (mode === "preview" && id === "single") {
            PreviewPane.load(entry.path);
          }
        },
      });
      trees[id] = tree;
    });
  }

  function renderTask() {
    const area = $id("task-area");
    if (!currentTask) {
      area.innerHTML = '<span class="text-secondary">无后台任务</span>';
      return;
    }
    const elapsed = Math.max(0, Math.floor((Date.now() - currentTask.startedAtMs) / 1000));
    const status = currentTask.status === "running" ? "进行中"
                 : currentTask.status === "done" ? "已完成"
                 : "失败";
    area.innerHTML = `
      <span class="qf-task">
        <span>${QF.escapeHtml(currentTask.description)}</span>
        <span class="text-muted">${elapsed}s</span>
        <span class="badge bg-${currentTask.status === "running" ? "primary" : currentTask.status === "done" ? "success" : "danger"}">${status}</span>
      </span>`;
  }

  function startTaskTimer() {
    if (taskTimer) return;
    taskTimer = setInterval(renderTask, 500);
  }
  function stopTaskTimer() {
    if (taskTimer) { clearInterval(taskTimer); taskTimer = null; }
  }

  async function initTaskListeners() {
    QF.listen("task://started", (ev) => {
      currentTask = Object.assign({}, ev.payload, { status: "running" });
      if (clearTaskTimer) { clearTimeout(clearTaskTimer); clearTaskTimer = null; }
      startTaskTimer();
      renderTask();
    });
    QF.listen("task://finished", (ev) => {
      currentTask = Object.assign({}, ev.payload, { status: "done" });
      stopTaskTimer();
      renderTask();
      // Refresh views relevant to task: simple approach — refresh expanded folders in all trees.
      Object.values(trees).forEach((t) => t.refreshOpen());
      clearTaskTimer = setTimeout(() => { currentTask = null; renderTask(); clearTaskTimer = null; }, 5000);
    });
    QF.listen("task://failed", (ev) => {
      const code = (ev.payload && ev.payload.code) || "ERROR";
      const message = (ev.payload && ev.payload.message) || "未知错误";
      if (currentTask) currentTask.status = "failed";
      stopTaskTimer();
      renderTask();
      QF.showError({ code, message });
      clearTaskTimer = setTimeout(() => { currentTask = null; renderTask(); clearTaskTimer = null; }, 5000);
    });

    // In case a task is still running when we start.
    try {
      const t = await QF.invoke("current_task");
      if (t) {
        currentTask = Object.assign({}, t, { status: "running" });
        startTaskTimer();
        renderTask();
      }
    } catch (e) { /* ignore */ }
  }

  function initModeToggle() {
    $id("mode-toggle").addEventListener("click", () => {
      mode = mode === "manage" ? "preview" : "manage";
      updateModeUI();
      if (mode === "preview" && trees.single && trees.single.selectedNode) {
        PreviewPane.load(trees.single.selectedNode.path);
      }
    });
  }

  async function start() {
    // Load icon manifest before creating any tree/preview so the first render uses Material icons.
    try { await QF.Icons.init(); } catch (_) { /* IconResolver handles its own fallback */ }
    PreviewPane.init($id("preview-pane"));
    updateModeUI();
    initModeToggle();
    initTrees();
    initResizers();
    Shortcuts.init();
    initTaskListeners();
  }

  function initResizers() {
    document.querySelectorAll(".qf-pane").forEach((pane) => {
      const divider = pane.querySelector(".qf-divider");
      if (!divider) return;
      divider.addEventListener("mousedown", (e) => startResize(e, pane, divider));
    });
  }

  function startResize(e, pane, divider) {
    e.preventDefault();
    const rect = pane.getBoundingClientRect();
    const dividerWidth = divider.getBoundingClientRect().width;
    const minLeft = 120;
    const minRight = 120;
    divider.classList.add("dragging");
    document.body.classList.add("qf-resizing");

    const onMove = (ev) => {
      let left = ev.clientX - rect.left;
      const maxLeft = rect.width - dividerWidth - minRight;
      if (left < minLeft) left = minLeft;
      if (left > maxLeft) left = maxLeft;
      const right = rect.width - dividerWidth - left;
      pane.style.gridTemplateColumns = `${left}px ${dividerWidth}px ${right}px`;
    };
    const onUp = () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      divider.classList.remove("dragging");
      document.body.classList.remove("qf-resizing");
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start);
  } else {
    start();
  }
})();

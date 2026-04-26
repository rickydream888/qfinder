// Wrapper around the Tauri 2 invoke / event API. Surfaces a Promise-based API.
(function (global) {
  function getCore() {
    if (global.__TAURI__ && global.__TAURI__.core) return global.__TAURI__.core;
    throw new Error("Tauri core API is not available");
  }
  function getEvent() {
    if (global.__TAURI__ && global.__TAURI__.event) return global.__TAURI__.event;
    throw new Error("Tauri event API is not available");
  }

  const QF = {
    invoke(cmd, args) {
      return getCore().invoke(cmd, args || {});
    },
    listen(event, cb) {
      return getEvent().listen(event, cb);
    },
    convertFileSrc(path) {
      const core = getCore();
      if (typeof core.convertFileSrc === "function") return core.convertFileSrc(path);
      return path;
    },
    formatBytes(bytes) {
      if (bytes == null) return "—";
      const units = ["B", "KB", "MB", "GB", "TB", "PB"];
      let i = 0;
      let v = Number(bytes);
      while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
      return (i === 0 ? v.toFixed(0) : v.toFixed(2)) + " " + units[i];
    },
    showDialog(title, body) {
      $("#qf-dialog-title").text(title);
      $("#qf-dialog-body").html(body);
      const m = bootstrap.Modal.getOrCreateInstance(document.getElementById("qf-dialog"));
      m.show();
    },
    showError(err) {
      const code = (err && err.code) || "ERROR";
      const msg = (err && err.message) || String(err);
      QF.showDialog("操作失败", `<div><div class="text-danger small">[${code}]</div><div>${escapeHtml(msg)}</div></div>`);
    },
    showInfo(msg) { QF.showDialog("提示", `<div>${escapeHtml(msg)}</div>`); },
    showBusy() { QF.showDialog("提示", "已有后台任务正在运行，暂不支持添加新的任务。"); },
    showChoice(title, message, choices) {
      // choices: [{label, value, variant}]; resolves to chosen value (null if dismissed).
      return new Promise((resolve) => {
        const m = document.getElementById("qf-dialog");
        m.querySelector("#qf-dialog-title").textContent = title;
        m.querySelector("#qf-dialog-body").innerHTML = `<div>${escapeHtml(message)}</div>`;
        const footer = m.querySelector(".modal-footer");
        const originalFooter = footer.innerHTML;
        footer.innerHTML = "";
        let chosen = null;
        choices.forEach((c) => {
          const btn = document.createElement("button");
          btn.type = "button";
          btn.className = `btn btn-${c.variant || "secondary"}`;
          btn.textContent = c.label;
          btn.setAttribute("data-bs-dismiss", "modal");
          btn.addEventListener("click", () => { chosen = c.value; });
          footer.appendChild(btn);
        });
        const inst = bootstrap.Modal.getOrCreateInstance(m);
        const onHidden = () => {
          m.removeEventListener("hidden.bs.modal", onHidden);
          footer.innerHTML = originalFooter;
          resolve(chosen);
        };
        m.addEventListener("hidden.bs.modal", onHidden);
        inst.show();
      });
    },
    async runFsOp(cmd, args) {
      try {
        return await QF.invoke(cmd, args);
      } catch (err) {
        if (err && err.code === "BUSY_TASK") { QF.showBusy(); return null; }
        if (err && err.code === "CONFLICT") {
          const isDir = !!err.isDir;
          const path = err.path || "";
          const message = `目标位置已存在同名${isDir ? "目录" : "文件"}：${path}`;
          const choices = isDir
            ? [
                { label: "合并目录", value: "merge", variant: "primary" },
                { label: "替换目录", value: "replace", variant: "danger" },
                { label: "取消", value: null, variant: "secondary" },
              ]
            : [
                { label: "替换文件", value: "replace", variant: "danger" },
                { label: "取消", value: null, variant: "secondary" },
              ];
          const choice = await QF.showChoice("目标已存在", message, choices);
          if (!choice) return null;
          try {
            return await QF.invoke(cmd, Object.assign({}, args, { onConflict: choice }));
          } catch (e2) {
            if (e2 && e2.code === "BUSY_TASK") QF.showBusy();
            else QF.showError(e2);
            return null;
          }
        }
        QF.showError(err);
        return null;
      }
    },
    escapeHtml,
  };

  function escapeHtml(s) {
    return String(s).replace(/[&<>"']/g, (c) => ({
      "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
    }[c]));
  }

  global.QF = QF;
})(window);

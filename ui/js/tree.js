// FileTree component. Each instance manages an independent tree.
(function (global) {
  const DBLCLICK_MS = 400;
  const RENAME_DELAY_MS = 450; // delay before single-click-after-select triggers rename

  let allTrees = [];

  function FileTree(container, options) {
    this.container = container;
    this.options = options || {};
    this.id = options.id || "tree";
    this.showHidden = false;
    this.selectedNode = null; // {path, isDir, el}
    this.lastClickAt = 0;
    this.lastClickPath = null;
    this.pendingRenameTimer = null;
    this.nodes = new Map(); // path -> { entry, expanded, childrenLoaded }
    this.render();
    allTrees.push(this);
  }

  FileTree.all = function () { return allTrees; };

  FileTree.prototype.render = function () {
    const self = this;
    this.container.innerHTML =
      `<div class="qf-tree-header">
         <div class="form-check">
           <input class="form-check-input" type="checkbox" id="hidden-${this.id}">
           <label class="form-check-label" for="hidden-${this.id}">显示隐藏目录和文件</label>
         </div>
       </div>
       <div class="qf-tree-scroll" tabindex="0"></div>`;
    this.scroll = this.container.querySelector(".qf-tree-scroll");
    const cb = this.container.querySelector(`#hidden-${this.id}`);
    cb.addEventListener("change", () => {
      self.showHidden = cb.checked;
      self.refreshOpen();
    });
    this.scroll.addEventListener("click", (e) => self.handleClick(e));
    this.scroll.addEventListener("dblclick", (e) => self.handleDblClick(e));
    this.scroll.addEventListener("dragover", (e) => self.handleDragOver(e));
    this.scroll.addEventListener("dragleave", (e) => self.handleDragLeave(e));
    this.scroll.addEventListener("drop", (e) => self.handleDrop(e));
    this.scroll.addEventListener("focus", () => { self.hasFocus = true; });
    this.scroll.addEventListener("blur", () => { self.hasFocus = false; });
    this.loadRoots();
  };

  FileTree.prototype.loadRoots = async function () {
    try {
      const roots = await QF.invoke("list_roots");
      this.scroll.innerHTML = "";
      for (const r of roots) {
        const entry = { name: r.label, path: r.path, isDir: true, isHidden: false, isRoot: true };
        const el = this.createNodeEl(entry, 0);
        this.scroll.appendChild(el);
      }
    } catch (err) { QF.showError(err); }
  };

  FileTree.prototype.createNodeEl = function (entry, depth) {
    const wrap = document.createElement("div");
    wrap.className = "qf-tree-item";
    wrap.dataset.path = entry.path;

    const node = document.createElement("div");
    node.className = "qf-node";
    node.dataset.path = entry.path;
    node.dataset.isDir = entry.isDir ? "1" : "0";
    node.dataset.depth = String(depth);
    node.style.paddingLeft = (6 + depth * 16) + "px";
    if (entry.isDir && !entry.isRoot) node.draggable = true;
    if (entry.isRoot) node.draggable = false;
    if (!entry.isDir) node.draggable = true;

    const toggle = document.createElement("span");
    toggle.className = "qf-toggle";
    toggle.textContent = entry.isDir ? "▶" : "";
    node.appendChild(toggle);

    const icon = document.createElement("span");
    icon.className = "qf-icon";
    icon.innerHTML = renderIcon(entry, false);
    node.appendChild(icon);

    const label = document.createElement("span");
    label.className = "qf-label";
    label.textContent = entry.name;
    node.appendChild(label);

    node.addEventListener("dragstart", (e) => this.handleDragStart(e, entry));

    wrap.appendChild(node);

    const children = document.createElement("div");
    children.className = "qf-children d-none";
    wrap.appendChild(children);

    this.nodes.set(entry.path, { entry, depth, expanded: false, childrenLoaded: false, el: wrap, nodeEl: node, childrenEl: children });
    return wrap;
  };

  FileTree.prototype.handleClick = function (e) {
    const nodeEl = e.target.closest(".qf-node");
    if (!nodeEl) return;
    if (e.target.classList.contains("qf-toggle")) {
      const path = nodeEl.dataset.path;
      const meta = this.nodes.get(path);
      if (meta && meta.entry.isDir) this.toggleNode(path);
      return;
    }

    const path = nodeEl.dataset.path;
    const meta = this.nodes.get(path);
    if (!meta) return;

    const now = Date.now();
    const wasSelected = this.selectedNode && this.selectedNode.path === path;
    const dt = now - this.lastClickAt;
    this.lastClickAt = now;
    this.lastClickPath = path;

    // If user clicks an already-selected node and it's not a likely double-click, schedule rename.
    if (wasSelected && dt > DBLCLICK_MS && !meta.entry.isRoot) {
      const self = this;
      if (this.pendingRenameTimer) clearTimeout(this.pendingRenameTimer);
      this.pendingRenameTimer = setTimeout(() => {
        self.pendingRenameTimer = null;
        self.beginRename(path);
      }, RENAME_DELAY_MS);
      return;
    }

    this.selectNode(path);
  };

  FileTree.prototype.handleDblClick = function (e) {
    if (this.pendingRenameTimer) {
      clearTimeout(this.pendingRenameTimer);
      this.pendingRenameTimer = null;
    }
    const nodeEl = e.target.closest(".qf-node");
    if (!nodeEl) return;
    const path = nodeEl.dataset.path;
    const meta = this.nodes.get(path);
    if (!meta) return;
    if (meta.entry.isDir) {
      this.toggleNode(path);
    } else {
      QF.invoke("open_default", { path }).catch((err) => QF.showError(err));
    }
  };

  FileTree.prototype.selectNode = function (path) {
    if (this.selectedNode && this.selectedNode.nodeEl) {
      this.selectedNode.nodeEl.classList.remove("selected");
    }
    const meta = this.nodes.get(path);
    if (!meta) { this.selectedNode = null; return; }
    meta.nodeEl.classList.add("selected");
    this.selectedNode = { path, isDir: meta.entry.isDir, nodeEl: meta.nodeEl, entry: meta.entry };
    if (this.options.onSelect) this.options.onSelect(this, meta.entry);
  };

  FileTree.prototype.toggleNode = async function (path) {
    const meta = this.nodes.get(path);
    if (!meta || !meta.entry.isDir) return;
    if (meta.expanded) {
      meta.expanded = false;
      meta.childrenEl.classList.add("d-none");
      meta.nodeEl.querySelector(".qf-toggle").textContent = "▶";
      updateNodeIcon(meta, false);
      return;
    }
    // Always reload on expand.
    meta.childrenEl.innerHTML = '<div class="qf-node" style="padding-left:' + (6 + (meta.depth + 1) * 16) + 'px"><span class="qf-spinner"></span> 加载中...</div>';
    meta.childrenEl.classList.remove("d-none");
    meta.nodeEl.querySelector(".qf-toggle").textContent = "▼";
    updateNodeIcon(meta, true);
    try {
      const items = await QF.invoke("read_dir", { path, showHidden: this.showHidden });
      meta.childrenEl.innerHTML = "";
      // Remove children entries from this.nodes that may be stale (descendants of this path).
      this.purgeDescendants(path);
      for (const it of items) {
        const entry = { name: it.name, path: it.path, isDir: it.isDir, isHidden: it.isHidden, size: it.size };
        const el = this.createNodeEl(entry, meta.depth + 1);
        meta.childrenEl.appendChild(el);
      }
      meta.expanded = true;
      meta.childrenLoaded = true;
    } catch (err) {
      meta.childrenEl.innerHTML = "";
      meta.childrenEl.classList.add("d-none");
      meta.nodeEl.querySelector(".qf-toggle").textContent = "▶";
      meta.expanded = false;
      updateNodeIcon(meta, false);
      QF.showError(err);
    }
  };

  FileTree.prototype.purgeDescendants = function (parentPath) {
    // After clearing the parent's childrenEl, the wrappers of stale descendants
    // are detached from the document. Use that to filter precisely instead of
    // matching by path prefix (which would wrongly drop sibling roots like the
    // user's home directory when expanding the drive that contains it).
    for (const [k, meta] of Array.from(this.nodes.entries())) {
      if (k === parentPath) continue;
      if (meta.el && !document.contains(meta.el)) {
        this.nodes.delete(k);
      }
    }
  };

  FileTree.prototype.refreshOpen = async function () {
    // Re-expand all currently-expanded directories with the new showHidden flag.
    const openPaths = [];
    for (const [path, meta] of this.nodes.entries()) {
      if (meta.expanded) openPaths.push(path);
    }
    // Collapse all then re-expand from shallowest to deepest.
    openPaths.sort((a, b) => a.length - b.length);
    for (const p of openPaths) {
      const meta = this.nodes.get(p);
      if (meta) { meta.expanded = false; meta.childrenEl.classList.add("d-none"); meta.nodeEl.querySelector(".qf-toggle").textContent = "▶"; }
    }
    for (const p of openPaths) {
      if (this.nodes.has(p)) await this.toggleNode(p);
    }
  };

  FileTree.prototype.beginRename = function (path) {
    const meta = this.nodes.get(path);
    if (!meta) return;
    const labelEl = meta.nodeEl.querySelector(".qf-label");
    const oldName = meta.entry.name;
    const input = document.createElement("input");
    input.type = "text";
    input.className = "qf-rename-input";
    input.value = oldName;
    labelEl.replaceWith(input);
    input.focus();
    // Select base name (without extension) for files.
    if (!meta.entry.isDir) {
      const dot = oldName.lastIndexOf(".");
      if (dot > 0) input.setSelectionRange(0, dot);
      else input.select();
    } else {
      input.select();
    }
    let done = false;
    const finish = (commit) => {
      if (done) return; done = true;
      const newName = input.value.trim();
      const restoreLabel = document.createElement("span");
      restoreLabel.className = "qf-label";
      restoreLabel.textContent = oldName;
      input.replaceWith(restoreLabel);
      if (commit && newName && newName !== oldName) {
        QF.invoke("op_rename", { path, newName })
          .catch((err) => { if (err && err.code === "BUSY_TASK") QF.showBusy(); else QF.showError(err); });
      }
    };
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") { e.preventDefault(); finish(true); }
      else if (e.key === "Escape") { e.preventDefault(); finish(false); }
    });
    input.addEventListener("blur", () => finish(true));
  };

  // Drag and drop -----------------------------------------------------------
  FileTree.prototype.handleDragStart = function (e, entry) {
    if (entry.isRoot) { e.preventDefault(); return; }
    const payload = { path: entry.path, isDir: entry.isDir, name: entry.name };
    e.dataTransfer.setData("application/x-qfinder-node", JSON.stringify(payload));
    e.dataTransfer.effectAllowed = "move";
    // Custom drag image
    const ghost = document.createElement("div");
    ghost.className = "qf-drag-ghost";
    ghost.innerHTML = renderIcon(entry, entry.isDir) + '<span class="qf-drag-ghost-label"></span>';
    ghost.querySelector(".qf-drag-ghost-label").textContent = entry.name;
    document.body.appendChild(ghost);
    if (e.dataTransfer.setDragImage) e.dataTransfer.setDragImage(ghost, 10, 10);
    setTimeout(() => ghost.remove(), 0);
  };

  FileTree.prototype.handleDragOver = function (e) {
    const nodeEl = e.target.closest(".qf-node");
    if (!nodeEl || nodeEl.dataset.isDir !== "1") return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    if (this._lastOver && this._lastOver !== nodeEl) this._lastOver.classList.remove("drop-target");
    nodeEl.classList.add("drop-target");
    this._lastOver = nodeEl;
  };

  FileTree.prototype.handleDragLeave = function (e) {
    if (this._lastOver && !this.scroll.contains(e.relatedTarget)) {
      this._lastOver.classList.remove("drop-target");
      this._lastOver = null;
    }
  };

  FileTree.prototype.handleDrop = function (e) {
    if (this._lastOver) { this._lastOver.classList.remove("drop-target"); this._lastOver = null; }
    const data = e.dataTransfer.getData("application/x-qfinder-node");
    if (!data) return;
    const nodeEl = e.target.closest(".qf-node");
    if (!nodeEl || nodeEl.dataset.isDir !== "1") return;
    e.preventDefault();
    const src = JSON.parse(data);
    const dstDir = nodeEl.dataset.path;
    if (src.path === dstDir) return;
    QF.runFsOp("op_move", { src: src.path, dstDir });
  };

  global.FileTree = FileTree;

  // Helpers ----------------------------------------------------------------
  function renderIcon(entry, expanded) {
    if (!global.QF || !QF.Icons) {
      return entry.isDir
        ? '<i class="bi bi-folder-fill" style="color:#f0c674"></i>'
        : '<i class="bi bi-file-earmark"></i>';
    }
    const iconName = entry.isDir
      ? QF.Icons.resolveFolder(entry.name, !!expanded, !!entry.isRoot)
      : QF.Icons.resolveFile(entry.name);
    if (!iconName || iconName === QF.Icons.FALLBACK) {
      return entry.isDir
        ? '<i class="bi bi-folder-fill" style="color:#f0c674"></i>'
        : '<i class="bi bi-file-earmark"></i>';
    }
    return QF.Icons.iconImg(iconName);
  }

  function updateNodeIcon(meta, expanded) {
    if (!meta || !meta.nodeEl) return;
    const iconEl = meta.nodeEl.querySelector(".qf-icon");
    if (!iconEl) return;
    iconEl.innerHTML = renderIcon(meta.entry, expanded);
  }
})(window);

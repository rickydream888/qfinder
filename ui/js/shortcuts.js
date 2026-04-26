// Keyboard shortcuts: Ctrl/Cmd + C/X/V, Delete.
(function (global) {
  let osFamily = "windows";
  // Internal clipboard: { mode: 'copy'|'cut', path }
  let clipboard = null;

  async function init() {
    try { osFamily = await QF.invoke("os_family"); } catch (e) { /* keep default */ }
    document.addEventListener("keydown", onKeyDown);
  }

  function isModifier(e) {
    return osFamily === "macos" ? e.metaKey : e.ctrlKey;
  }

  function getActiveTreeAndSelection() {
    // Prefer tree that contains document.activeElement; else the last focused tree.
    const trees = FileTree.all();
    let tree = null;
    if (document.activeElement) {
      for (const t of trees) {
        if (t.scroll && t.scroll.contains(document.activeElement)) { tree = t; break; }
      }
    }
    if (!tree) {
      for (const t of trees) { if (t.selectedNode) { tree = t; break; } }
    }
    if (!tree) return { tree: null, selection: null };
    return { tree, selection: tree.selectedNode };
  }

  function targetDirOf(selection) {
    if (!selection) return null;
    if (selection.isDir) return selection.path;
    // Use parent dir.
    const p = selection.path;
    const idx = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
    return idx > 0 ? p.slice(0, idx) : null;
  }

  async function onKeyDown(e) {
    // Ignore shortcuts while user is typing in an input/textarea/contenteditable
    // (e.g. inline rename input, or focused dialog controls).
    const t = e.target;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable)) {
      return;
    }
    const { tree, selection } = getActiveTreeAndSelection();
    if (!tree) return;
    if (e.key === "Delete" || (osFamily === "macos" && e.key === "Backspace" && e.metaKey)) {
      if (!selection) return;
      e.preventDefault();
      try { await QF.invoke("op_delete", { path: selection.path }); }
      catch (err) { if (err && err.code === "BUSY_TASK") QF.showBusy(); else QF.showError(err); }
      return;
    }
    if (!isModifier(e)) return;
    const k = e.key.toLowerCase();
    if (k === "c") {
      if (!selection) return;
      e.preventDefault();
      clipboard = { mode: "copy", path: selection.path };
    } else if (k === "x") {
      if (!selection) return;
      e.preventDefault();
      clipboard = { mode: "cut", path: selection.path };
    } else if (k === "v") {
      if (!clipboard) return;
      const dstDir = targetDirOf(selection);
      if (!dstDir) { QF.showInfo("请先在文件树中选择一个目标目录或目标目录中的文件。"); return; }
      e.preventDefault();
      const cmd = clipboard.mode === "copy" ? "op_copy" : "op_move";
      const result = await QF.runFsOp(cmd, { src: clipboard.path, dstDir });
      if (result && clipboard.mode === "cut") clipboard = null;
    }
  }

  global.Shortcuts = { init };
})(window);

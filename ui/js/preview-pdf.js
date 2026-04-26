// PDF first-page renderer using PDF.js (lazy-loaded).
(function (global) {
  let pdfjsPromise = null;
  let currentTask = null;     // active RenderTask
  let currentDoc = null;      // active pdfDocument

  function loadPdfjs() {
    if (pdfjsPromise) return pdfjsPromise;
    pdfjsPromise = (async () => {
      const mod = await import("../vendor/pdfjs/pdf.min.mjs");
      const lib = mod.default || mod;
      // Resolve worker URL relative to the page (works under tauri:// scheme too).
      lib.GlobalWorkerOptions.workerSrc = new URL(
        "../vendor/pdfjs/pdf.worker.min.mjs",
        document.baseURI
      ).toString();
      return lib;
    })();
    return pdfjsPromise;
  }

  async function render(container, srcUrl) {
    cancelCurrent();
    container.innerHTML =
      '<div class="qf-loading">' +
      '  <div class="qf-spinner-lg"></div>' +
      '  <div class="qf-loading-text">加载 PDF 渲染器…</div>' +
      '</div>';
    let pdfjs;
    try {
      pdfjs = await loadPdfjs();
    } catch (e) {
      container.innerHTML =
        '<div class="qf-preview-warn">无法加载 PDF 渲染组件：' +
        QF.escapeHtml(String(e && e.message || e)) + '</div>';
      return;
    }
    const localToken = Symbol("pdf-render");
    container._pdfRenderToken = localToken;

    let doc;
    try {
      const task = pdfjs.getDocument({ url: srcUrl });
      doc = await task.promise;
    } catch (e) {
      if (container._pdfRenderToken !== localToken) return;
      container.innerHTML =
        '<div class="qf-preview-warn">无法打开 PDF：' +
        QF.escapeHtml(String(e && e.message || e)) + '</div>';
      return;
    }
    if (container._pdfRenderToken !== localToken) {
      doc.destroy();
      return;
    }
    currentDoc = doc;

    let page;
    try {
      page = await doc.getPage(1);
    } catch (e) {
      container.innerHTML =
        '<div class="qf-preview-warn">读取首页失败：' +
        QF.escapeHtml(String(e && e.message || e)) + '</div>';
      doc.destroy();
      currentDoc = null;
      return;
    }
    if (container._pdfRenderToken !== localToken) {
      doc.destroy();
      return;
    }

    container.innerHTML = '';
    const canvas = document.createElement('canvas');
    canvas.className = 'qf-pdf-canvas';
    container.appendChild(canvas);

    const baseViewport = page.getViewport({ scale: 1 });
    const containerWidth = Math.max(200, container.clientWidth - 4);
    const scale = containerWidth / baseViewport.width;
    const viewport = page.getViewport({ scale });
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.floor(viewport.width * dpr);
    canvas.height = Math.floor(viewport.height * dpr);
    canvas.style.width = Math.floor(viewport.width) + 'px';
    canvas.style.height = Math.floor(viewport.height) + 'px';
    const ctx = canvas.getContext('2d');
    ctx.scale(dpr, dpr);

    const total = doc.numPages;
    if (total > 1) {
      const note = document.createElement('div');
      note.className = 'qf-sheet-summary';
      note.textContent = `共 ${total} 页，仅预览第 1 页`;
      container.insertBefore(note, canvas);
    }

    try {
      currentTask = page.render({ canvasContext: ctx, viewport });
      await currentTask.promise;
    } catch (e) {
      if (e && e.name === 'RenderingCancelledException') return;
      container.innerHTML =
        '<div class="qf-preview-warn">渲染失败：' +
        QF.escapeHtml(String(e && e.message || e)) + '</div>';
    } finally {
      currentTask = null;
    }
  }

  function cancelCurrent() {
    if (currentTask && typeof currentTask.cancel === 'function') {
      try { currentTask.cancel(); } catch (_) { /* noop */ }
    }
    currentTask = null;
    if (currentDoc) {
      try { currentDoc.destroy(); } catch (_) { /* noop */ }
      currentDoc = null;
    }
  }

  global.QF = global.QF || {};
  global.QF.PreviewPDF = { render, cancelCurrent };
})(window);

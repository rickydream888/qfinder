// DOCX renderer using mammoth.js (lazy-loaded).
(function (global) {
  const HTML_BYTE_LIMIT = 100 * 1024;     // 100 KB
  const MAX_BLOCK_ELEMENTS = 200;
  let mammothPromise = null;

  function loadMammoth() {
    if (mammothPromise) return mammothPromise;
    mammothPromise = new Promise((resolve, reject) => {
      if (global.mammoth) return resolve(global.mammoth);
      const s = document.createElement('script');
      s.src = 'vendor/mammoth/mammoth.browser.min.js';
      s.onload = () => global.mammoth ? resolve(global.mammoth)
                                     : reject(new Error('mammoth.js loaded but global is missing'));
      s.onerror = () => reject(new Error('mammoth.js 加载失败'));
      document.head.appendChild(s);
    });
    return mammothPromise;
  }

  async function render(container, srcUrl) {
    container.innerHTML =
      '<div class="qf-loading">' +
      '  <div class="qf-spinner-lg"></div>' +
      '  <div class="qf-loading-text">解析 DOCX…</div>' +
      '</div>';
    const localToken = Symbol('docx-render');
    container._docxRenderToken = localToken;

    let mammoth;
    try {
      mammoth = await loadMammoth();
    } catch (e) {
      container.innerHTML =
        '<div class="qf-preview-warn">无法加载 DOCX 渲染组件：' +
        QF.escapeHtml(String(e && e.message || e)) + '</div>';
      return;
    }
    if (container._docxRenderToken !== localToken) return;

    let arrayBuffer;
    try {
      const resp = await fetch(srcUrl);
      if (!resp.ok) throw new Error('HTTP ' + resp.status);
      arrayBuffer = await resp.arrayBuffer();
    } catch (e) {
      container.innerHTML =
        '<div class="qf-preview-warn">读取 DOCX 文件失败：' +
        QF.escapeHtml(String(e && e.message || e)) + '</div>';
      return;
    }
    if (container._docxRenderToken !== localToken) return;

    let html, messages;
    try {
      const result = await mammoth.convertToHtml({ arrayBuffer });
      html = result.value || '';
      messages = result.messages || [];
    } catch (e) {
      container.innerHTML =
        '<div class="qf-preview-warn">无法解析 DOCX：' +
        QF.escapeHtml(String(e && e.message || e)) + '</div>';
      return;
    }
    if (container._docxRenderToken !== localToken) return;

    let truncated = false;
    if (html.length > HTML_BYTE_LIMIT) {
      html = html.slice(0, HTML_BYTE_LIMIT);
      truncated = true;
    }

    // Sanitize-ish: parse and keep only top-level body children, drop scripts/styles.
    const doc = new DOMParser().parseFromString(html, 'text/html');
    doc.querySelectorAll('script, style, link, meta').forEach((n) => n.remove());

    const body = doc.body;
    const children = Array.from(body.children);
    if (children.length > MAX_BLOCK_ELEMENTS) {
      truncated = true;
      for (let i = MAX_BLOCK_ELEMENTS; i < children.length; i++) {
        children[i].remove();
      }
    }

    const wrap = document.createElement('div');
    wrap.className = 'qf-docx';
    while (body.firstChild) wrap.appendChild(body.firstChild);

    container.innerHTML = '';
    if (truncated) {
      const note = document.createElement('div');
      note.className = 'qf-sheet-summary qf-sheet-trunc';
      note.textContent = '文档较长，已截断显示开头部分';
      container.appendChild(note);
    }
    container.appendChild(wrap);
    if (truncated) {
      const tail = document.createElement('p');
      tail.className = 'text-muted';
      tail.textContent = '…（已截断）';
      wrap.appendChild(tail);
    }
  }

  global.QF = global.QF || {};
  global.QF.PreviewDocx = { render };
})(window);

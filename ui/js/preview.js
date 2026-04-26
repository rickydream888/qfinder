// Preview pane renderer.
(function (global) {
  const PreviewPane = {
    el: null,
    _seq: 0,
    _debounceTimer: null,
    _spinnerTimer: null,
    init(el) { this.el = el; this.renderEmpty(); },
    renderEmpty() {
      this.el.innerHTML = '<div class="qf-empty">请在左侧选择一个目录或文件</div>';
    },
    load(path) {
      // Cancel any pending request and start a debounced one.
      if (this._debounceTimer) clearTimeout(this._debounceTimer);
      if (this._spinnerTimer) clearTimeout(this._spinnerTimer);
      // Cancel in-flight PDF render (if any) when switching files.
      if (global.QF && QF.PreviewPDF) QF.PreviewPDF.cancelCurrent();
      const seq = ++this._seq;
      if (!path) { this.renderEmpty(); return; }
      this._debounceTimer = setTimeout(() => this._loadNow(path, seq), 200);
    },
    async _loadNow(path, seq) {
      // Show spinner only if loading takes longer than 250ms (avoids flicker
      // for fast local previews; ensures visible feedback on slow network shares).
      this._spinnerTimer = setTimeout(() => {
        if (seq !== this._seq) return;
        this.el.innerHTML =
          '<div class="qf-loading">' +
          '  <div class="qf-spinner-lg"></div>' +
          '  <div class="qf-loading-text">加载预览中...</div>' +
          '  <div class="qf-loading-path">' + QF.escapeHtml(path) + '</div>' +
          '</div>';
      }, 250);
      try {
        const payload = await QF.invoke("preview", { path });
        if (seq !== this._seq) return; // stale result
        this.render(path, payload);
      } catch (err) {
        if (seq !== this._seq) return;
        QF.showError(err);
        this.renderEmpty();
      } finally {
        if (this._spinnerTimer) { clearTimeout(this._spinnerTimer); this._spinnerTimer = null; }
      }
    },
    render(path, p) {
      const name = pathBasename(path);
      const folderIcon = headerIcon(name, true, false);
      const fileIcon = headerIcon(name, false, false);
      let html = `<div class="mb-2 text-muted small">${QF.escapeHtml(path)}</div>`;
      switch (p.kind) {
        case "directory":
          html += `<h5 class="mb-3">${folderIcon} ${QF.escapeHtml(name)}</h5>`;
          html += `<table class="table table-sm w-auto"><tbody>`;
          html += `<tr><th>子目录数</th><td>${p.subDirs}</td></tr>`;
          html += `<tr><th>子文件数</th><td>${p.subFiles}</td></tr>`;
          html += `<tr><th>磁盘占用 (du)</th><td>${p.totalSize == null ? "—" : QF.formatBytes(p.totalSize)}</td></tr>`;
          html += `</tbody></table>`;
          break;
        case "text": {
          html += `<h5 class="mb-2">${fileIcon} ${QF.escapeHtml(name)}</h5>`;
          html += `<div class="text-muted small mb-2">大小：${QF.formatBytes(p.totalSize)}${p.truncated ? '，<span class="text-warning">已截断（仅展示前 10KB）</span>' : ""}</div>`;
          html += `<pre class="qf-text">${QF.escapeHtml(p.content)}</pre>`;
          break;
        }
        case "image": {
          const src = QF.convertFileSrc(p.path);
          html += `<h5 class="mb-2">${fileIcon} ${QF.escapeHtml(name)}</h5>`;
          html += `<div class="text-muted small mb-2">大小：${QF.formatBytes(p.size)}</div>`;
          html += `<img class="qf-image" src="${src}" alt="preview" />`;
          break;
        }
        case "imageTooLarge":
          html += `<h5 class="mb-2">${fileIcon} ${QF.escapeHtml(name)}</h5>`;
          html += `<div class="alert alert-warning">文件大小：${QF.formatBytes(p.size)}，超过预览限制（20MB）。</div>`;
          break;
        case "spreadsheet": {
          html += `<h5 class="mb-2">${fileIcon} ${QF.escapeHtml(name)}</h5>`;
          html += renderSpreadsheetSummary(p);
          html += renderSpreadsheetTable(p);
          break;
        }
        case "pdf": {
          html += `<h5 class="mb-2">${fileIcon} ${QF.escapeHtml(name)}</h5>`;
          html += `<div class="text-muted small mb-2">大小：${QF.formatBytes(p.size)}</div>`;
          html += `<div class="qf-pdf-host"></div>`;
          this.el.innerHTML = html;
          QF.PreviewPDF.render(this.el.querySelector('.qf-pdf-host'),
                               QF.convertFileSrc(p.path));
          return;
        }
        case "docx": {
          html += `<h5 class="mb-2">${fileIcon} ${QF.escapeHtml(name)}</h5>`;
          html += `<div class="text-muted small mb-2">大小：${QF.formatBytes(p.size)}</div>`;
          html += `<div class="qf-docx-host"></div>`;
          this.el.innerHTML = html;
          QF.PreviewDocx.render(this.el.querySelector('.qf-docx-host'),
                                QF.convertFileSrc(p.path));
          return;
        }
        case "pptx": {
          html += `<h5 class="mb-2">${fileIcon} ${QF.escapeHtml(name)}</h5>`;
          html += `<div class="text-muted small mb-2">大小：${QF.formatBytes(p.size)}（已转换为 PDF 进行预览）</div>`;
          html += `<div class="qf-pdf-host"></div>`;
          this.el.innerHTML = html;
          QF.PreviewPDF.render(this.el.querySelector('.qf-pdf-host'),
                               QF.convertFileSrc(p.pdfPath));
          return;
        }
        case "officeImage": {
          const src = QF.convertFileSrc(p.imagePath);
          html += `<h5 class="mb-2">${fileIcon} ${QF.escapeHtml(name)}</h5>`;
          html += `<div class="text-muted small mb-2">大小：${QF.formatBytes(p.size)} · 渲染引擎：${QF.escapeHtml(p.engine)}（仅首页缩略图）</div>`;
          html += `<img class="qf-image" src="${src}" alt="preview" />`;
          break;
        }
        case "unsupported":
          html += `<h5 class="mb-2">${fileIcon} ${QF.escapeHtml(name)}</h5>`;
          html += `<div class="text-muted small mb-2">大小：${QF.formatBytes(p.size)}</div>`;
          html += `<div class="qf-preview-warn">${QF.escapeHtml(p.reason)}</div>`;
          break;
        case "other":
          html += `<h5 class="mb-2">${fileIcon} ${QF.escapeHtml(name)}</h5>`;
          html += `<div>文件大小：${QF.formatBytes(p.size)}</div>`;
          break;
        default:
          html += `<div class="text-muted">不支持的预览类型</div>`;
      }
      this.el.innerHTML = html;
    },
  };

  function headerIcon(name, isDir, isRoot) {
    if (global.QF && QF.Icons) {
      const iconName = isDir
        ? QF.Icons.resolveFolder(name, true, isRoot)
        : QF.Icons.resolveFile(name);
      if (iconName && iconName !== QF.Icons.FALLBACK) {
        return QF.Icons.iconImg(iconName, { className: "qf-icon-img qf-icon-img-lg" });
      }
    }
    if (isDir) return '<i class="bi bi-folder2-open"></i>';
    return '<i class="bi bi-file-earmark"></i>';
  }

  function pathBasename(p) {
    if (!p) return "";
    const idx = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
    return idx >= 0 ? p.slice(idx + 1) : p;
  }

  function renderSpreadsheetSummary(p) {
    const parts = [
      `工作表：<strong>${QF.escapeHtml(p.sheetName)}</strong>`,
      `共 ${p.totalRows} 行 × ${p.totalCols} 列`,
    ];
    if (p.otherSheets && p.otherSheets.length) {
      parts.push(`其它工作表：${p.otherSheets.map(QF.escapeHtml).join('、')}`);
    }
    const truncs = [];
    if (p.truncatedRows) truncs.push(`仅显示前 100 行`);
    if (p.truncatedCols) truncs.push(`仅显示前 20 列`);
    let html = `<div class="qf-sheet-summary">${parts.join('　·　')}</div>`;
    if (truncs.length) {
      html += `<div class="qf-sheet-summary qf-sheet-trunc">${truncs.join('，')}</div>`;
    }
    return html;
  }

  function renderSpreadsheetTable(p) {
    let html = '<div class="qf-sheet-wrap"><table class="qf-sheet">';
    if (p.headers && p.headers.length) {
      html += '<thead><tr>';
      for (const h of p.headers) {
        const v = QF.escapeHtml(h);
        html += `<th title="${v}">${v}</th>`;
      }
      html += '</tr></thead>';
    }
    html += '<tbody>';
    for (const row of p.rows) {
      html += '<tr>';
      for (const cell of row) {
        const v = QF.escapeHtml(cell);
        html += `<td title="${v}">${v}</td>`;
      }
      html += '</tr>';
    }
    html += '</tbody></table></div>';
    return html;
  }

  global.PreviewPane = PreviewPane;
})(window);

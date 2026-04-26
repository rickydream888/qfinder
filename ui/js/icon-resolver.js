// IconResolver: maps a file/folder name to a Material Icon Theme icon name and URL.
//
// Loaded from ui/icons/material/manifest.json (built by scripts/build-material-icons.ps1).
// All lookups are case-insensitive. Light-mode tables take precedence over default tables.
(function (global) {
  // Extension -> VSCode languageId fallback table (only used when fileExtensions
  // map is missing or maps to an empty string, which means "look up by language").
  const EXT_TO_LANG = {
    js: "javascript", mjs: "javascript", cjs: "javascript",
    ts: "typescript", mts: "typescript", cts: "typescript",
    json: "json", jsonc: "json",
    html: "html", htm: "html",
    css: "css",
    xml: "xml", xsl: "xml", xslt: "xml",
    go: "go",
    java: "java", kt: "kotlin", kts: "kotlin",
    rb: "ruby", php: "php",
    sh: "shellscript", bash: "shellscript", zsh: "shellscript", fish: "shellscript",
    bat: "bat", cmd: "bat",
    ps1: "powershell", psm1: "powershell", psd1: "powershell",
    yml: "yaml", yaml: "yaml",
    toml: "toml", ini: "ini",
    sql: "sql",
    c: "c", h: "c",
    cpp: "cpp", cc: "cpp", cxx: "cpp", hpp: "cpp", hh: "cpp", hxx: "cpp",
    cs: "csharp",
    swift: "swift",
    md: "markdown", markdown: "markdown",
    txt: "plaintext"
  };

  const FALLBACK = "__bootstrap__";

  function lcMap(obj) {
    const m = new Map();
    if (!obj) return m;
    for (const k of Object.keys(obj)) {
      const v = obj[k];
      if (typeof v === "string" && v.length > 0) m.set(k.toLowerCase(), v);
    }
    return m;
  }

  const Icons = {
    _ready: false,
    _fallback: false,
    _defaults: null,
    _fileNames: null, _fileExt: null, _langIds: null,
    _folderNames: null, _folderNamesOpen: null,
    _lightFileNames: null, _lightFileExt: null, _lightLangIds: null,
    _lightFolderNames: null, _lightFolderNamesOpen: null,
    _lightRootNames: null, _lightRootNamesOpen: null,
    _resolveFileCache: new Map(),
    _resolveFolderCache: new Map(),
    _urlCache: new Map(),

    async init() {
      try {
        const res = await fetch("icons/material/manifest.json", { cache: "force-cache" });
        if (!res.ok) throw new Error("HTTP " + res.status);
        const m = await res.json();
        this._defaults = m.defaults || {};
        this._fileNames = lcMap(m.fileNames);
        this._fileExt = new Map(); // keep empty-string entries to trigger language fallback
        if (m.fileExtensions) {
          for (const k of Object.keys(m.fileExtensions)) {
            this._fileExt.set(k.toLowerCase(), m.fileExtensions[k] || "");
          }
        }
        this._langIds = lcMap(m.languageIds);
        this._folderNames = lcMap(m.folderNames);
        this._folderNamesOpen = lcMap(m.folderNamesExpanded);
        const light = m.light || {};
        this._lightFileNames = lcMap(light.fileNames);
        this._lightFileExt = new Map();
        if (light.fileExtensions) {
          for (const k of Object.keys(light.fileExtensions)) {
            this._lightFileExt.set(k.toLowerCase(), light.fileExtensions[k] || "");
          }
        }
        this._lightLangIds = lcMap(light.languageIds);
        this._lightFolderNames = lcMap(light.folderNames);
        this._lightFolderNamesOpen = lcMap(light.folderNamesExpanded);
        this._lightRootNames = lcMap(light.rootFolderNames);
        this._lightRootNamesOpen = lcMap(light.rootFolderNamesExpanded);
        this._ready = true;
      } catch (err) {
        this._fallback = true;
        // Surface once but do not block the app.
        if (global.console) console.error("[IconResolver] init failed, falling back to Bootstrap Icons:", err);
        if (global.QF && QF.showError) {
          try { QF.showError({ code: "ICONS_INIT_FAILED", message: "图标资源加载失败，已回退到 Bootstrap Icons" }); } catch (_) {}
        }
      }
    },

    isFallback() { return this._fallback; },

    resolveFile(name) {
      if (this._fallback) return FALLBACK;
      if (!this._ready) return this._defaults ? this._defaults.file : FALLBACK;
      const key = (name || "").toLowerCase();
      const cached = this._resolveFileCache.get(key);
      if (cached !== undefined) return cached;
      const icon = this._resolveFileImpl(key);
      this._resolveFileCache.set(key, icon);
      return icon;
    },

    _resolveFileImpl(lcName) {
      // 1) full-name match (light first)
      let v = this._lightFileNames.get(lcName);
      if (v) return v;
      v = this._fileNames.get(lcName);
      if (v) return v;
      // 2) extension chain, longest first
      // Split on '.', strip leading empty (dotfiles like ".gitignore" already matched in step 1).
      const parts = lcName.split(".");
      // Try chains: "tar.gz", "gz" for "a.tar.gz" → start from i=1 to skip the basename.
      for (let i = 1; i < parts.length; i++) {
        const ext = parts.slice(i).join(".");
        // a) light extensions
        let extVal = this._lightFileExt.get(ext);
        if (extVal !== undefined) {
          if (extVal) return extVal;
          // empty -> language fallback
          const lang = EXT_TO_LANG[ext];
          if (lang) {
            const lv = this._lightLangIds.get(lang) || this._langIds.get(lang);
            if (lv) return lv;
          }
        }
        // b) default extensions
        extVal = this._fileExt.get(ext);
        if (extVal !== undefined) {
          if (extVal) return extVal;
          const lang = EXT_TO_LANG[ext];
          if (lang) {
            const lv = this._lightLangIds.get(lang) || this._langIds.get(lang);
            if (lv) return lv;
          }
        }
      }
      return this._defaults.file;
    },

    resolveFolder(name, expanded, isRoot) {
      if (this._fallback) return FALLBACK;
      if (!this._ready) {
        if (!this._defaults) return FALLBACK;
        return expanded ? this._defaults.folderExpanded : this._defaults.folder;
      }
      const key = (name || "").toLowerCase() + "|" + (expanded ? 1 : 0) + "|" + (isRoot ? 1 : 0);
      const cached = this._resolveFolderCache.get(key);
      if (cached !== undefined) return cached;
      const icon = this._resolveFolderImpl((name || "").toLowerCase(), !!expanded, !!isRoot);
      this._resolveFolderCache.set(key, icon);
      return icon;
    },

    _resolveFolderImpl(lcName, expanded, isRoot) {
      if (isRoot) {
        if (expanded) {
          return this._lightRootNamesOpen.get(lcName)
              || this._lightRootNames.get(lcName)
              || this._defaults.rootFolderExpanded
              || this._defaults.folderExpanded;
        }
        return this._lightRootNames.get(lcName)
            || this._defaults.rootFolder
            || this._defaults.folder;
      }
      if (expanded) {
        return this._lightFolderNamesOpen.get(lcName)
            || this._folderNamesOpen.get(lcName)
            || this._defaults.folderExpanded;
      }
      return this._lightFolderNames.get(lcName)
          || this._folderNames.get(lcName)
          || this._defaults.folder;
    },

    iconUrl(iconName) {
      if (!iconName || iconName === FALLBACK) return "";
      const cached = this._urlCache.get(iconName);
      if (cached) return cached;
      const url = "icons/material/svg/" + encodeURIComponent(iconName) + ".svg";
      this._urlCache.set(iconName, url);
      return url;
    },

    iconImg(iconName, opts) {
      const cls = (opts && opts.className) || "qf-icon-img";
      if (!iconName || iconName === FALLBACK) return "";
      const url = this.iconUrl(iconName);
      return '<img class="' + cls + '" src="' + url + '" alt="" draggable="false" loading="lazy">';
    },

    FALLBACK
  };

  global.QF = global.QF || {};
  global.QF.Icons = Icons;
})(window);

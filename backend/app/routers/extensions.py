import mimetypes
from pathlib import Path

from fastapi import APIRouter, HTTPException
from fastapi.responses import FileResponse, HTMLResponse, Response

from .. import __version__
from ..engine import rpc as engine_rpc

router = APIRouter(tags=["extensions"])

SHIM = """(function () {
  if (window.__adobe_cep__) return;
  var seq = 0, waits = {}, listeners = {};
  window.addEventListener('message', function (ev) {
    var m = ev.data;
    if (!m || ev.source !== window.parent) return;
    if (m.fvx === 'csi-result') {
      var cb = waits[m.id];
      delete waits[m.id];
      if (cb) cb(m.result);
    } else if (m.fvx === 'csi-event') {
      var list = listeners[m.event && m.event.type] || [];
      for (var i = 0; i < list.length; i++) list[i](m.event);
    }
  });
  function post(msg) { window.parent.postMessage(msg, '*'); }
  var env = {
    appName: 'FVFX', appVersion: '__VERSION__', appLocale: 'en_US', appUILocale: 'en_US', appId: 'FVFX', isAppOnline: true,
    appSkinInfo: {baseFontFamily: 'system-ui', baseFontSize: 12, appBarBackgroundColor: {color: {red: 30, green: 30, blue: 30, alpha: 255}}, panelBackgroundColor: {color: {red: 30, green: 30, blue: 30, alpha: 255}}, appBarBackgroundColorSRGB: {color: {red: 30, green: 30, blue: 30, alpha: 255}}, panelBackgroundColorSRGB: {color: {red: 30, green: 30, blue: 30, alpha: 255}}, systemHighlightColor: {red: 249, green: 115, blue: 22, alpha: 255}}
  };
  window.__adobe_cep__ = {
    evalScript: function (script, cb) { var id = ++seq; if (cb) waits[id] = cb; post({fvx: 'csi-eval', id: id, script: String(script)}); },
    getHostEnvironment: function () { return JSON.stringify(env); },
    getSystemPath: function () { return ''; },
    getExtensions: function () { return '[]'; },
    getCurrentApiVersion: function () { return JSON.stringify({major: 11, minor: 0, micro: 0}); },
    getNetworkPreferences: function () { return '{}'; },
    addEventListener: function (type, fn) { (listeners[type] = listeners[type] || []).push(fn); },
    removeEventListener: function (type, fn) { listeners[type] = (listeners[type] || []).filter(function (f) { return f !== fn; }); },
    dispatchEvent: function (e) { post({fvx: 'csi-dispatch', event: typeof e === 'string' ? JSON.parse(e) : e}); },
    closeExtension: function () { post({fvx: 'csi-close'}); },
    requestOpenExtension: function (id) { post({fvx: 'csi-open', id: String(id)}); },
    openURLInDefaultBrowser: function (url) { post({fvx: 'csi-url', url: String(url)}); },
    getScaleFactor: function () { return window.devicePixelRatio || 1; },
    setScaleFactorChangedHandler: function () {},
    invokeSync: function () { return ''; },
    invokeAsync: function () {},
    registerKeyEventsInterest: function () {},
    setWindowTitle: function (t) { post({fvx: 'csi-title', title: String(t)}); },
    getWindowTitle: function () { return document.title; }
  };
  if (!window.CSInterface) {
    var C = function () { this.hostEnvironment = env; };
    C.prototype.evalScript = function (s, cb) { window.__adobe_cep__.evalScript(s, cb); };
    C.prototype.getHostEnvironment = function () { return env; };
    C.prototype.addEventListener = function (t, f) { window.__adobe_cep__.addEventListener(t, f); };
    C.prototype.removeEventListener = function (t, f) { window.__adobe_cep__.removeEventListener(t, f); };
    C.prototype.dispatchEvent = function (e) { window.__adobe_cep__.dispatchEvent(e); };
    C.prototype.closeExtension = function () { window.__adobe_cep__.closeExtension(); };
    C.prototype.requestOpenExtension = function (id) { window.__adobe_cep__.requestOpenExtension(id); };
    C.prototype.openURLInDefaultBrowser = function (u) { window.__adobe_cep__.openURLInDefaultBrowser(u); };
    C.prototype.getApplicationID = function () { return 'FVFX'; };
    C.prototype.getSystemPath = function () { return ''; };
    C.prototype.getScaleFactor = function () { return window.devicePixelRatio || 1; };
    window.CSInterface = C;
    window.CSEvent = function (type, scope, appId, extensionId) { this.type = type; this.scope = scope; this.appId = appId; this.extensionId = extensionId; this.data = ''; };
    window.SystemPath = {USER_DATA: 'userData', COMMON_FILES: 'commonFiles', MY_DOCUMENTS: 'myDocuments', APPLICATION: 'application', EXTENSION: 'extension', HOST_APPLICATION: 'hostApplication'};
  }
})();
"""


@router.get("/api/extensions/_csinterface.js")
def shim() -> Response:
    return Response(SHIM.replace("__VERSION__", __version__), media_type="application/javascript")


@router.get("/api/extensions/{folder}/{path:path}")
async def extension_file(folder: str, path: str) -> Response:
    data = await engine_rpc.execute("extensions.list", {}, "ui")
    if not any(x.get("folder") == folder for x in (data or {}).get("extensions", [])):
        raise HTTPException(404, "no such extension")
    root = (Path(data["folder"]) / folder).resolve()
    target = (root / path).resolve()
    if root not in target.parents and target != root:
        raise HTTPException(403, "outside the extension")
    if not target.is_file():
        raise HTTPException(404, "not found")
    if target.suffix.lower() in (".html", ".htm"):
        text = target.read_text(encoding="utf-8", errors="replace")
        tag = '<script src="/api/extensions/_csinterface.js"></script>'
        low = text.lower()
        at = low.find("<head>")
        text = text[: at + 6] + tag + text[at + 6 :] if at >= 0 else tag + text
        return HTMLResponse(text, headers={"Cache-Control": "no-store"})
    kind = mimetypes.guess_type(target.name)[0] or "application/octet-stream"
    return FileResponse(target, media_type=kind)

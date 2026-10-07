// The script the surface runs in every frame (research.md §8, §10). Rust
// writes the surface's secret in where SECRET is; the script is otherwise the
// same for every surface. It is a belt over the braces Rust holds in native
// code: nothing here is what stops a document from reaching anything.
(function () {
  'use strict';
  var SECRET = '__HOPLODEX_SURFACE_SECRET__';

  function cancel(event) {
    event.preventDefault();
    event.stopPropagation();
  }

  // No context menu (macOS: Open in Preview is in it), and no Save, Print or
  // Open keys, in any frame the script reaches.
  addEventListener('contextmenu', cancel, true);
  addEventListener(
    'keydown',
    function (event) {
      if ((event.ctrlKey || event.metaKey) && !event.altKey) {
        var key = String(event.key).toLowerCase();
        if (key === 's' || key === 'p' || key === 'o') {
          cancel(event);
        }
      }
    },
    true
  );

  // Only PDF.js's viewer frame (WebKitGTK, Linux) has anything more to set.
  if (location.protocol !== 'webkit-pdfjs-viewer:') {
    return;
  }

  // PDF.js's own print goes through window.print; assigning to it is ignored
  // rather than refused, so its start-up code doesn't fail on it.
  var noop = function () {};
  try {
    Object.defineProperty(window, 'print', {
      configurable: false,
      get: function () {
        return noop;
      },
      set: function () {},
    });
  } catch (e) {
    window.print = noop;
  }

  // PDF.js says it is about to start with `webviewerloaded`, on its parent's
  // document when that is reachable and on its own otherwise: the one place
  // an embedder can change its options before it reads them.
  function onLoaded(event) {
    var options =
      (event && event.detail && event.detail.source && event.detail.source.PDFViewerApplicationOptions) ||
      window.PDFViewerApplicationOptions;
    if (!options) {
      // Without the options the PDF's scripting can't be turned off, so the
      // hook is not reported, and Rust closes the surface.
      return;
    }
    options.set('enableScripting', false);
    options.set('enableXfa', false);
    options.set('annotationEditorMode', -1);
    // Tells the protocol handler this document's viewer is set up.
    new Image().src = 'hdpreview://localhost/hooked/' + SECRET;
  }
  document.addEventListener('webviewerloaded', onLoaded);
  try {
    parent.document.addEventListener('webviewerloaded', onLoaded);
  } catch (e) {
    // The parent is another origin.
  }

  // No Save, Print, Open or editing controls in the toolbar (the shortcuts
  // are cancelled above, and Rust refuses the download and answers the print
  // signal).
  var style = document.createElement('style');
  style.textContent =
    '#download, #secondaryDownload, #print, #secondaryPrint, #openFile, #secondaryOpenFile,' +
    ' #editorModeButtons, #editorModeSeparator, #viewBookmark, #secondaryViewBookmark' +
    ' { display: none !important; }';
  (document.head || document.documentElement).appendChild(style);
})();

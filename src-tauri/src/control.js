// Injected into the my.brain.fm webview at document start.
// Two jobs:
//   1) Expose window.__brainfm(action) so the Rust/MPRIS side can drive playback.
//   2) Poll player state + metadata and push it back to Rust via Tauri IPC so the
//      MPRIS/desktop media widget shows the right title and play/pause icon.
//
// Brain.fm's logged-in player DOM isn't public, so control uses a layered fallback
// strategy. If your DE's media keys don't toggle playback, tweak SELECTORS below.
(function () {
  "use strict";
  if (window.__brainfmInstalled) return;
  window.__brainfmInstalled = true;

  // Buttons we try to click, most-specific first. Adjust if Brain.fm changes its UI.
  var SELECTORS = {
    toggle: [
      'button[aria-label*="play" i]',
      'button[aria-label*="pause" i]',
      'button[data-testid*="play" i]',
      '[role="button"][aria-label*="play" i]',
    ],
    next: [
      'button[aria-label*="next" i]',
      'button[aria-label*="skip" i]',
      'button[data-testid*="next" i]',
    ],
    prev: [
      'button[aria-label*="previous" i]',
      'button[aria-label*="back" i]',
      'button[data-testid*="prev" i]',
    ],
  };

  function mediaEl() {
    var els = document.querySelectorAll("audio, video");
    for (var i = 0; i < els.length; i++) {
      // pick the one with a real source
      if (els[i].src || els[i].currentSrc || els[i].srcObject) return els[i];
    }
    return els[0] || null;
  }

  function clickFirst(list) {
    for (var i = 0; i < list.length; i++) {
      var el = document.querySelector(list[i]);
      if (el) {
        el.click();
        return true;
      }
    }
    return false;
  }

  function sendSpace() {
    var t = document.activeElement || document.body;
    ["keydown", "keyup"].forEach(function (type) {
      t.dispatchEvent(
        new KeyboardEvent(type, {
          key: " ",
          code: "Space",
          keyCode: 32,
          which: 32,
          bubbles: true,
        })
      );
    });
  }

  function isPlaying() {
    if (
      navigator.mediaSession &&
      navigator.mediaSession.playbackState &&
      navigator.mediaSession.playbackState !== "none"
    ) {
      return navigator.mediaSession.playbackState === "playing";
    }
    var el = mediaEl();
    if (el) return !el.paused && !el.ended && el.readyState > 2;
    return false;
  }

  function play() {
    var el = mediaEl();
    if (el && el.paused) {
      el.play().catch(function () {});
      return;
    }
    if (!isPlaying() && !clickFirst(SELECTORS.toggle)) sendSpace();
  }

  function pause() {
    var el = mediaEl();
    if (el && !el.paused) {
      el.pause();
      return;
    }
    if (isPlaying() && !clickFirst(SELECTORS.toggle)) sendSpace();
  }

  function toggle() {
    isPlaying() ? pause() : play();
  }

  window.__brainfm = function (action) {
    try {
      switch (action) {
        case "play": return play();
        case "pause": return pause();
        case "toggle": return toggle();
        case "next": return clickFirst(SELECTORS.next);
        case "prev": return clickFirst(SELECTORS.prev);
        case "stop": return pause();
      }
    } catch (e) {
      /* swallow — never break the page */
    }
  };

  // ---- report state back to Rust ----
  function invoke(cmd, args) {
    try {
      if (window.__TAURI__ && window.__TAURI__.core) {
        window.__TAURI__.core.invoke(cmd, args);
      } else if (window.__TAURI_INTERNALS__) {
        window.__TAURI_INTERNALS__.invoke(cmd, args);
      }
    } catch (e) {}
  }

  var last = "";
  function poll() {
    var md =
      (navigator.mediaSession && navigator.mediaSession.metadata) || null;
    var title = (md && md.title) || document.title || "Brain.fm";
    var artist = (md && md.artist) || "Brain.fm";
    var album = (md && md.album) || "";
    var art = "";
    if (md && md.artwork && md.artwork.length) {
      art = md.artwork[md.artwork.length - 1].src || "";
    }
    var playing = isPlaying();
    var sig = playing + "|" + title + "|" + artist;
    if (sig !== last) {
      last = sig;
      invoke("update_now_playing", {
        playing: playing,
        title: title,
        artist: artist,
        album: album,
        artUrl: art,
      });
    }
  }

  setInterval(poll, 1000);
  document.addEventListener("DOMContentLoaded", poll);
  setTimeout(poll, 1500);
})();

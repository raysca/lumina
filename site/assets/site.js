// Lumina product page: reveal-on-scroll, a hero that settles as you scroll,
// counters, the culling key demo and platform detection. Runs locally; the
// page makes no network requests beyond its own files.
(function () {
  "use strict";
  var reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  // Reveal anything marked .reveal (and the animated widgets) when it enters.
  var watched = document.querySelectorAll(".reveal, .map, .searchbox, .rules, .cluster");
  if ("IntersectionObserver" in window && !reduce) {
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (e.isIntersecting) { e.target.classList.add("in"); io.unobserve(e.target); }
      });
    }, { rootMargin: "0px 0px -12% 0px", threshold: 0.12 });
    watched.forEach(function (el) { io.observe(el); });
  } else {
    watched.forEach(function (el) { el.classList.add("in"); });
  }

  // Solid nav after the hero starts to scroll; hero window grows to full size.
  var nav = document.querySelector(".nav");
  var shot = document.querySelector(".hero-shot");
  var glow = document.querySelector(".hero-glow");
  var ticking = false;
  function onScroll() {
    var y = window.scrollY;
    nav.classList.toggle("solid", y > 24);
    if (!reduce && shot) {
      var t = Math.min(1, y / 520);
      shot.style.setProperty("--s", (0.92 + 0.08 * t).toFixed(4));
      if (glow) glow.style.setProperty("--glow", (1 - 0.6 * t).toFixed(3));
    }
    ticking = false;
  }
  window.addEventListener("scroll", function () {
    if (!ticking) { ticking = true; requestAnimationFrame(onScroll); }
  }, { passive: true });
  onScroll();

  // Count-up numbers in the stats strip.
  document.querySelectorAll("[data-count]").forEach(function (el) {
    var end = parseFloat(el.getAttribute("data-count"));
    var suffix = el.getAttribute("data-suffix") || "";
    if (reduce || !("IntersectionObserver" in window)) { el.textContent = end + suffix; return; }
    var obs = new IntersectionObserver(function (entries) {
      if (!entries[0].isIntersecting) return;
      obs.disconnect();
      var start = performance.now();
      (function step(now) {
        var p = Math.min(1, (now - start) / 1200);
        var eased = 1 - Math.pow(1 - p, 3);
        el.textContent = Math.round(end * eased) + suffix;
        if (p < 1) requestAnimationFrame(step);
      })(start);
    });
    obs.observe(el);
  });

  // Culling demo: light the keys in turn.
  var keys = document.querySelectorAll(".keys kbd");
  if (keys.length && !reduce) {
    var i = 0;
    setInterval(function () {
      keys.forEach(function (k) { k.classList.remove("lit"); });
      keys[i % keys.length].classList.add("lit");
      i++;
    }, 1100);
  }

  // Highlight the visitor's platform.
  var ua = navigator.userAgent;
  var os = /Mac/.test(ua) ? "mac" : (/Linux|X11/.test(ua) && !/Android/.test(ua)) ? "linux" : /Windows/.test(ua) ? "windows" : null;
  if (os) {
    var card = document.querySelector('.platform[data-os="' + os + '"]');
    if (card) card.classList.add("mine");
    var hero = document.getElementById("hero-download");
    if (hero && os === "mac") hero.textContent = "Download for macOS";
    if (hero && os === "linux") hero.textContent = "Download for Linux";
  }
})();

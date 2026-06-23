"use strict";
  const ACTIVE_RELOAD_BANNER_COOLDOWN_MS = 30000;
  const ACTIVE_RELOAD_BANNER_COUNTDOWN_SECONDS = 3;
  let activeReloadBannerWasInactive = false;
  let activeReloadBannerEventsBound = false;
  let activeReloadBannerLastShownAt = 0;
  let activeReloadBannerTimer = null;
  let activeReloadBannerRemainingSeconds = 0;

  function bindActiveReloadBannerEvents() {
    if (activeReloadBannerEventsBound) return;
    activeReloadBannerEventsBound = true;
    activeReloadBannerWasInactive = isOverviewTabInactive();

    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "hidden") {
        markActiveReloadBannerInactive("visibilitychange");
      } else {
        handleActiveReloadBannerActivation("visibilitychange");
      }
    });

    window.addEventListener("blur", () => markActiveReloadBannerInactive("blur"));
    window.addEventListener("pagehide", () => markActiveReloadBannerInactive("pagehide"));
    window.addEventListener("focus", () => handleActiveReloadBannerActivation("focus"));
    window.addEventListener("pageshow", evt => handleActiveReloadBannerActivation("pageshow", { force: Boolean(evt.persisted) }));
  }

  function isOverviewTabInactive() {
    return document.visibilityState === "hidden"
      || (typeof document.hasFocus === "function" && !document.hasFocus());
  }

  function markActiveReloadBannerInactive(reason) {
    activeReloadBannerWasInactive = true;
    hideActiveReloadBanner();
    logOverview("active reload banner marked inactive", { reason });
  }

  function handleActiveReloadBannerActivation(reason, options = {}) {
    if (document.visibilityState === "hidden") return;
    if (!activeReloadBannerWasInactive && !options.force) return;
    activeReloadBannerWasInactive = false;
    showActiveReloadBanner(reason);
  }

  function showActiveReloadBanner(reason) {
    if (!shouldShowActiveReloadBanner()) return;

    const now = Date.now();
    if (now - activeReloadBannerLastShownAt < ACTIVE_RELOAD_BANNER_COOLDOWN_MS) {
      logOverview("active reload banner skipped by cooldown", {
        reason,
        elapsedMs: now - activeReloadBannerLastShownAt
      });
      return;
    }

    activeReloadBannerLastShownAt = now;
    activeReloadBannerRemainingSeconds = ACTIVE_RELOAD_BANNER_COUNTDOWN_SECONDS;
    const banner = ensureActiveReloadBanner();
    banner.classList.remove("hidden");
    banner.dataset.visible = "true";
    updateActiveReloadBannerText();
    startActiveReloadBannerCountdown();
    logOverview("active reload banner shown", { reason });
  }

  function shouldShowActiveReloadBanner() {
    const dataSourceDialog = document.getElementById("data-source-dialog");
    const dataSourceDialogVisible = Boolean(dataSourceDialog && !dataSourceDialog.classList.contains("hidden"));
    return Boolean(manifest)
      && activeDataSource === DATA_SOURCE_LOCAL
      && document.visibilityState !== "hidden"
      && !dataSourceDialogVisible;
  }

  function ensureActiveReloadBanner() {
    let banner = document.getElementById("active-reload-banner");
    if (banner) return banner;

    banner = document.createElement("section");
    banner.id = "active-reload-banner";
    banner.className = "active-reload-banner hidden";
    banner.setAttribute("role", "status");
    banner.setAttribute("aria-live", "polite");
    banner.innerHTML = `
      <span class="active-reload-banner-message" data-role="message"></span>
      <span class="active-reload-banner-countdown" data-role="countdown"></span>
      <button class="active-reload-banner-action" type="button" data-role="reload"></button>
      <button class="active-reload-banner-dismiss" type="button" data-role="dismiss" aria-label=""></button>
    `;

    banner.querySelector("[data-role='reload']").addEventListener("click", () => {
      window.location.reload();
    });
    banner.querySelector("[data-role='dismiss']").addEventListener("click", () => {
      hideActiveReloadBanner();
    });
    document.body.appendChild(banner);
    return banner;
  }

  function updateActiveReloadBannerText() {
    const banner = ensureActiveReloadBanner();
    const message = banner.querySelector("[data-role='message']");
    const countdown = banner.querySelector("[data-role='countdown']");
    const reload = banner.querySelector("[data-role='reload']");
    const dismiss = banner.querySelector("[data-role='dismiss']");
    message.textContent = t("activeReloadBannerMessage");
    countdown.textContent = t("activeReloadBannerCountdown").replace("{seconds}", String(activeReloadBannerRemainingSeconds));
    reload.textContent = t("activeReloadBannerReload");
    dismiss.textContent = t("activeReloadBannerDismiss");
    dismiss.setAttribute("aria-label", t("activeReloadBannerDismiss"));
  }

  function refreshActiveReloadBannerLanguage() {
    const banner = document.getElementById("active-reload-banner");
    if (!banner || banner.classList.contains("hidden")) return;
    updateActiveReloadBannerText();
  }

  function startActiveReloadBannerCountdown() {
    clearActiveReloadBannerTimer();
    activeReloadBannerTimer = window.setInterval(() => {
      activeReloadBannerRemainingSeconds -= 1;
      if (activeReloadBannerRemainingSeconds <= 0) {
        hideActiveReloadBanner();
        return;
      }
      updateActiveReloadBannerText();
    }, 1000);
  }

  function hideActiveReloadBanner() {
    clearActiveReloadBannerTimer();
    const banner = document.getElementById("active-reload-banner");
    if (!banner) return;
    banner.classList.add("hidden");
    banner.dataset.visible = "false";
  }

  function clearActiveReloadBannerTimer() {
    if (!activeReloadBannerTimer) return;
    window.clearInterval(activeReloadBannerTimer);
    activeReloadBannerTimer = null;
  }

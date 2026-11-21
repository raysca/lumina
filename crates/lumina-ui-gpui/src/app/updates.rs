//! Self-update: Check for Updates, the opt-in daily check, the download in
//! the background, and the install at quit (Restart to Update, or the next
//! quit after Later). The work is in `lumina_core::update`.

use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use gpui::*;
use lumina_core::update::install::{self, StagedUpdate};
use lumina_core::update::{self as core_update, Release};

use crate::app::preferences::PreferencesTab;
use crate::app::LuminaApp;
use crate::views::toast::{ToastAction, ToastKind};

/// The daily check waits this long after the last one.
// Only launch (not in tests) and the macOS-only update tests call the check.
#[cfg_attr(all(test, not(target_os = "macos")), allow(dead_code))]
const AUTO_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Whether this build offers updates: only macOS has published releases.
pub const fn supported() -> bool {
    cfg!(any(target_os = "macos", target_os = "linux"))
}

/// Where the update flow is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdatePhase {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Downloading {
        version: String,
    },
    /// Verified and waiting; installs at the next quit.
    Ready(StagedUpdate),
    /// A newer release exists, but this build must not replace itself: the
    /// package manager (Flatpak/apt) owns the files. `message` says how to
    /// install it; `page_url` opens the release page.
    Managed {
        version: String,
        page_url: String,
        message: String,
    },
    /// `page_url` offers the release page for a manual download.
    Failed {
        message: String,
        page_url: Option<String>,
    },
}

/// The steps that touch the network, the disk and the process. Tests swap
/// in fakes so they never download or replace anything.
#[derive(Clone, Copy)]
pub struct UpdateBackend {
    pub check: fn(&AtomicBool) -> Result<Release>,
    pub manifest: fn(&Release, &AtomicBool) -> Result<core_update::manifest::UpdateManifest>,
    pub stage: fn(&Release, &AtomicBool) -> Result<StagedUpdate>,
    pub install: fn(&StagedUpdate) -> Result<()>,
    pub relaunch: fn() -> Result<()>,
}

impl Default for UpdateBackend {
    fn default() -> Self {
        Self {
            check: core_update::fetch_latest,
            manifest: core_update::fetch_manifest,
            stage: |release, cancel| install::stage(release, &install::running_bundle()?, cancel),
            install: |staged| install::install(staged, &install::running_bundle()?),
            relaunch: || install::relaunch_after_exit(&install::running_bundle()?),
        }
    }
}

/// Update state owned by [`LuminaApp`].
#[derive(Default)]
pub struct UpdateState {
    pub phase: UpdatePhase,
    pub backend: UpdateBackend,
    /// Restart to Update asked for a relaunch after the quit's install.
    /// Cleared when that quit is cancelled (the downloads prompt), so a
    /// later plain quit doesn't reopen the app.
    pub(crate) relaunch: bool,
}

impl UpdateState {
    /// Checking, downloading, or an update is waiting: Check is unavailable.
    pub fn busy(&self) -> bool {
        matches!(
            self.phase,
            UpdatePhase::Checking | UpdatePhase::Downloading { .. } | UpdatePhase::Ready(_)
        )
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

impl LuminaApp {
    /// At launch: the daily check, when the user turned it on and a day passed.
    #[cfg_attr(all(test, not(target_os = "macos")), allow(dead_code))]
    pub fn auto_check_for_updates(&mut self, cx: &mut Context<Self>) {
        let due = self
            .config
            .last_update_check_secs
            .is_none_or(|last| now_secs().saturating_sub(last) >= AUTO_CHECK_INTERVAL.as_secs());
        if supported() && self.config.auto_update_check && due {
            self.check_for_updates(false, cx);
        }
    }

    /// Looks for a newer release and, if there is one, downloads it.
    /// `manual` (Check for Updates) reports every outcome; the daily check
    /// stays quiet unless an update is ready or couldn't install.
    pub fn check_for_updates(&mut self, manual: bool, cx: &mut Context<Self>) {
        if !supported() {
            return;
        }
        // The answer shows on the About page; the menu and ⌘K open it.
        if manual && !(self.preferences_open && self.preferences_tab == PreferencesTab::About) {
            self.open_about(cx);
        }
        if self.updates.busy() {
            return;
        }
        self.set_update_phase(UpdatePhase::Checking);
        cx.notify();
        let check = self.updates.backend.check;
        let task = cx
            .background_executor()
            .spawn(async move { check(&AtomicBool::new(false)) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |app, cx| app.finish_check(result, manual, cx));
        })
        .detach();
    }

    fn finish_check(&mut self, result: Result<Release>, manual: bool, cx: &mut Context<Self>) {
        // Every attempt counts, so offline launches don't retry each time:
        // the daily check runs at most once a day. A failed save is in the banner.
        self.config.last_update_check_secs = Some(now_secs());
        self.save_config();
        match result {
            Err(err) => {
                log::warn!("update check: {err:#}");
                self.set_update_phase(if manual {
                    UpdatePhase::Failed {
                        message: err.to_string(),
                        page_url: None,
                    }
                } else {
                    UpdatePhase::Idle
                });
            }
            Ok(release) => {
                if core_update::is_newer(&release.version, env!("CARGO_PKG_VERSION")) {
                    if cfg!(target_os = "macos") {
                        self.download_update(release, manual, cx);
                    } else {
                        self.managed_update(release, manual, cx);
                    }
                } else {
                    self.set_update_phase(UpdatePhase::UpToDate);
                }
            }
        }
        cx.notify();
    }

    /// Linux: verify the release's manifest, then point at the package
    /// manager instead of replacing root-owned files.
    fn managed_update(&mut self, release: Release, manual: bool, cx: &mut Context<Self>) {
        let manifest = self.updates.backend.manifest;
        let version = release.version.clone();
        let page_url = release.page_url.clone();
        let task = cx
            .background_executor()
            .spawn(async move { manifest(&release, &AtomicBool::new(false)) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |app, cx| match result {
                Ok(_) => {
                    let message =
                        core_update::managed_message(core_update::package_kind(), &version);
                    app.set_update_phase(UpdatePhase::Managed {
                        version: version.clone(),
                        page_url,
                        message: message.clone(),
                    });
                    app.show_toast_kind(
                        ToastKind::Positive,
                        format!("Lumina {version} is available"),
                        Some(&message),
                        None,
                        cx,
                    );
                    cx.notify();
                }
                Err(err) => {
                    log::warn!("update {version}: {err:#}");
                    let message = err.to_string();
                    if !manual {
                        app.show_toast_kind(
                            ToastKind::Warning,
                            format!("Lumina {version} is available"),
                            Some(&message),
                            None,
                            cx,
                        );
                    }
                    app.set_update_phase(UpdatePhase::Failed {
                        message,
                        page_url: Some(page_url),
                    });
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn download_update(&mut self, release: Release, manual: bool, cx: &mut Context<Self>) {
        self.set_update_phase(UpdatePhase::Downloading {
            version: release.version.clone(),
        });
        let stage = self.updates.backend.stage;
        // ponytail: the download holds an executor worker for its length;
        // a dedicated thread if that ever starves thumbnails.
        let task = cx.background_executor().spawn(async move {
            let staged = stage(&release, &AtomicBool::new(false));
            (release, staged)
        });
        cx.spawn(async move |this, cx| {
            let (release, staged) = task.await;
            let _ = this.update(cx, |app, cx| match staged {
                Ok(staged) => {
                    let title = format!("Lumina {} is ready to install", staged.version);
                    app.set_update_phase(UpdatePhase::Ready(staged));
                    app.show_toast_kind(
                        ToastKind::Positive,
                        title,
                        Some("Restart now, or it installs when you quit."),
                        Some(ToastAction::RestartToUpdate),
                        cx,
                    );
                }
                Err(err) => {
                    log::warn!("update {}: {err:#}", release.version);
                    let message = err.to_string();
                    if !manual {
                        let title = format!("Lumina {} is available", release.version);
                        app.show_toast_kind(ToastKind::Warning, title, Some(&message), None, cx);
                    }
                    app.set_update_phase(UpdatePhase::Failed {
                        message,
                        page_url: Some(release.page_url),
                    });
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Restart to Update: quits (asking first if downloads are running);
    /// the quit installs the update and opens the new version.
    pub fn restart_to_update(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.updates.phase, UpdatePhase::Ready(_)) {
            return;
        }
        self.dismiss_toast(cx);
        self.updates.relaunch = true;
        self.request_quit(cx);
    }

    /// Called by the quit, last thing: installs a ready update, and
    /// relaunches when Restart to Update asked for it. A failed install
    /// keeps the current version (the swap puts it back). After Restart to
    /// Update it returns false: the quit stops and the banner says why. A
    /// plain quit goes ahead and the failure is logged.
    pub(crate) fn install_update_on_quit(&mut self) -> bool {
        let relaunch = std::mem::take(&mut self.updates.relaunch);
        let UpdatePhase::Ready(staged) = std::mem::take(&mut self.updates.phase) else {
            return true;
        };
        let backend = self.updates.backend;
        if let Err(err) = (backend.install)(&staged) {
            log::error!("installing update {}: {err:#}", staged.version);
            if !relaunch {
                return true;
            }
            self.error_message = Some(err.to_string());
            self.set_update_phase(UpdatePhase::Failed {
                message: err.to_string(),
                page_url: None,
            });
            return false;
        }
        log::info!("installed update {}", staged.version);
        if relaunch {
            if let Err(err) = (backend.relaunch)() {
                log::error!("relaunching after update: {err:#}");
            }
        }
        true
    }

    /// Settings › About › Check Daily, and the ⌘K on/off commands. A failed
    /// save puts the old choice back and shows the error banner.
    pub fn set_auto_update_check(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.config.auto_update_check == on {
            return;
        }
        self.config.auto_update_check = on;
        if !self.save_config() {
            self.config.auto_update_check = !on;
        }
        cx.notify();
    }

    /// The release page in the browser, for a manual download.
    pub fn open_release_page(&mut self, cx: &mut Context<Self>) {
        match &self.updates.phase {
            UpdatePhase::Failed {
                page_url: Some(url),
                ..
            }
            | UpdatePhase::Managed { page_url: url, .. } => cx.open_url(url),
            _ => {}
        }
    }

    /// Moves to `phase`. About's update button changes with it; a Tab
    /// focus on that button follows to the new one.
    fn set_update_phase(&mut self, phase: UpdatePhase) {
        use crate::app::preferences::SettingsButton::*;
        self.updates.phase = phase;
        if matches!(
            self.settings_focus,
            Some(CheckUpdates | RestartToUpdate | ReleasePage)
        ) {
            self.settings_focus = Some(self.update_button());
        }
    }

    /// About's update button for the current state.
    pub fn update_button(&self) -> crate::app::preferences::SettingsButton {
        use crate::app::preferences::SettingsButton;
        match &self.updates.phase {
            UpdatePhase::Ready(_) => SettingsButton::RestartToUpdate,
            UpdatePhase::Failed {
                page_url: Some(_), ..
            }
            | UpdatePhase::Managed { .. } => SettingsButton::ReleasePage,
            _ => SettingsButton::CheckUpdates,
        }
    }

    /// The About page's status line.
    pub fn update_status(&self) -> String {
        match &self.updates.phase {
            UpdatePhase::Idle => String::new(),
            UpdatePhase::Checking => "Checking for updates…".into(),
            UpdatePhase::UpToDate => "Lumina is up to date.".into(),
            UpdatePhase::Downloading { version } => format!("Downloading Lumina {version}…"),
            UpdatePhase::Ready(staged) => format!(
                "Lumina {} is ready. It installs when you restart or quit.",
                staged.version
            ),
            UpdatePhase::Managed { message, .. } => message.clone(),
            UpdatePhase::Failed { message, .. } => message.clone(),
        }
    }
}

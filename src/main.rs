// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cidr;
mod i18n;
mod icon;
mod installer;
mod log;
mod net;
mod netsh;
mod store;
mod win;
mod worker;

use std::cell::RefCell;
use std::net::Ipv4Addr;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use slint::{ModelRc, Timer, TimerMode, VecModel};

use cidr::Cidr;
use store::UiState;
use worker::{AdapterRef, Event, Job};

slint::include_modules!();

fn parse_dns_list(text: &str) -> Result<Vec<Ipv4Addr>, String> {
    let mut servers = Vec::new();
    for raw in text.split(|c: char| c == ',' || c == ';' || c.is_whitespace()) {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let ip = raw
            .parse::<Ipv4Addr>()
            .map_err(|_| crate::i18n::tr().invalid_dns(raw))?;
        if ip.is_unspecified() || ip.is_multicast() {
            return Err(crate::i18n::tr().invalid_dns(raw));
        }
        if !servers.contains(&ip) {
            servers.push(ip);
        }
    }
    Ok(servers)
}

struct Alias {
    id: i32,
    cidr: Cidr,
    enabled: bool,
    busy: bool,
}

struct App {
    window: slint::Weak<MainWindow>,
    tray: Option<slint::Weak<AppTray>>,
    admin: bool,
    adapters: Vec<net::AdapterInfo>,
    selected: usize,
    aliases: Vec<Alias>,
    next_id: i32,
    ui_state: UiState,
    ip_input: String,
    gateway_input: String,
    dns_input: String,
    busy: bool,
    session_dirty: bool,
    message: String,
    message_error: bool,
    quitting: bool,
    jobs: Sender<Job>,
    events: Receiver<Event>,
}

fn main() -> anyhow::Result<()> {
    log::init();
    log::write("--- démarrage de FastIpSwitch ---");

    let admin = win::is_admin();
    let wants_elevate = std::env::args().any(|arg| arg == "--elevate");
    log::write(if admin {
        "Mode administrateur"
    } else {
        "Mode sans élévation : interface verrouillée"
    });

    if wants_elevate && !admin {
        if win::admin_instance_running() {
            win::info_box(crate::i18n::tr().already_running_admin());
            return Ok(());
        }
        if let Err(e) = win::elevate_self() {
            win::error_box(&crate::i18n::tr().elevate_failed(&e));
        }
        return Ok(());
    }

    if !win::single_instance(admin) {
        win::info_box(crate::i18n::tr().already_running());
        return Ok(());
    }

    let mut adapters = net::read_adapters().unwrap_or_default();
    let recovery = if admin {
        worker::recover_pending(&adapters)
    } else {
        None
    };
    if recovery.is_some() {
        adapters = net::read_adapters().unwrap_or_default();
    }

    let forced_theme = std::env::var("FASTIPSWITCH_THEME").ok().and_then(|value| {
        match value.to_ascii_lowercase().as_str() {
            "dark" | "sombre" => Some(true),
            "light" | "clair" => Some(false),
            _ => None,
        }
    });

    let window = MainWindow::new()?;
    window
        .global::<RackTheme>()
        .set_dark(forced_theme.unwrap_or_else(win::system_is_dark));
    window.set_admin(admin);
    window.set_logo(icon::logo_image());

    let tray = if admin {
        let tray = AppTray::new()?;
        tray.set_tray_icon(icon::tray_image());
        tray.set_tip("FastIpSwitch".into());
        Some(tray)
    } else {
        None
    };

    {
        let t = crate::i18n::tr();
        window.set_t_address(t.address_label().into());
        window.set_t_virtual(t.virtual_label().into());
        window.set_t_apply(t.apply().into());
        window.set_t_gateway(t.gateway_placeholder().into());
        window.set_t_add(t.add().into());
        window.set_t_quit(t.quit().into());
        window.set_t_no_aliases(t.no_aliases().into());
        window.set_t_no_adapter(t.no_adapter().into());
        window.set_t_lock_title(t.lock_title().into());
        window.set_t_lock_body(t.lock_body().into());
        window.set_t_elevate(t.elevate().into());
        window.set_t_close(t.close().into());
        window.set_t_install_title(t.install_title().into());
        window.set_t_install_dest(t.install_dest().into());
        window.set_t_install_autostart(t.install_autostart().into());
        window.set_t_install_autostart_hint(t.install_autostart_hint().into());
        window.set_t_install_button(t.install_button().into());
        window.set_t_reinstall(t.reinstall_button().into());
        window.set_t_cancel(t.cancel().into());
        window.set_t_add_tip(t.add_tip().into());
        window.set_t_remove_tip(t.remove_tip().into());
        window.set_t_toggle_tip(t.toggle_tip().into());
        window.set_t_dhcp_tip(t.dhcp_tip().into());
        window.set_t_install_tip(t.install_tip().into());
        window.set_t_installed_tag(t.installed_tag().into());
        window.set_t_gateway_label(t.gateway_label().into());
        window.set_t_dns_label(t.dns_label().into());
        window.set_t_dns_placeholder(t.dns_placeholder().into());
        window.set_t_autostart_tip(t.autostart_tip().into());
        window.set_installed(installer::is_installed());
        window.set_autostart(installer::autostart_enabled());
        window.set_install_dir(installer::install_dir().display().to_string().into());
        if let Some(tray) = &tray {
            tray.set_menu_show(t.menu_show().into());
            tray.set_menu_dhcp(t.menu_dhcp().into());
            tray.set_menu_quit(t.menu_quit().into());
        }
    }

    let (event_tx, event_rx) = std::sync::mpsc::channel::<Event>();
    let jobs = if admin {
        worker::spawn(event_tx)
    } else {
        // Aucune commande réseau possible sans élévation : pas de worker.
        let (tx, _rx) = std::sync::mpsc::channel();
        tx
    };
    let ui_state = store::load_ui_state();

    let app = Rc::new(RefCell::new(App::new(
        &window,
        tray.as_ref(),
        admin,
        adapters,
        ui_state,
        jobs,
        event_rx,
    )));

    {
        let app = app.clone();
        window.on_refresh(move || app.borrow_mut().refresh());
    }
    {
        let app = app.clone();
        window.on_select_adapter(move |name| app.borrow_mut().select_adapter(&name));
    }
    {
        let app = app.clone();
        window.on_apply(move || app.borrow_mut().apply());
    }
    {
        let app = app.clone();
        window.on_dhcp(move || app.borrow_mut().dhcp());
    }
    {
        let app = app.clone();
        window.on_add_alias(move |text| app.borrow_mut().add_alias(&text));
    }
    {
        let app = app.clone();
        window.on_toggle_alias(move |id, checked| app.borrow_mut().toggle_alias(id, checked));
    }
    {
        let app = app.clone();
        window.on_remove_alias(move |id| app.borrow_mut().remove_alias(id));
    }
    {
        let app = app.clone();
        window.on_quit(move || app.borrow_mut().quit());
    }
    {
        let app = app.clone();
        window.on_toggle_dhcp(move |enabled| {
            if enabled {
                app.borrow_mut().dhcp();
            } else {
                app.borrow_mut().apply();
            }
        });
    }
    {
        let app = app.clone();
        window.on_elevate(move || {
            log::write("Demande d'élévation");
            if win::admin_instance_running() {
                app.borrow()
                    .set_lock_error(crate::i18n::tr().admin_instance_running().into());
                return;
            }
            match win::elevate_self() {
                Ok(()) => {
                    let _ = slint::quit_event_loop();
                }
                Err(e) => app
                    .borrow()
                    .set_lock_error(crate::i18n::tr().elevation_cancelled(&e)),
            }
        });
    }
    {
        window.on_close_app(|| {
            let _ = slint::quit_event_loop();
        });
    }
    {
        let app = app.clone();
        window.on_install(move |autostart| {
            if !app.borrow().admin {
                return;
            }
            let t = crate::i18n::tr();
            match installer::install(autostart) {
                Ok(path) => {
                    if let Some(window) = app.borrow().window.upgrade() {
                        window.set_installer_open(false);
                        window.set_installed(true);
                        window.set_install_dir(path.clone().into());
                    }
                    app.borrow_mut().set_message(t.installed_ok(&path), false);
                }
                Err(e) => {
                    if let Some(window) = app.borrow().window.upgrade() {
                        window.set_installer_message(t.install_failed(&e).into());
                        window.set_installer_error(true);
                    }
                }
            }
        });
    }
    {
        let app = app.clone();
        window.on_close_installer(move || {
            if let Some(window) = app.borrow().window.upgrade() {
                window.set_installer_open(false);
            }
        });
    }

    if let Some(tray) = &tray {
        {
            let app = app.clone();
            tray.on_show_window(move || app.borrow().show_window());
        }
        {
            let app = app.clone();
            tray.on_activate(move || app.borrow().show_window());
        }
        {
            let app = app.clone();
            tray.on_restore_dhcp(move || app.borrow_mut().dhcp());
        }
        {
            let app = app.clone();
            tray.on_quit(move || app.borrow_mut().quit());
        }
    }

    {
        let app = app.clone();
        window.window().on_close_requested(move || {
            if app.borrow().admin {
                slint::CloseRequestResponse::HideWindow
            } else {
                let _ = slint::quit_event_loop();
                slint::CloseRequestResponse::KeepWindowShown
            }
        });
    }

    {
        let app = app.borrow();
        app.sync();
        app.push_inputs();
    }
    if let Some(note) = recovery {
        app.borrow_mut().set_message(note, false);
    }

    window.show()?;
    if let Some(tray) = &tray {
        tray.show()?;
    }

    let timer = Timer::default();
    let app_timer = app.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        App::pump(&app_timer);
    });

    let theme_timer = Timer::default();
    let theme_window = window.as_weak();
    theme_timer.start(TimerMode::Repeated, Duration::from_secs(1), move || {
        if !admin && win::admin_instance_running() {
            let _ = slint::quit_event_loop();
            return;
        }
        if forced_theme.is_some() {
            return;
        }
        if let Some(window) = theme_window.upgrade() {
            let theme = window.global::<RackTheme>();
            let dark = win::system_is_dark();
            if theme.get_dark() != dark {
                theme.set_dark(dark);
            }
        }
    });

    let refresh_timer = Timer::default();
    let refresh_app = app.clone();
    refresh_timer.start(TimerMode::Repeated, Duration::from_secs(2), move || {
        App::auto_refresh(&refresh_app);
    });

    slint::run_event_loop()?;
    log::write("--- arrêt de FastIpSwitch ---");
    drop(refresh_timer);
    drop(theme_timer);
    drop(timer);
    Ok(())
}

impl App {
    fn new(
        window: &MainWindow,
        tray: Option<&AppTray>,
        admin: bool,
        adapters: Vec<net::AdapterInfo>,
        ui_state: UiState,
        jobs: Sender<Job>,
        events: Receiver<Event>,
    ) -> Self {
        let selected = ui_state
            .adapter_mac
            .as_ref()
            .and_then(|mac| {
                adapters
                    .iter()
                    .position(|a| !a.mac.is_empty() && a.mac.eq_ignore_ascii_case(mac))
            })
            .or_else(|| adapters.iter().position(|a| a.is_up && a.gateway.is_some()))
            .or_else(|| adapters.iter().position(|a| a.is_up))
            .unwrap_or(0);

        let aliases: Vec<Alias> = ui_state
            .aliases
            .iter()
            .filter_map(|s| Cidr::parse(s).ok())
            .enumerate()
            .map(|(i, cidr)| Alias {
                id: i as i32,
                cidr,
                enabled: false,
                busy: false,
            })
            .collect();

        let next_id = aliases.len() as i32;
        let mut app = Self {
            window: window.as_weak(),
            tray: tray.map(|tray| tray.as_weak()),
            admin,
            adapters,
            selected,
            aliases,
            next_id,
            ui_state,
            ip_input: String::new(),
            gateway_input: String::new(),
            dns_input: String::new(),
            busy: false,
            session_dirty: store::load_session()
                .map(|session| !session.is_empty())
                .unwrap_or(false),
            message: crate::i18n::tr().ready().into(),
            message_error: false,
            quitting: false,
            jobs,
            events,
        };
        app.fill_inputs();
        app
    }

    fn adapter(&self) -> Option<&net::AdapterInfo> {
        self.adapters.get(self.selected)
    }

    fn fill_inputs(&mut self) {
        let Some(adapter) = self.adapters.get(self.selected) else {
            return;
        };
        let primary = adapter.primary().map(|p| p.cidr.to_string());
        let gateway = adapter.gateway.map(|g| g.to_string());
        let dns = adapter
            .dns
            .iter()
            .map(|ip| ip.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let mac = adapter.mac.clone();

        if let Some(primary) = primary {
            self.ip_input = primary;
            self.gateway_input = gateway.unwrap_or_default();
            self.dns_input = dns;
        } else {
            // APIPA (169.254.x.x) ou aucune adresse : on ne préremplit pas
            // avec une valeur d'un autre réseau.
            self.ip_input.clear();
            self.gateway_input.clear();
            self.dns_input.clear();
        }
        self.ui_state.adapter_mac = Some(mac);
    }

    fn push_inputs(&self) {
        if let Some(window) = self.window.upgrade() {
            window.set_ip_input(self.ip_input.clone().into());
            window.set_gateway_input(self.gateway_input.clone().into());
            window.set_dns_input(self.dns_input.clone().into());
        }
    }

    fn sync(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };

        let names: Vec<slint::SharedString> = self
            .adapters
            .iter()
            .map(|a| slint::SharedString::from(a.name.clone()))
            .collect();
        window.set_adapter_names(ModelRc::new(VecModel::from(names)));
        window.set_selected_adapter(self.selected as i32);
        window.set_has_adapters(!self.adapters.is_empty());
        let is_dhcp = self.adapters.get(self.selected).is_some_and(|a| a.dhcp);
        window.set_dhcp_mode(is_dhcp);

        let t = crate::i18n::tr();
        let (status, mode) = match self.adapters.get(self.selected) {
            Some(adapter) => (
                adapter
                    .primary()
                    .map(|p| p.cidr.to_string())
                    .unwrap_or_else(|| "—".to_string()),
                if adapter.dhcp {
                    t.mode_dhcp()
                } else {
                    t.mode_static()
                }
                .to_string(),
            ),
            None => (t.no_adapter().to_string(), "—".to_string()),
        };
        window.set_status_main(status.clone().into());
        window.set_mode_text(mode.clone().into());

        let rows: Vec<AliasRow> = self
            .aliases
            .iter()
            .map(|a| AliasRow {
                id: a.id,
                cidr: a.cidr.to_string().into(),
                enabled: a.enabled,
                busy: a.busy,
            })
            .collect();
        window.set_aliases(ModelRc::new(VecModel::from(rows)));
        window.set_active_count(self.aliases.iter().filter(|a| a.enabled).count() as i32);
        window.set_busy(self.busy);
        window.set_message(self.message.clone().into());
        window.set_message_error(self.message_error);

        if let Some(tray) = self.tray.as_ref().and_then(|tray| tray.upgrade()) {
            let active = self.aliases.iter().filter(|a| a.enabled).count();
            let tip = t.tooltip(&status, &mode, active);
            tray.set_tip(tip.into());
        }
    }

    fn set_message(&mut self, text: impl Into<String>, error: bool) {
        self.message = text.into();
        self.message_error = error;
        self.sync();
    }

    fn set_lock_error(&self, text: String) {
        if let Some(window) = self.window.upgrade() {
            window.set_elevation_error(text.into());
        }
    }

    fn show_window(&self) {
        if let Some(window) = self.window.upgrade() {
            let _ = window.show();
            window.window().request_redraw();
        }
    }

    fn refresh(&mut self) {
        match net::read_adapters() {
            Ok(adapters) => {
                let mac = self.adapter().map(|a| a.mac.clone());
                self.adapters = adapters;
                if let Some(mac) = mac
                    && let Some(pos) = self
                        .adapters
                        .iter()
                        .position(|a| !mac.is_empty() && a.mac.eq_ignore_ascii_case(&mac))
                {
                    self.selected = pos;
                }
                if self.selected >= self.adapters.len() {
                    self.selected = 0;
                }
                self.message = crate::i18n::tr().refreshed().into();
                self.message_error = false;
            }
            Err(e) => {
                self.message = crate::i18n::tr().read_failed(&e.to_string());
                self.message_error = true;
            }
        }
        self.sync();
    }

    fn select_adapter(&mut self, name: &str) {
        if !self.admin {
            return;
        }
        if self.busy || self.quitting {
            self.sync();
            return;
        }
        if self.session_dirty {
            self.set_message(crate::i18n::tr().finish_changes_first(), true);
            return;
        }
        if self.aliases.iter().any(|a| a.enabled) {
            self.set_message(crate::i18n::tr().disable_aliases_first(), true);
            return;
        }

        // Relit l'état réel : le Wi-Fi a pu se reconnecter, un bail DHCP
        // arriver, l'adaptateur disparaître, etc.
        match net::read_adapters() {
            Ok(adapters) => self.adapters = adapters,
            Err(e) => {
                self.set_message(crate::i18n::tr().read_failed(&e.to_string()), true);
                return;
            }
        }

        let Some(pos) = self.adapters.iter().position(|a| a.name == name) else {
            self.set_message(crate::i18n::tr().adapter_not_found(name), true);
            return;
        };

        self.selected = pos;
        let mac = self.adapters[pos].mac.clone();
        self.ui_state.adapter_mac = Some(mac);
        store::save_ui_state(&self.ui_state);
        self.fill_inputs();
        self.push_inputs();

        if self.adapters[pos].primary().is_none() {
            self.set_message(crate::i18n::tr().no_usable_address(name), true);
            return;
        }

        self.message = crate::i18n::tr().adapter_selected(&self.adapters[pos].name);
        self.message_error = false;
        self.sync();
    }

    fn apply(&mut self) {
        if !self.admin || self.busy || self.quitting {
            return;
        }
        let Some(adapter) = self.adapter().cloned() else {
            self.set_message(crate::i18n::tr().no_adapter_selected(), true);
            return;
        };

        let (ip_text, gw_text, dns_text) = match self.window.upgrade() {
            Some(window) => (
                window.get_ip_input().to_string(),
                window.get_gateway_input().to_string(),
                window.get_dns_input().to_string(),
            ),
            None => (
                self.ip_input.clone(),
                self.gateway_input.clone(),
                self.dns_input.clone(),
            ),
        };
        self.ip_input = ip_text.clone();
        self.gateway_input = gw_text.clone();
        self.dns_input = dns_text.clone();

        let cidr = match Cidr::parse(&ip_text) {
            Ok(cidr) => cidr,
            Err(e) => {
                self.set_message(e, true);
                return;
            }
        };

        let gateway = match gw_text.trim() {
            "" => None,
            raw => match raw.parse::<Ipv4Addr>() {
                Ok(gateway) if gateway == cidr.addr => {
                    self.set_message(crate::i18n::tr().gateway_is_address(), true);
                    return;
                }
                Ok(gateway) if !cidr.contains(gateway) => {
                    self.set_message(crate::i18n::tr().gateway_not_in_subnet(gateway, cidr), true);
                    return;
                }
                Ok(gateway)
                    if cidr.prefix <= 30
                        && (gateway == cidr.network() || gateway == cidr.broadcast()) =>
                {
                    self.set_message(crate::i18n::tr().gateway_not_host(gateway), true);
                    return;
                }
                Ok(gateway) => Some(gateway),
                Err(_) => {
                    self.set_message(crate::i18n::tr().invalid_gateway(raw), true);
                    return;
                }
            },
        };

        let dns = match parse_dns_list(&dns_text) {
            Ok(dns) => dns,
            Err(e) => {
                self.set_message(e, true);
                return;
            }
        };

        self.ui_state.last_ip = cidr.to_string();
        self.ui_state.last_gateway = gw_text.trim().to_string();
        self.ui_state.last_dns = dns_text.trim().to_string();
        store::save_ui_state(&self.ui_state);

        let aliases: Vec<Cidr> = self
            .aliases
            .iter()
            .filter(|a| a.enabled)
            .map(|a| a.cidr)
            .collect();

        self.busy = true;
        self.message = crate::i18n::tr().applying(cidr);
        self.message_error = false;
        self.sync();

        let _ = self.jobs.send(Job::ApplyStatic {
            adapter: AdapterRef::from(&adapter),
            cidr,
            gateway,
            dns,
            aliases,
        });
    }

    fn dhcp(&mut self) {
        if !self.admin || self.busy || self.quitting {
            return;
        }
        let Some(adapter) = self.adapter().cloned() else {
            self.set_message(crate::i18n::tr().no_adapter_selected(), true);
            return;
        };

        self.busy = true;
        self.message = crate::i18n::tr().returning_dhcp().into();
        self.message_error = false;
        self.sync();

        let _ = self.jobs.send(Job::SetDhcp {
            adapter: AdapterRef::from(&adapter),
        });
    }

    fn add_alias(&mut self, text: &str) {
        if !self.admin || self.busy || self.quitting {
            return;
        }
        let Some(adapter) = self.adapter().cloned() else {
            self.set_message(crate::i18n::tr().no_adapter_selected(), true);
            return;
        };

        let cidr = match Cidr::parse(text) {
            Ok(cidr) => cidr,
            Err(e) => {
                self.set_message(e, true);
                return;
            }
        };

        if adapter.has_address(cidr.addr) || self.aliases.iter().any(|a| a.cidr.addr == cidr.addr) {
            self.set_message(crate::i18n::tr().alias_exists(cidr.addr), true);
            return;
        }

        let id = self.next_id;
        self.next_id += 1;
        self.aliases.push(Alias {
            id,
            cidr,
            enabled: true,
            busy: true,
        });
        self.persist_aliases();

        if let Some(window) = self.window.upgrade() {
            window.set_adding(false);
            window.set_add_input("".into());
        }

        self.busy = true;
        self.message = crate::i18n::tr().enabling(cidr);
        self.message_error = false;
        self.sync();

        let _ = self.jobs.send(Job::AddAlias {
            id,
            adapter: AdapterRef::from(&adapter),
            cidr,
        });
    }

    fn toggle_alias(&mut self, id: i32, checked: bool) {
        if !self.admin || self.busy || self.quitting {
            self.sync();
            return;
        }
        let Some(alias) = self.aliases.iter_mut().find(|a| a.id == id) else {
            return;
        };
        if alias.busy {
            return;
        }
        alias.enabled = checked;
        alias.busy = true;
        let cidr = alias.cidr;

        let Some(adapter) = self.adapter().cloned() else {
            self.set_message(crate::i18n::tr().no_adapter_selected(), true);
            return;
        };

        self.busy = true;
        self.message = if checked {
            crate::i18n::tr().enabling(cidr)
        } else {
            crate::i18n::tr().disabling(cidr)
        };
        self.message_error = false;
        self.sync();

        let _ = self.jobs.send(Job::SetAlias {
            id,
            adapter: AdapterRef::from(&adapter),
            cidr,
            enabled: checked,
        });
    }

    fn remove_alias(&mut self, id: i32) {
        if !self.admin || self.busy || self.quitting {
            return;
        }
        let Some(pos) = self.aliases.iter().position(|a| a.id == id) else {
            return;
        };

        if self.aliases[pos].enabled {
            let Some(adapter) = self.adapter().cloned() else {
                self.set_message(crate::i18n::tr().no_adapter_selected(), true);
                return;
            };
            self.aliases[pos].busy = true;
            let cidr = self.aliases[pos].cidr;

            self.busy = true;
            self.message = crate::i18n::tr().removing(cidr);
            self.message_error = false;
            self.sync();

            let _ = self.jobs.send(Job::RemoveAlias {
                id,
                adapter: AdapterRef::from(&adapter),
                cidr,
            });
        } else {
            self.aliases.remove(pos);
            self.persist_aliases();
            self.message = crate::i18n::tr().alias_removed().into();
            self.message_error = false;
            self.sync();
        }
    }

    fn persist_aliases(&mut self) {
        self.ui_state.aliases = self.aliases.iter().map(|a| a.cidr.to_string()).collect();
        store::save_ui_state(&self.ui_state);
    }

    fn quit(&mut self) {
        if !self.admin || self.quitting {
            return;
        }
        self.quitting = true;
        self.busy = true;
        self.message = crate::i18n::tr().restoring_quit().into();
        self.message_error = false;
        self.sync();
        let _ = self.jobs.send(Job::Quit);
    }

    fn auto_refresh(app: &Rc<RefCell<App>>) {
        let mut app = app.borrow_mut();

        if !app.admin
            || app.busy
            || app.quitting
            || app.session_dirty
            || app.aliases.iter().any(|a| a.enabled)
        {
            return;
        }

        let Some(window) = app.window.upgrade() else {
            return;
        };
        if window.get_adapter_menu_open() {
            return;
        }

        // Adaptateur courant connecté et avec une adresse exploitable : rien à faire.
        if app
            .adapter()
            .is_some_and(|adapter| adapter.is_up && adapter.primary().is_some())
        {
            return;
        }

        let Ok(adapters) = net::read_adapters() else {
            return;
        };

        let candidate = adapters
            .iter()
            .position(|adapter| {
                adapter.is_up && adapter.primary().is_some() && adapter.gateway.is_some()
            })
            .or_else(|| {
                adapters
                    .iter()
                    .position(|adapter| adapter.is_up && adapter.primary().is_some())
            });

        let Some(index) = candidate else {
            return;
        };

        let candidate_mac = adapters[index].mac.clone();
        let candidate_name = adapters[index].name.clone();
        let current_is_up = app.adapter().is_some_and(|current| current.is_up);
        let same = app.adapter().is_some_and(|current| {
            !current.mac.is_empty() && current.mac.eq_ignore_ascii_case(&candidate_mac)
        });

        // Ne pas voler la sélection si l'utilisateur a explicitement choisi
        // un adaptateur déjà déconnecté pour le configurer.
        if !same && !current_is_up {
            return;
        }

        app.adapters = adapters;
        app.selected = index;

        if !same {
            app.ui_state.adapter_mac = Some(candidate_mac);
            store::save_ui_state(&app.ui_state);
        }

        app.fill_inputs();
        app.push_inputs();
        app.message = crate::i18n::tr().adapter_selected(&candidate_name);
        app.message_error = false;
        app.sync();
    }

    fn pump(app: &Rc<RefCell<App>>) {
        let mut batch = Vec::new();
        {
            let app = app.borrow();
            while let Ok(event) = app.events.try_recv() {
                batch.push(event);
            }
        }
        if batch.is_empty() {
            return;
        }

        let mut app = app.borrow_mut();
        for event in batch {
            match event {
                Event::Busy(busy) => app.busy = busy,
                Event::Adapters(list) => {
                    let mac = app.adapter().map(|a| a.mac.clone());
                    app.adapters = list;
                    if let Some(mac) = mac
                        && let Some(pos) = app
                            .adapters
                            .iter()
                            .position(|a| !mac.is_empty() && a.mac.eq_ignore_ascii_case(&mac))
                    {
                        app.selected = pos;
                    }
                    if app.selected >= app.adapters.len() {
                        app.selected = 0;
                    }
                }
                Event::Message { text, error } => {
                    app.message = text;
                    app.message_error = error;
                }
                Event::AliasState { id, enabled } => {
                    if let Some(alias) = app.aliases.iter_mut().find(|a| a.id == id) {
                        alias.enabled = enabled;
                        alias.busy = false;
                    }
                }
                Event::AliasGone { id } => {
                    app.aliases.retain(|a| a.id != id);
                    app.persist_aliases();
                }
                Event::DisableAll => {
                    for alias in &mut app.aliases {
                        alias.enabled = false;
                        alias.busy = false;
                    }
                }
                Event::Dirty(dirty) => {
                    app.session_dirty = dirty;
                }
                Event::QuitFailed => {
                    app.quitting = false;
                    app.busy = false;
                }
                Event::QuitReady => {
                    log::write("Restauration terminée, fermeture.");
                    if let Some(tray) = app.tray.as_ref().and_then(|tray| tray.upgrade()) {
                        let _ = tray.hide();
                    }
                    if let Some(window) = app.window.upgrade() {
                        let _ = window.hide();
                    }
                    let _ = slint::quit_event_loop();
                }
            }
        }
        app.sync();
    }
}

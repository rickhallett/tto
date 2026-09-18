//! The menu bar app: the whole GUI for now. A stick man, a plug, and the
//! presets. It talks to the root helper over the socket like the CLI does.

use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel,
};
use objc2_app_kit::{
    NSAlert, NSApplication, NSApplicationActivationPolicy, NSControlStateValueOff,
    NSControlStateValueOn, NSImage, NSMenu, NSMenuDelegate, NSMenuItem, NSStatusBar, NSStatusItem,
    NSVariableStatusItemLength,
};
use objc2_foundation::{NSData, NSObject, NSObjectProtocol, NSSize, NSString, NSTimer, ns_string};

use crate::blocklist::Blocklist;
use crate::paths::Paths;
use crate::proto::{self, ClientError, Request};
use crate::state::State;
use crate::when;

const PRESETS: &[(&str, &str)] = &[
    ("For 1 hour", "1h"),
    ("For 2 hours", "2h"),
    ("For 4 hours", "4h"),
    ("Tonight, until 7am", "tonight"),
    ("The rest of today", "today"),
    ("The weekend", "weekend"),
];

struct Ivars {
    paths: Paths,
    status_item: RefCell<Option<Retained<NSStatusItem>>>,
    /// Category names ticked in "What to turn off". Default: all.
    selected: RefCell<Vec<String>>,
    blocklist: Blocklist,
    last: RefCell<Option<State>>,
    installed: Cell<bool>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; Controller has no Drop impl.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    struct Controller;

    unsafe impl NSObjectProtocol for Controller {}

    unsafe impl NSMenuDelegate for Controller {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            self.refresh();
            self.rebuild(menu);
        }
    }

    impl Controller {
        #[unsafe(method(tick:))]
        fn tick(&self, _timer: Option<&AnyObject>) {
            self.poll();
        }

        #[unsafe(method(preset:))]
        fn preset(&self, sender: &NSMenuItem) {
            let Some((label, spec)) = PRESETS.get(sender.tag() as usize) else { return };
            let until = match when::parse(spec) {
                Ok(u) => u,
                Err(e) => return self.alert("Can't do that", &e),
            };
            let selected = self.ivars().selected.borrow().clone();
            let what = if selected.len() == self.ivars().blocklist.categories.len() {
                "everything".to_string()
            } else {
                selected
                    .iter()
                    .filter_map(|n| self.ivars().blocklist.categories.get(n))
                    .map(|c| c.title.to_lowercase())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let msg = format!("Turn off {what} {}?", label.to_lowercase());
            let info = format!(
                "Until {}. This cannot be turned off early. Not by you, not by restarting. There is a recovery procedure; it takes about ten minutes on purpose.",
                when::describe(until)
            );
            if !self.confirm(&msg, &info, "Turn them off") {
                return;
            }
            match proto::call(&self.ivars().paths.socket(), &Request::Off { until, categories: selected }) {
                Ok(_) => self.poll(),
                Err(e) => self.alert("That didn't work", &e.to_string()),
            }
        }

        #[unsafe(method(toggleCategory:))]
        fn toggle_category(&self, sender: &NSMenuItem) {
            let names = self.ivars().blocklist.names();
            let Some(name) = names.get(sender.tag() as usize) else { return };
            let mut sel = self.ivars().selected.borrow_mut();
            match sel.iter().position(|s| s == name) {
                Some(i) if sel.len() > 1 => {
                    sel.remove(i);
                }
                Some(_) => {}
                None => sel.push(name.to_string()),
            }
        }

        #[unsafe(method(install:))]
        fn install(&self, _sender: Option<&AnyObject>) {
            let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
            let script = format!(
                "do shell script \"{} install\" with administrator privileges with prompt \"Turn Them Off needs to install its helper. This happens once.\"",
                exe.replace('"', "\\\"")
            );
            let out = std::process::Command::new("/usr/bin/osascript").args(["-e", &script]).output();
            match out {
                Ok(o) if o.status.success() => self.poll(),
                Ok(o) => self.alert("Install failed", String::from_utf8_lossy(&o.stderr).trim()),
                Err(e) => self.alert("Install failed", &e.to_string()),
            }
        }

        #[unsafe(method(recovery:))]
        fn recovery(&self, _sender: Option<&AnyObject>) {
            let _ = std::process::Command::new("/usr/bin/open")
                .arg("https://github.com/rickhallett/tto/blob/main/docs/RECOVERY.md")
                .status();
        }

        #[unsafe(method(quitSelf:))]
        fn quit_self(&self, _sender: Option<&AnyObject>) {
            NSApplication::sharedApplication(self.mtm()).terminate(None);
        }
    }
);

impl Controller {
    fn new(mtm: MainThreadMarker, paths: Paths) -> Retained<Self> {
        let blocklist = Blocklist::embedded();
        let selected = blocklist.names().iter().map(|s| s.to_string()).collect();
        let ivars = Ivars {
            paths,
            status_item: RefCell::new(None),
            selected: RefCell::new(selected),
            blocklist,
            last: RefCell::new(None),
            installed: Cell::new(false),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        // SAFETY: NSObject's init takes no arguments and returns an instance.
        unsafe { msg_send![super(this), init] }
    }

    fn poll(&self) {
        self.refresh();
        self.update_button();
    }

    fn refresh(&self) {
        match proto::call(&self.ivars().paths.socket(), &Request::Status) {
            Ok(resp) => {
                self.ivars().installed.set(true);
                *self.ivars().last.borrow_mut() = Some(resp.state);
            }
            Err(ClientError::NotInstalled) => {
                self.ivars().installed.set(false);
                *self.ivars().last.borrow_mut() = None;
            }
            Err(_) => {}
        }
    }

    fn update_button(&self) {
        let Some(item) = self.ivars().status_item.borrow().clone() else {
            return;
        };
        let Some(button) = item.button(self.mtm()) else {
            return;
        };
        let title = match &*self.ivars().last.borrow() {
            Some(s) if s.active() => format!(" {}", when::remaining(s.remaining())),
            _ => String::new(),
        };
        button.setTitle(&NSString::from_str(&title));
    }

    fn item(
        &self,
        title: &str,
        action: Option<objc2::runtime::Sel>,
        key: &str,
    ) -> Retained<NSMenuItem> {
        // SAFETY: the selector, when given, names a method defined on Controller above.
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(self.mtm()),
                &NSString::from_str(title),
                action,
                &NSString::from_str(key),
            )
        };
        if action.is_some() {
            // SAFETY: self outlives the menu (both live until the app exits).
            unsafe { item.setTarget(Some(self)) };
        }
        item
    }

    fn rebuild(&self, menu: &NSMenu) {
        let mtm = self.mtm();
        menu.removeAllItems();
        let last = self.ivars().last.borrow().clone();

        if !self.ivars().installed.get() {
            let h = self.item("Turn Them Off isn't set up yet", None, "");
            h.setEnabled(false);
            menu.addItem(&h);
            menu.addItem(&self.item(
                "Install helper… (asks for your password, once)",
                Some(sel!(install:)),
                "",
            ));
        } else if let Some(s) = last.filter(State::active) {
            let h = self.item(&format!("Off until {}", when::describe(s.until)), None, "");
            h.setEnabled(false);
            menu.addItem(&h);
            let sub = self.item(
                &format!(
                    "{} to go · {}",
                    when::remaining(s.remaining()),
                    s.categories.join(", ")
                ),
                None,
                "",
            );
            sub.setEnabled(false);
            menu.addItem(&sub);
            menu.addItem(&NSMenuItem::separatorItem(mtm));
            let h2 = self.item("Make it longer", None, "");
            h2.setEnabled(false);
            menu.addItem(&h2);
            self.add_presets(menu);
        } else {
            let h = self.item("Turn them off…", None, "");
            h.setEnabled(false);
            menu.addItem(&h);
            self.add_presets(menu);
            menu.addItem(&NSMenuItem::separatorItem(mtm));
            let what = self.item("What to turn off", None, "");
            let sub = NSMenu::new(mtm);
            for (i, (name, cat)) in self.ivars().blocklist.categories.iter().enumerate() {
                let it = self.item(&cat.title, Some(sel!(toggleCategory:)), "");
                it.setTag(i as isize);
                it.setToolTip(Some(&NSString::from_str(&cat.blurb)));
                let on = self.ivars().selected.borrow().iter().any(|s| s == name);
                it.setState(if on {
                    NSControlStateValueOn
                } else {
                    NSControlStateValueOff
                });
                sub.addItem(&it);
            }
            what.setSubmenu(Some(&sub));
            menu.addItem(&what);
        }

        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&self.item("How recovery works…", Some(sel!(recovery:)), ""));
        menu.addItem(&self.item(
            "Quit this menu (the block stays on)",
            Some(sel!(quitSelf:)),
            "q",
        ));
    }

    fn add_presets(&self, menu: &NSMenu) {
        for (i, (label, _)) in PRESETS.iter().enumerate() {
            let it = self.item(label, Some(sel!(preset:)), "");
            it.setTag(i as isize);
            menu.addItem(&it);
        }
    }

    fn confirm(&self, msg: &str, info: &str, button: &str) -> bool {
        let alert = NSAlert::new(self.mtm());
        alert.setMessageText(&NSString::from_str(msg));
        alert.setInformativeText(&NSString::from_str(info));
        alert.addButtonWithTitle(&NSString::from_str(button));
        alert.addButtonWithTitle(ns_string!("Not now"));
        NSApplication::sharedApplication(self.mtm()).activate();
        // NSAlertFirstButtonReturn == 1000
        alert.runModal() == 1000
    }

    fn alert(&self, msg: &str, info: &str) {
        let alert = NSAlert::new(self.mtm());
        alert.setMessageText(&NSString::from_str(msg));
        alert.setInformativeText(&NSString::from_str(info));
        NSApplication::sharedApplication(self.mtm()).activate();
        alert.runModal();
    }
}

/// The stick man and his plug. Vector, template, crisp at any scale.
fn menubar_icon() -> Option<Retained<NSImage>> {
    const SVG: &[u8] = include_bytes!("../assets/menubar.svg");
    let img = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(SVG))?;
    img.setSize(NSSize::new(18.0, 18.0));
    img.setTemplate(true);
    Some(img)
}

pub fn run(paths: Paths) {
    let mtm = MainThreadMarker::new().expect("tto menu must start on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let controller = Controller::new(mtm, paths);
    let status_item =
        NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
    if let Some(button) = status_item.button(mtm) {
        match menubar_icon() {
            Some(img) => button.setImage(Some(&img)),
            None => button.setTitle(ns_string!("⏻")),
        }
        button.setToolTip(Some(ns_string!("Turn Them Off")));
    }
    let menu = NSMenu::new(mtm);
    menu.setDelegate(Some(ProtocolObject::from_ref(&*controller)));
    status_item.setMenu(Some(&menu));
    *controller.ivars().status_item.borrow_mut() = Some(status_item);
    controller.poll();

    // SAFETY: the selector names a method defined on Controller; the run
    // loop retains the timer, and the controller lives for the app's life.
    let _timer = unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            30.0,
            &controller,
            sel!(tick:),
            None,
            true,
        )
    };
    app.run();
}

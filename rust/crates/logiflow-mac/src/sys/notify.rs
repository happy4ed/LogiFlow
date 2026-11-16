//! 메뉴(설정 창의 채널 메뉴 · 「⋯」 메뉴)와 알림. Windows `notify` 와 같은 모양 — `popup_menu` 는 고를 때까지
//! 기다렸다가 고른 번호(없으면 0)를 돌려준다. 알림은 macOS 알림 센터(osascript)로.

use super::dialog::{localize, HWND};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSMenu, NSMenuItem};
use objc2_foundation::{NSPoint, NSString};
use std::cell::Cell;

pub enum MenuItem {
    Item { id: u32, text: String, enabled: bool },
    Separator,
}

pub struct PickIvars {
    picked: Cell<isize>,
}

define_class!(
    // SAFETY: NSObject 는 하위 클래스 조건이 없고, 이 클래스는 Drop 을 구현하지 않는다.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "LogiFlowRsMenuPick"]
    #[ivars = PickIvars]
    pub struct MenuPick;

    impl MenuPick {
        #[unsafe(method(pick:))]
        fn pick(&self, sender: &NSMenuItem) {
            self.ivars().picked.set(sender.tag());
        }
    }
);

impl MenuPick {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(PickIvars { picked: Cell::new(0) });
        unsafe { msg_send![super(this), init] }
    }
    pub fn picked(&self) -> isize {
        self.ivars().picked.get()
    }
}

/// 메뉴 항목 하나(대상·동작 연결). text 는 이 OS 말로 바꿔 넣는다.
pub fn menu_item(mtm: MainThreadMarker, text: &str, tag: isize, enabled: bool, target: Option<&AnyObject>) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(&localize(text)),
            if enabled && target.is_some() { Some(sel!(pick:)) } else { None },
            &NSString::from_str(""),
        )
    };
    item.setTag(tag);
    item.setEnabled(enabled);
    if enabled {
        unsafe { item.setTarget(target) };
    }
    item
}

/// 화면 좌표(Cocoa, 주 화면 왼쪽 아래 기준 · `dialog::to_screen` 결과)에 메뉴를 띄우고 고른 번호를 돌려준다.
pub fn popup_menu(_owner: HWND, items: &[MenuItem], x: i32, y: i32, _bottom_align: bool) -> u32 {
    let Some(mtm) = MainThreadMarker::new() else {
        return 0;
    };
    let pick = MenuPick::new(mtm);
    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);
    for it in items {
        match it {
            MenuItem::Separator => menu.addItem(&NSMenuItem::separatorItem(mtm)),
            MenuItem::Item { id, text, enabled } => {
                let target: &AnyObject = &pick;
                menu.addItem(&menu_item(mtm, text, *id as isize, *enabled, Some(target)));
            }
        }
    }
    // 위치 = 메뉴 왼쪽 위. view 없이 화면 좌표.
    let _ = menu.popUpMenuPositioningItem_atLocation_inView(None, NSPoint::new(x as f64, y as f64), None);
    pick.picked().max(0) as u32
}

/// 알림 센터에 한 줄(클립보드 받음 · 암호문구 다름 등). 기다리지 않는다.
pub fn balloon(title: &str, text: &str) {
    let esc = |s: &str| localize(s).replace('\\', "\\\\").replace('"', "\\\"").replace("\r\n", " ").replace('\n', " ");
    let script = format!("display notification \"{}\" with title \"{}\"", esc(text), esc(title));
    let _ = std::process::Command::new("/usr/bin/osascript").arg("-e").arg(script).spawn().map(|mut c| {
        std::thread::spawn(move || {
            let _ = c.wait();
        })
    });
}

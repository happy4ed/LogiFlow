//! 값 바꾸기 입력 상자 — Windows `prompt` 와 같은 모양(제목 · 설명 · 처음 값 → 확인이면 Some).
//! 가린 입력(암호문구)은 ● 로 가리고, 옆 눈 아이콘으로 보기/숨기기(아이콘만, 이름은 도움말 풍선).

use super::dialog::{localize, HWND};
use logiflow_core::theme::Rgb;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSAlertStyle, NSApplication, NSButton, NSImage, NSSecureTextField, NSTextField, NSView};
use objc2_foundation::{NSObject, NSPoint, NSRect, NSSize, NSString};
use std::cell::Cell;

const FIELD_W: f64 = 300.0;
const FIELD_H: f64 = 24.0;
const EYE_W: f64 = 28.0;

pub struct EyeIvars {
    secure: Retained<NSSecureTextField>,
    plain: Retained<NSTextField>,
    shown: Cell<bool>,
}

define_class!(
    // SAFETY: NSObject 는 하위 클래스 조건이 없고, 이 클래스는 Drop 을 구현하지 않는다.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "LogiFlowRsEyeToggle"]
    #[ivars = EyeIvars]
    struct EyeToggle;

    impl EyeToggle {
        #[unsafe(method(toggle:))]
        fn toggle(&self, sender: &NSButton) {
            let iv = self.ivars();
            let show = !iv.shown.get();
            iv.shown.set(show);
            if show {
                iv.plain.setStringValue(&iv.secure.stringValue());
            } else {
                iv.secure.setStringValue(&iv.plain.stringValue());
            }
            iv.plain.setHidden(!show);
            iv.secure.setHidden(show);
            set_eye(sender, show);
        }
    }
);

fn set_eye(b: &NSButton, shown: bool) {
    let (sym, tip) = if shown { ("eye.slash", "숨기기") } else { ("eye", "보기") };
    let img = NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(sym), Some(&NSString::from_str(tip)));
    b.setImage(img.as_deref());
    b.setToolTip(Some(&NSString::from_str(tip)));
}

fn field_rect(w: f64) -> NSRect {
    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(w, FIELD_H))
}

pub fn show(owner: HWND, title: &str, label: &str, initial: &str, bg: Rgb, fg: Rgb) -> Option<String> {
    show_inner(owner, title, label, initial, bg, fg, false)
}

pub fn show_secret(owner: HWND, title: &str, label: &str, initial: &str, bg: Rgb, fg: Rgb) -> Option<String> {
    show_inner(owner, title, label, initial, bg, fg, true)
}

fn show_inner(_owner: HWND, title: &str, label: &str, initial: &str, _bg: Rgb, _fg: Rgb, secret: bool) -> Option<String> {
    let mtm = MainThreadMarker::new()?;
    #[allow(deprecated)]
    NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(&localize(title)));
    alert.setInformativeText(&NSString::from_str(&localize(label).replace("\r\n", "\n")));
    alert.setAlertStyle(NSAlertStyle::Informational);
    alert.addButtonWithTitle(&NSString::from_str("확인"));
    alert.addButtonWithTitle(&NSString::from_str("취소"));
    let init = NSString::from_str(initial);

    if !secret {
        let field = NSTextField::initWithFrame(NSTextField::alloc(mtm), field_rect(FIELD_W));
        field.setStringValue(&init);
        alert.setAccessoryView(Some(&field));
        alert.window().setInitialFirstResponder(Some(&field));
        let ok = alert.runModal() == NSAlertFirstButtonReturn;
        return ok.then(|| field.stringValue().to_string());
    }

    let fw = FIELD_W - EYE_W - 4.0;
    let holder = NSView::initWithFrame(NSView::alloc(mtm), field_rect(FIELD_W));
    let secure = NSSecureTextField::initWithFrame(NSSecureTextField::alloc(mtm), field_rect(fw));
    let plain = NSTextField::initWithFrame(NSTextField::alloc(mtm), field_rect(fw));
    secure.setStringValue(&init);
    plain.setStringValue(&init);
    plain.setHidden(true);
    let eye = NSButton::initWithFrame(
        NSButton::alloc(mtm),
        NSRect::new(NSPoint::new(FIELD_W - EYE_W, 0.0), NSSize::new(EYE_W, FIELD_H)),
    );
    eye.setBordered(false);
    set_eye(&eye, false);
    let toggler = EyeToggle::alloc(mtm).set_ivars(EyeIvars {
        secure: secure.clone(),
        plain: plain.clone(),
        shown: Cell::new(false),
    });
    let toggler: Retained<EyeToggle> = unsafe { msg_send![super(toggler), init] };
    unsafe {
        let t: &AnyObject = &toggler;
        eye.setTarget(Some(t));
        eye.setAction(Some(sel!(toggle:)));
    }
    holder.addSubview(&secure);
    holder.addSubview(&plain);
    holder.addSubview(&eye);
    alert.setAccessoryView(Some(&holder));
    alert.window().setInitialFirstResponder(Some(&secure));
    let ok = alert.runModal() == NSAlertFirstButtonReturn;
    let value = if toggler.ivars().shown.get() {
        plain.stringValue()
    } else {
        secure.stringValue()
    };
    ok.then(|| value.to_string())
}

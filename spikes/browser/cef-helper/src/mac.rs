//! macOS: the application object CEF requires.
//!
//! CEF needs `NSApp` to be an `NSApplication` subclass that implements `CefAppProtocol`
//! (`isHandlingSendEvent` / `setHandlingSendEvent:`). GPUI registers its own `GPUIApplication`
//! without those methods, which is why CEF does not run inside the app's process. This helper
//! owns its process, so it installs its own subclass before CEF starts. It never shows a window,
//! a Dock icon or a menu bar.

use cef::application_mac::{CefAppProtocol, CrAppControlProtocol, CrAppProtocol};
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObjectProtocol as _};
use objc2::{ClassType, DefinedClass, MainThreadMarker, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSEvent};
use std::cell::Cell;

#[derive(Default)]
pub struct HelperApplicationIvars {
    handling_send_event: Cell<Bool>,
}

define_class!(
    #[unsafe(super(NSApplication))]
    #[ivars = HelperApplicationIvars]
    pub struct HelperApplication;

    impl HelperApplication {
        #[unsafe(method(sendEvent:))]
        unsafe fn send_event(&self, event: &NSEvent) {
            let was = self.ivars().handling_send_event.get();
            self.ivars().handling_send_event.set(Bool::YES);
            let _: () = unsafe { msg_send![super(self), sendEvent: event] };
            self.ivars().handling_send_event.set(was);
        }
    }

    unsafe impl CrAppProtocol for HelperApplication {
        #[unsafe(method(isHandlingSendEvent))]
        unsafe fn is_handling_send_event_impl(&self) -> Bool {
            self.ivars().handling_send_event.get()
        }
    }

    unsafe impl CrAppControlProtocol for HelperApplication {
        #[unsafe(method(setHandlingSendEvent:))]
        unsafe fn set_handling_send_event_impl(&self, value: Bool) {
            self.ivars().handling_send_event.set(value);
        }
    }

    unsafe impl CefAppProtocol for HelperApplication {}
);

/// Makes `NSApp` a `HelperApplication`. Must run on the main thread before anything touches
/// `NSApp` and before CEF initializes.
pub fn install_application() {
    let mtm = MainThreadMarker::new().expect("the CEF helper starts on the main thread");
    let app: Retained<HelperApplication> =
        unsafe { msg_send![HelperApplication::class(), sharedApplication] };
    let app: &NSApplication = &app;
    assert!(
        app.isKindOfClass(HelperApplication::class()),
        "NSApp existed before the helper installed its own application object"
    );
    // Off-screen only: no Dock icon, no menu bar, never activates.
    app.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
    let _ = mtm;
}

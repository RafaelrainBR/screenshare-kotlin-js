use block2::RcBlock;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{NSWorkspace, NSWorkspaceDidWakeNotification, NSWorkspaceWillSleepNotification};
use objc2_foundation::{ns_string, NSDistributedNotificationCenter, NSNotification, NSNotificationCenter, NSObjectProtocol};
use std::{cell::RefCell, ptr::NonNull, sync::{Arc, atomic::{AtomicBool, Ordering}}};
use tauri::Emitter;

thread_local! {
    // Keep the four native registrations alive for the app's main-thread lifetime.
    static OBSERVERS: RefCell<Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>> = const { RefCell::new(Vec::new()) };
}

pub fn install(app: &tauri::AppHandle) {
    let locked = Arc::new(AtomicBool::new(false));
    let sleeping = Arc::new(AtomicBool::new(false));
    let workspace = NSWorkspace::sharedWorkspace().notificationCenter();
    let distributed = NSDistributedNotificationCenter::defaultCenter();
    // loginwindow distributes screen lock/unlock; workspace handles machine sleep/wake.
    // Actual OS delivery remains an installed-build gate, separate from observer registration.
    let notifications: [(&NSNotificationCenter, _, _, bool); 4] = unsafe { [
        (&distributed, ns_string!("com.apple.screenIsLocked"), locked.clone(), true),
        (&distributed, ns_string!("com.apple.screenIsUnlocked"), locked.clone(), false),
        (&workspace, NSWorkspaceWillSleepNotification, sleeping.clone(), true),
        (&workspace, NSWorkspaceDidWakeNotification, sleeping.clone(), false),
    ] };
    for (center, name, state, value) in notifications {
        let app = app.clone();
        let locked = locked.clone();
        let sleeping = sleeping.clone();
        let callback = RcBlock::new(move |_: NonNull<NSNotification>| {
            state.store(value, Ordering::SeqCst);
            let available = !locked.load(Ordering::SeqCst) && !sleeping.load(Ordering::SeqCst);
            let _ = app.emit("session-media-available", available);
        });
        // The block owns its captured state; no borrowed notification escapes the callback.
        let observer = unsafe { center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &callback) };
        OBSERVERS.with(|observers| observers.borrow_mut().push(observer));
    }
}

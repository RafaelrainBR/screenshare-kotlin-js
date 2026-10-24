//! Windows lowers every other sound by 80% while a communications stream is open ("Comunicações"
//! in the Sound control panel), and libwebrtc opens the microphone and speakers as one. A call app
//! opts out per audio session (Disabling the Default Ducking Experience, IAudioSessionControl2::
//! SetDuckingPreference), so the process's default session is opted out on every endpoint before
//! the engine opens its devices.
use std::sync::Once;

use windows::Win32::{
    Media::Audio::{
        DEVICE_STATE_ACTIVE, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, MMDeviceEnumerator,
        eAll,
    },
    System::Com::{CLSCTX_ALL, CoCreateInstance, CoIncrementMTAUsage},
};
use windows::core::Interface;

pub fn opt_out() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // Its own thread: the caller may be an STA, and the sessions must outlive it.
        let opted = std::thread::spawn(|| unsafe { opt_out_sessions() }).join();
        if let Ok(Err(error)) = opted {
            eprintln!("ducking: opt-out failed ({error})");
        }
    });
}

/// SAFETY: plain COM calls on a thread in the MTA, which CoIncrementMTAUsage keeps for the process.
unsafe fn opt_out_sessions() -> windows::core::Result<()> {
    unsafe {
        CoIncrementMTAUsage()?;
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let devices = enumerator.EnumAudioEndpoints(eAll, DEVICE_STATE_ACTIVE)?;
        for index in 0..devices.GetCount()? {
            let Ok(device) = devices.Item(index) else { continue };
            let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { continue };
            // The default session (no GUID), the one libwebrtc's streams join.
            let Ok(session) = manager.GetAudioSessionControl(None, 0).and_then(|control| control.cast::<IAudioSessionControl2>())
            else {
                continue;
            };
            if session.SetDuckingPreference(true).is_ok() {
                // The preference lives with the session; kept for the whole process.
                std::mem::forget(session);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn opts_out_of_ducking() {
        // Fails only on a COM error; a machine with no audio endpoint has nothing to opt out.
        unsafe { super::opt_out_sessions() }.unwrap();
    }
}

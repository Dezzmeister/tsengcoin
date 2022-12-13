use std::{
    ffi::CString,
    sync::mpsc::{channel, RecvError},
};

use fltk::{app::awake_callback, utils::FlString};

/// Creates a dialog box with a positive and negative choice. `true` is returned if the positive choice is taken,
/// `false` otherwise.
pub fn dialog2(text: &str, pos_btn: &str, neg_btn: &str) -> bool {
    unsafe {
        let txt_c = CString::safe_new(text);
        let pos_btn_c = CString::safe_new(pos_btn);
        let neg_btn_c = CString::safe_new(neg_btn);
        let ret = fltk_sys::dialog::Fl_choice2_n(
            txt_c.as_ptr(),
            std::ptr::null(),
            neg_btn_c.as_ptr(),
            pos_btn_c.as_ptr(),
        );

        ret == 2
    }
}

/// Runs a function on the GUI thread (the main thread) and returns the result or an error. FLTK
/// requires windows to be created, hidden, and updated on the main thread. The native [awake_callback]
/// does not accept functions with a return value, so this function wraps it and uses a channel to get the return value out.
/// This is useful for things like showing a dialog box and getting the user's choice back, or creating a window
/// and getting a handle to it on a worker thread.
///
/// If you don't actually need anything returned, it's better to just use [awake_callback] directly.
pub fn do_on_gui_thread<F, R>(mut cb: F) -> Result<R, RecvError>
where
    F: (FnMut() -> R) + 'static,
    R: Send + Sync + 'static,
{
    // Make a sender and receiver and give the sender to the GUI thread
    // We will use the receiver to get the value sent by the GUI thread
    let (sender, receiver) = channel::<R>();
    awake_callback(move || {
        let res = cb();
        match sender.send(res) {
            Ok(_) => (),
            Err(_) => println!("Error in GUI callback"),
        }
    });

    receiver.recv()
}

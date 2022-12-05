use std::ffi::CString;

use fltk::utils::FlString;

/// Creates a dialog box with a positive and negative choice. `true` is returned if the positive choice is taken,
/// `false` otherwise.
pub fn dialog2(text: &str, pos_btn: &str, neg_btn: &str) -> bool {
    unsafe {
        let txt_c = CString::safe_new(text);
        let pos_btn_c = CString::safe_new(pos_btn);
        let neg_btn_c = CString::safe_new(neg_btn);
        let ret = fltk_sys::dialog::Fl_choice2_n(txt_c.as_ptr(), std::ptr::null(), neg_btn_c.as_ptr(), pos_btn_c.as_ptr());

        ret == 2
    }
}

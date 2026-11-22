//! Exposes a local clip as a Windows file drop, with copy-only semantics.

use std::path::Path;

pub fn started(response: &eframe::egui::Response) -> bool {
    cfg!(windows) && response.drag_started_by(eframe::egui::PointerButton::Primary)
}

/// Puts the file on the Windows clipboard as CF_HDROP, so it can be pasted in
/// Explorer or attached in another app. Returns `false` when that failed.
pub fn copy_file_to_clipboard(path: &Path) -> bool {
    #[cfg(windows)]
    {
        imp::copy_to_clipboard_windows(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

pub fn drag_file_out(path: &Path) {
    #[cfg(windows)]
    imp::drag_file_out_windows(path);
    #[cfg(not(windows))]
    let _ = path;
}
#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;

    use windows::core::{implement, Error, Result, BOOL, HRESULT};
    use windows::Win32::Foundation::{
        GlobalFree, DATA_S_SAMEFORMATETC, DRAGDROP_S_CANCEL, DRAGDROP_S_DROP,
        DRAGDROP_S_USEDEFAULTCURSORS, DV_E_FORMATETC, DV_E_TYMED, E_INVALIDARG, E_NOTIMPL,
        E_OUTOFMEMORY, HGLOBAL, LPARAM, S_OK, WPARAM,
    };
    use windows::Win32::System::Com::{
        IAdviseSink, IDataObject, IDataObject_Impl, IEnumFORMATETC, IEnumSTATDATA, DATADIR_GET,
        DVASPECT_CONTENT, FORMATETC, STGMEDIUM, TYMED_HGLOBAL,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE, GMEM_ZEROINIT,
    };
    use windows::Win32::System::Ole::{
        DoDragDrop, IDropSource, IDropSource_Impl, OleFlushClipboard, OleInitialize,
        OleSetClipboard, OleUninitialize, CF_HDROP, DROPEFFECT, DROPEFFECT_COPY,
    };
    use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetActiveWindow, GetAsyncKeyState, GetCapture, ReleaseCapture, VK_LBUTTON,
    };
    use windows::Win32::UI::Shell::SHCreateStdEnumFmtEtc;
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_LBUTTONUP};

    fn hdrop_format() -> FORMATETC {
        FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        }
    }

    fn supports(format: &FORMATETC) -> Result<()> {
        if format.cfFormat != CF_HDROP.0 || format.lindex != -1 {
            return Err(Error::from_hresult(DV_E_FORMATETC));
        }
        if format.dwAspect & DVASPECT_CONTENT.0 == 0 {
            return Err(Error::from_hresult(DV_E_FORMATETC));
        }
        if format.tymed & TYMED_HGLOBAL.0 as u32 == 0 {
            return Err(Error::from_hresult(DV_E_TYMED));
        }
        Ok(())
    }

    /// Builds a fresh `CF_HDROP` global-memory block for `path`.
    /// Ownership goes to the drop target (it `GlobalFree`s the `STGMEDIUM`
    /// it receives from `GetData`), so every call allocates a new one.
    fn make_hdrop(path: &Path) -> Option<HGLOBAL> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        // DROPFILES header is 20 bytes: pFiles=20, point=(0,0), fNC=0, fWide=1.
        let mut words: Vec<u16> = vec![20, 0, 0, 0, 0, 0, 0, 0, 1, 0];
        words.extend_from_slice(&wide);
        words.push(0);
        words.push(0);
        let bytes = words.len() * 2;
        unsafe {
            let handle = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes).ok()?;
            let ptr = GlobalLock(handle);
            if ptr.is_null() {
                let _ = GlobalFree(Some(handle));
                return None;
            }
            std::ptr::copy_nonoverlapping(words.as_ptr() as *const u8, ptr as *mut u8, bytes);
            let _ = GlobalUnlock(handle);
            Some(handle)
        }
    }

    #[implement(IDataObject)]
    struct FileDataObject {
        path: PathBuf,
    }

    impl IDataObject_Impl for FileDataObject_Impl {
        fn GetData(&self, pformatetcin: *const FORMATETC) -> Result<STGMEDIUM> {
            unsafe {
                if pformatetcin.is_null() {
                    return Err(Error::from_hresult(E_INVALIDARG));
                }
                supports(&*pformatetcin)?;
                let hglobal =
                    make_hdrop(&self.path).ok_or_else(|| Error::from_hresult(E_OUTOFMEMORY))?;
                let mut medium = STGMEDIUM::default();
                medium.tymed = TYMED_HGLOBAL.0 as u32;
                medium.u.hGlobal = hglobal;
                Ok(medium)
            }
        }

        fn GetDataHere(
            &self,
            _pformatetc: *const FORMATETC,
            _pmedium: *mut STGMEDIUM,
        ) -> Result<()> {
            Err(Error::from_hresult(E_NOTIMPL))
        }

        fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
            unsafe {
                if pformatetc.is_null() {
                    return E_INVALIDARG;
                }
                match supports(&*pformatetc) {
                    Ok(()) => S_OK,
                    Err(error) => error.code(),
                }
            }
        }

        fn GetCanonicalFormatEtc(
            &self,
            pformatectin: *const FORMATETC,
            pformatetcout: *mut FORMATETC,
        ) -> HRESULT {
            unsafe {
                if pformatectin.is_null() || pformatetcout.is_null() {
                    return E_INVALIDARG;
                }
                *pformatetcout = *pformatectin;
                (*pformatetcout).ptd = std::ptr::null_mut();
                DATA_S_SAMEFORMATETC
            }
        }

        fn SetData(
            &self,
            _pformatetc: *const FORMATETC,
            _pmedium: *const STGMEDIUM,
            _frelease: BOOL,
        ) -> Result<()> {
            Err(Error::from_hresult(E_NOTIMPL))
        }

        fn EnumFormatEtc(&self, dwdirection: u32) -> Result<IEnumFORMATETC> {
            if dwdirection == DATADIR_GET.0 as u32 {
                unsafe { SHCreateStdEnumFmtEtc(&[hdrop_format()]) }
            } else {
                Err(Error::from_hresult(E_NOTIMPL))
            }
        }
        fn DAdvise(
            &self,
            _pformatetc: *const FORMATETC,
            _advf: u32,
            _padvsink: windows::core::Ref<IAdviseSink>,
        ) -> Result<u32> {
            Err(Error::from_hresult(E_NOTIMPL))
        }

        fn DUnadvise(&self, _dwconnection: u32) -> Result<()> {
            Err(Error::from_hresult(E_NOTIMPL))
        }

        fn EnumDAdvise(&self) -> Result<IEnumSTATDATA> {
            Err(Error::from_hresult(E_NOTIMPL))
        }
    }

    #[implement(IDropSource)]
    struct FileDropSource;

    impl IDropSource_Impl for FileDropSource_Impl {
        fn QueryContinueDrag(&self, escape: BOOL, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
            if escape.as_bool() {
                DRAGDROP_S_CANCEL
            } else if keys.contains(MK_LBUTTON) {
                S_OK
            } else {
                DRAGDROP_S_DROP
            }
        }

        fn GiveFeedback(&self, _effect: DROPEFFECT) -> HRESULT {
            DRAGDROP_S_USEDEFAULTCURSORS
        }
    }

    pub(super) fn copy_to_clipboard_windows(path: &Path) -> bool {
        let Ok(path) = std::path::absolute(path) else {
            return false;
        };
        if !path.is_file() {
            return false;
        }
        unsafe {
            // The window may already have OLE initialised in another apartment
            // mode; the clipboard still works, so only the balancing call matters.
            let initialised = OleInitialize(None).is_ok();
            let data: IDataObject = FileDataObject { path }.into();
            let copied = OleSetClipboard(&data).is_ok();
            // Flush renders the data now, so it survives the OLE shutdown below.
            let flushed = copied && OleFlushClipboard().is_ok();
            if initialised {
                OleUninitialize();
            }
            copied && flushed
        }
    }

    pub(super) fn drag_file_out_windows(path: &Path) {
        let Ok(path) = std::path::absolute(path) else {
            return;
        };
        if !path.is_file() {
            return;
        }
        unsafe {
            if GetAsyncKeyState(VK_LBUTTON.0 as i32) >= 0 || OleInitialize(None).is_err() {
                return;
            }
            {
                let data: IDataObject = FileDataObject { path }.into();
                let source: IDropSource = FileDropSource.into();
                let mut effect = DROPEFFECT::default();
                let capture = GetCapture();
                let window = if capture.is_invalid() {
                    GetActiveWindow()
                } else {
                    capture
                };
                if !capture.is_invalid() {
                    let _ = ReleaseCapture();
                }
                let _ = DoDragDrop(&data, &source, DROPEFFECT_COPY, &mut effect);
                // OLE consumes the release. Send it through the window's normal
                // input pipeline so egui can finish its original drag gesture.
                if !window.is_invalid() {
                    let _ = PostMessageW(Some(window), WM_LBUTTONUP, WPARAM(0), LPARAM(0));
                }
            }
            OleUninitialize();
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use windows::Win32::System::Ole::ReleaseStgMedium;
        use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

        #[test]
        fn windows_receives_the_clip_path_on_button_release() {
            let path = PathBuf::from(r"D:\Klipy\Mój zapis.mp4");
            let data: IDataObject = FileDataObject { path: path.clone() }.into();
            let source: IDropSource = FileDropSource.into();
            let mut buffer = [0_u16; 256];
            let length = unsafe {
                let mut medium = data.GetData(&hdrop_format()).unwrap();
                let drop = HDROP(medium.u.hGlobal.0);
                let length = DragQueryFileW(drop, 0, Some(&mut buffer));
                ReleaseStgMedium(&mut medium);
                assert_eq!(source.QueryContinueDrag(false, MK_LBUTTON), S_OK);
                assert_eq!(
                    source.QueryContinueDrag(false, MODIFIERKEYS_FLAGS(0)),
                    DRAGDROP_S_DROP
                );
                assert_eq!(
                    source.QueryContinueDrag(true, MK_LBUTTON),
                    DRAGDROP_S_CANCEL
                );
                length
            };
            assert_eq!(
                String::from_utf16(&buffer[..length as usize]).unwrap(),
                path.to_str().unwrap()
            );
        }
    }
}

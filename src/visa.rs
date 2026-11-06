//! Minimal VISA binding, loaded at run time from the system VISA library (`visa64.dll` on 64-bit
//! Windows: the IVI shared-components router, which dispatches to whichever VISA is installed).
//!
//! Only what a USBTMC instrument session needs: find, open, write, read, clear, timeout.

use std::ffi::{CStr, CString, c_char};
use std::sync::Arc;

use libloading::Library;

type ViStatus = i32;
type ViSession = u32;
type ViObject = u32;
type ViAttr = u32;
#[cfg(target_pointer_width = "64")]
type ViAttrState = u64;
#[cfg(not(target_pointer_width = "64"))]
type ViAttrState = u32;

const VI_SUCCESS: ViStatus = 0;
const VI_SUCCESS_MAX_CNT: ViStatus = 0x3FFF_0006;
const VI_ERROR_TMO: ViStatus = 0xBFFF_0015_u32 as i32;
const VI_ERROR_RSRC_NFOUND: ViStatus = 0xBFFF_0011_u32 as i32;
const VI_ATTR_TMO_VALUE: ViAttr = 0x3FFF_001A;
const VI_NULL: u32 = 0;
const FIND_BUFLEN: usize = 256;

#[cfg(windows)]
const LIBRARY: &str = "visa64.dll";
#[cfg(not(windows))]
const LIBRARY: &str = "libvisa.so";

#[derive(Debug, thiserror::Error)]
pub enum VisaError {
    #[error("VISA library not available ({0})")]
    Library(String),
    #[error("timeout")]
    Timeout,
    #[error("resource not found")]
    NotFound,
    #[error("VISA error {code:#010x}: {desc}")]
    Status { code: u32, desc: String },
}

pub type Result<T> = std::result::Result<T, VisaError>;

struct Api {
    _lib: Library,
    open_default_rm: unsafe extern "system" fn(*mut ViSession) -> ViStatus,
    find_rsrc: unsafe extern "system" fn(ViSession, *const c_char, *mut u32, *mut u32, *mut c_char) -> ViStatus,
    find_next: unsafe extern "system" fn(u32, *mut c_char) -> ViStatus,
    open: unsafe extern "system" fn(ViSession, *const c_char, u32, u32, *mut ViSession) -> ViStatus,
    close: unsafe extern "system" fn(ViObject) -> ViStatus,
    read: unsafe extern "system" fn(ViSession, *mut u8, u32, *mut u32) -> ViStatus,
    write: unsafe extern "system" fn(ViSession, *const u8, u32, *mut u32) -> ViStatus,
    clear: unsafe extern "system" fn(ViSession) -> ViStatus,
    set_attribute: unsafe extern "system" fn(ViObject, ViAttr, ViAttrState) -> ViStatus,
    status_desc: unsafe extern "system" fn(ViObject, ViStatus, *mut c_char) -> ViStatus,
}

impl Api {
    fn load() -> Result<Self> {
        // SAFETY: loading the vendor VISA library runs its initialisers; that is what every VISA
        // client does. The symbol types below follow the IVI VISA specification (visa.h).
        unsafe {
            let lib = Library::new(LIBRARY).map_err(|e| VisaError::Library(format!("{LIBRARY}: {e}")))?;
            macro_rules! sym {
                ($name:literal) => {
                    *lib.get(concat!($name, "\0").as_bytes())
                        .map_err(|e| VisaError::Library(format!("{}: {e}", $name)))?
                };
            }
            Ok(Self {
                open_default_rm: sym!("viOpenDefaultRM"),
                find_rsrc: sym!("viFindRsrc"),
                find_next: sym!("viFindNext"),
                open: sym!("viOpen"),
                close: sym!("viClose"),
                read: sym!("viRead"),
                write: sym!("viWrite"),
                clear: sym!("viClear"),
                set_attribute: sym!("viSetAttribute"),
                status_desc: sym!("viStatusDesc"),
                _lib: lib,
            })
        }
    }
}

/// The default resource manager. Cheap to clone; sessions keep it alive.
#[derive(Clone)]
pub struct ResourceManager {
    inner: Arc<Rm>,
}

struct Rm {
    api: Api,
    session: ViSession,
}

impl Drop for Rm {
    fn drop(&mut self) {
        // SAFETY: session was opened by viOpenDefaultRM and is closed exactly once.
        unsafe { (self.api.close)(self.session) };
    }
}

impl ResourceManager {
    pub fn new() -> Result<Self> {
        let api = Api::load()?;
        let mut session = 0;
        // SAFETY: valid out-pointer.
        let st = unsafe { (api.open_default_rm)(&mut session) };
        if st < VI_SUCCESS {
            return Err(VisaError::Status { code: st as u32, desc: "viOpenDefaultRM failed".into() });
        }
        Ok(Self { inner: Arc::new(Rm { api, session }) })
    }

    fn check(&self, obj: ViObject, st: ViStatus) -> Result<ViStatus> {
        match st {
            s if s >= VI_SUCCESS => Ok(s),
            VI_ERROR_TMO => Err(VisaError::Timeout),
            VI_ERROR_RSRC_NFOUND => Err(VisaError::NotFound),
            s => {
                let mut buf = [0 as c_char; FIND_BUFLEN];
                // SAFETY: viStatusDesc writes at most 256 bytes including the terminator.
                unsafe { (self.inner.api.status_desc)(obj, s, buf.as_mut_ptr()) };
                // SAFETY: NUL-terminated by viStatusDesc (buffer zero-initialised as a fallback).
                let desc = unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned();
                Err(VisaError::Status { code: s as u32, desc })
            }
        }
    }

    /// Resource names matching a VISA expression such as `USB?*INSTR`.
    pub fn find(&self, expr: &str) -> Result<Vec<String>> {
        let api = &self.inner.api;
        let expr = CString::new(expr).expect("no NUL in expression");
        let (mut list, mut count) = (0u32, 0u32);
        let mut buf = [0 as c_char; FIND_BUFLEN];
        // SAFETY: valid pointers; desc buffer is VI_FIND_BUFLEN long.
        let st = unsafe { (api.find_rsrc)(self.inner.session, expr.as_ptr(), &mut list, &mut count, buf.as_mut_ptr()) };
        match self.check(self.inner.session, st) {
            Err(VisaError::NotFound) => return Ok(Vec::new()),
            Err(e) => return Err(e),
            Ok(_) => {}
        }
        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count {
            if i > 0 {
                // SAFETY: list comes from viFindRsrc above.
                let st = unsafe { (api.find_next)(list, buf.as_mut_ptr()) };
                if st < VI_SUCCESS {
                    break;
                }
            }
            // SAFETY: NUL-terminated by VISA.
            out.push(unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned());
        }
        // SAFETY: find lists are VISA objects closed with viClose.
        unsafe { (api.close)(list) };
        Ok(out)
    }

    pub fn open(&self, resource: &str, timeout_ms: u32) -> Result<Session> {
        let name = CString::new(resource).expect("no NUL in resource name");
        let mut vi = 0;
        // SAFETY: valid pointers; exclusive lock not requested (mode VI_NULL).
        let st = unsafe { (self.inner.api.open)(self.inner.session, name.as_ptr(), VI_NULL, VI_NULL, &mut vi) };
        self.check(self.inner.session, st)?;
        let session = Session { rm: self.clone(), vi };
        session.set_timeout(timeout_ms)?;
        Ok(session)
    }
}

/// An open instrument session. Closed on drop.
pub struct Session {
    rm: ResourceManager,
    vi: ViSession,
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: vi was opened by viOpen and is closed exactly once.
        unsafe { (self.rm.inner.api.close)(self.vi) };
    }
}

impl Session {
    fn api(&self) -> &Api {
        &self.rm.inner.api
    }

    pub fn set_timeout(&self, ms: u32) -> Result<()> {
        // SAFETY: plain attribute set on our own session.
        let st = unsafe { (self.api().set_attribute)(self.vi, VI_ATTR_TMO_VALUE, ms as ViAttrState) };
        self.rm.check(self.vi, st).map(|_| ())
    }

    pub fn write(&self, data: &[u8]) -> Result<()> {
        let mut done = 0u32;
        // SAFETY: data outlives the call; VISA does not keep the pointer.
        let st = unsafe { (self.api().write)(self.vi, data.as_ptr(), data.len() as u32, &mut done) };
        self.rm.check(self.vi, st).map(|_| ())
    }

    /// Read one complete message (until the instrument signals END).
    pub fn read(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        let mut chunk = vec![0u8; 64 * 1024];
        loop {
            let mut got = 0u32;
            // SAFETY: chunk is writable for chunk.len() bytes.
            let st = unsafe { (self.api().read)(self.vi, chunk.as_mut_ptr(), chunk.len() as u32, &mut got) };
            let st = self.rm.check(self.vi, st)?;
            out.extend_from_slice(&chunk[..got as usize]);
            if st != VI_SUCCESS_MAX_CNT {
                return Ok(out);
            }
        }
    }

    /// Device clear: drops a pending reply so the next exchange starts clean.
    pub fn clear(&self) -> Result<()> {
        // SAFETY: plain call on our own session.
        let st = unsafe { (self.api().clear)(self.vi) };
        self.rm.check(self.vi, st).map(|_| ())
    }
}

// The session handle is just an integer; VISA sessions may be used from any one thread at a time.
unsafe impl Send for Session {}
unsafe impl Send for ResourceManager {}
unsafe impl Sync for ResourceManager {}

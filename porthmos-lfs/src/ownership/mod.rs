use std::{collections::HashMap, ffi::CStr, mem::MaybeUninit, os::raw::c_char, ptr};

const FIRST_BUFFER: usize = 1024;
const LARGEST_BUFFER: usize = 1 << 20;
const MAX_INTERRUPTIONS: u32 = 8;

enum Lookup {
    Found(String),
    Missing,
    TooSmall,
    Interrupted,
}

fn resolve(id: u32, query: impl Fn(&mut [c_char]) -> Lookup) -> String {
    let mut size = FIRST_BUFFER;
    let mut interruptions = 0;
    while size <= LARGEST_BUFFER {
        let mut buffer = vec![0 as c_char; size];
        match query(&mut buffer) {
            Lookup::Found(name) if !name.is_empty() => return name,
            Lookup::Found(_) | Lookup::Missing => break,
            Lookup::TooSmall => size *= 2,
            Lookup::Interrupted => {
                interruptions += 1;
                if interruptions >= MAX_INTERRUPTIONS {
                    break;
                }
            }
        }
    }
    id.to_string()
}

fn text_of(name: *const c_char) -> Lookup {
    if name.is_null() {
        return Lookup::Missing;
    }
    Lookup::Found(unsafe { CStr::from_ptr(name) }.to_string_lossy().into_owned())
}

pub(crate) struct NameCache {
    lookup: fn(u32) -> String,
    known: HashMap<u32, String>,
}

impl NameCache {
    pub(crate) fn new(lookup: fn(u32) -> String) -> Self {
        Self { lookup, known: HashMap::new() }
    }

    pub(crate) fn name(&mut self, id: u32) -> String {
        self.known.entry(id).or_insert_with(|| (self.lookup)(id)).clone()
    }
}

pub(crate) fn user_name(uid: u32) -> String {
    resolve(uid, |buffer| {
        let mut record = MaybeUninit::<libc::passwd>::uninit();
        let mut found = ptr::null_mut();
        let code = unsafe { libc::getpwuid_r(uid, record.as_mut_ptr(), buffer.as_mut_ptr(), buffer.len(), &mut found) };
        match (code, found.is_null()) {
            (0, false) => text_of(unsafe { (*found).pw_name }),
            (libc::ERANGE, _) => Lookup::TooSmall,
            (libc::EINTR, _) => Lookup::Interrupted,
            _ => Lookup::Missing,
        }
    })
}

pub(crate) fn group_name(gid: u32) -> String {
    resolve(gid, |buffer| {
        let mut record = MaybeUninit::<libc::group>::uninit();
        let mut found = ptr::null_mut();
        let code = unsafe { libc::getgrgid_r(gid, record.as_mut_ptr(), buffer.as_mut_ptr(), buffer.len(), &mut found) };
        match (code, found.is_null()) {
            (0, false) => text_of(unsafe { (*found).gr_name }),
            (libc::ERANGE, _) => Lookup::TooSmall,
            (libc::EINTR, _) => Lookup::Interrupted,
            _ => Lookup::Missing,
        }
    })
}

#[cfg(test)]
mod tests;

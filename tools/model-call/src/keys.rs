//! Provider API keys, held by the operating system rather than by this project.
//!
//! ## Where a key must not be
//!
//! Not in a tracked file, not in `fleet.txt`, not on a command line, and not in plaintext
//! on disk. The first three are obvious. The fourth is the one projects get wrong, and this
//! repository already carries the finding: backlog F1 is `cloudflare-r2-tokens.txt` in plain
//! text on the desktop, closed as Richard's call with the residual risk written down. A tool
//! whose argument is that the memory is yours cannot leave the credential where anything
//! that reads the disk can take it.
//!
//! So the key lives in the operating system's own secret store, the one place on a desktop
//! that is encrypted at rest, scoped to a single user account, and not this project's to
//! defend.
//!
//! ## No crate holds the key
//!
//! There is a good cross-platform keyring crate. It is not used here, for the same reason
//! `kb` has one dependency: a credential helper is the worst possible place to add a
//! supply chain, because it is the dependency with the secret in its hands.
//!
//! Windows goes straight to DPAPI through `crypt32.dll`, which is two `extern` declarations
//! and no dependency at all. macOS and Linux shell out to `security` and `secret-tool`,
//! which are the operating system's own binaries rather than somebody's package.
//!
//! ## Reading is a function, not a subcommand
//!
//! [`get`] is callable from the binary and from the desktop app. There is deliberately no
//! `model-call keys get`: that would put a secret on somebody's stdout, and from there into
//! shell history, into a scrollback, and into a pasted bug report. `set` reads from stdin
//! for the matching reason, because an argument is visible in the process list to every
//! other process on the machine while it runs.


/// The providers this fleet knows how to call, and the environment variable each falls
/// back to. Adding one is a line here plus a row in [`crate::providers`].
pub const PROVIDERS: [(&str, &str); 4] = [
    ("gemini", "GEMINI_API_KEY"),
    ("openai", "OPENAI_API_KEY"),
    ("anthropic", "ANTHROPIC_API_KEY"),
    ("openrouter", "OPENROUTER_API_KEY"),
];

/// The service name in the OS keychain. Windows keys off the file path instead, so this
/// is read only on macOS and Linux.
#[cfg(not(windows))]
const SERVICE: &str = "ulpia";

pub fn is_provider(name: &str) -> bool {
    PROVIDERS.iter().any(|(p, _)| *p == name)
}

fn env_var(provider: &str) -> Option<&'static str> {
    PROVIDERS.iter().find(|(p, _)| *p == provider).map(|(_, v)| *v)
}

/// The key for a provider: the OS store first, the environment second.
///
/// The store wins because a key put there was put there deliberately. The environment stays
/// as a fallback so that CI, containers and headless boxes with no keyring still work, and
/// so this can be adopted with no migration.
pub fn get(provider: &str) -> Option<String> {
    if let Some(found) = from_store(provider).filter(|s| !s.trim().is_empty()) {
        return Some(found.trim().to_string());
    }
    env_var(provider)
        .and_then(|v| std::env::var(v).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Where the key came from, for a `list` that never prints a value.
pub fn source(provider: &str) -> &'static str {
    if from_store(provider).is_some_and(|s| !s.trim().is_empty()) {
        "os store"
    } else if env_var(provider).and_then(|v| std::env::var(v).ok()).is_some_and(|s| !s.trim().is_empty()) {
        "environment"
    } else {
        "not set"
    }
}

pub fn set(provider: &str, secret: &str) -> Result<(), String> {
    platform::set(provider, secret)
}

pub fn remove(provider: &str) -> Result<(), String> {
    platform::remove(provider)
}

fn from_store(provider: &str) -> Option<String> {
    platform::get(provider).ok().flatten()
}

// ---------------------------------------------------------------------------
// Windows: DPAPI, directly.
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod platform {
    use std::path::PathBuf;

    /// Where the ciphertext sits.
    ///
    /// Under the roaming profile rather than beside the fleet: a key belongs to a person
    /// and a machine, not to a checkout. Same reason `fleet-root.txt` lives out here, and
    /// the same rule from ADR-0011 that no absolute path exists inside the fleet.
    fn dir() -> PathBuf {
        let base = std::env::var("APPDATA")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".into());
        PathBuf::from(base).join("Ulpia").join("keys")
    }

    fn file(provider: &str) -> PathBuf {
        dir().join(format!("{provider}.dpapi"))
    }

    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    // The whole Windows dependency, and it is the operating system rather than a package.
    // DPAPI encrypts under the logged-on user's credentials, so the ciphertext is useless
    // on another account or another machine, which is exactly the property wanted here.
    #[link(name = "crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(
            data_in: *const DataBlob,
            description: *const u16,
            optional_entropy: *const DataBlob,
            reserved: *mut core::ffi::c_void,
            prompt_struct: *mut core::ffi::c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
        fn CryptUnprotectData(
            data_in: *const DataBlob,
            description: *mut *mut u16,
            optional_entropy: *const DataBlob,
            reserved: *mut core::ffi::c_void,
            prompt_struct: *mut core::ffi::c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(mem: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
    }

    fn protect(plain: &[u8]) -> Result<Vec<u8>, String> {
        let input = DataBlob { cb_data: plain.len() as u32, pb_data: plain.as_ptr() as *mut u8 };
        let mut out = DataBlob { cb_data: 0, pb_data: std::ptr::null_mut() };
        let ok = unsafe {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                &mut out,
            )
        };
        if ok == 0 {
            return Err("DPAPI refused to encrypt".into());
        }
        let bytes = unsafe { std::slice::from_raw_parts(out.pb_data, out.cb_data as usize) }.to_vec();
        unsafe { LocalFree(out.pb_data as *mut core::ffi::c_void) };
        Ok(bytes)
    }

    fn unprotect(cipher: &[u8]) -> Result<Vec<u8>, String> {
        let input = DataBlob { cb_data: cipher.len() as u32, pb_data: cipher.as_ptr() as *mut u8 };
        let mut out = DataBlob { cb_data: 0, pb_data: std::ptr::null_mut() };
        let ok = unsafe {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                &mut out,
            )
        };
        if ok == 0 {
            return Err("DPAPI refused to decrypt".into());
        }
        let bytes = unsafe { std::slice::from_raw_parts(out.pb_data, out.cb_data as usize) }.to_vec();
        unsafe { LocalFree(out.pb_data as *mut core::ffi::c_void) };
        Ok(bytes)
    }

    pub fn set(provider: &str, secret: &str) -> Result<(), String> {
        let cipher = protect(secret.as_bytes())?;
        std::fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
        std::fs::write(file(provider), cipher).map_err(|e| e.to_string())
    }

    pub fn get(provider: &str) -> Result<Option<String>, String> {
        let path = file(provider);
        if !path.exists() {
            return Ok(None);
        }
        let cipher = std::fs::read(&path).map_err(|e| e.to_string())?;
        // A ciphertext this account cannot decrypt is a real state, not a bug: the file was
        // copied from another machine or another user. Report it as absent so the
        // environment fallback still answers.
        Ok(unprotect(&cipher).ok().and_then(|b| String::from_utf8(b).ok()))
    }

    pub fn remove(provider: &str) -> Result<(), String> {
        match std::fs::remove_file(file(provider)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// macOS and Linux: the OS's own secret binaries.
// ---------------------------------------------------------------------------

#[cfg(not(windows))]
mod platform {
    use super::*;
    use std::io::Write;
    use std::process::{Command, Stdio};

    #[cfg(target_os = "macos")]
    pub fn set(provider: &str, secret: &str) -> Result<(), String> {
        // -w with the value would put the secret in the process list; -w with no value
        // makes `security` read it from stdin.
        run_with_stdin(
            "security",
            &["add-generic-password", "-U", "-s", SERVICE, "-a", provider, "-w"],
            secret,
        )
    }

    #[cfg(target_os = "macos")]
    pub fn get(provider: &str) -> Result<Option<String>, String> {
        let out = Command::new("security")
            .args(["find-generic-password", "-s", SERVICE, "-a", provider, "-w"])
            .output()
            .map_err(|e| e.to_string())?;
        Ok(out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()))
    }

    #[cfg(target_os = "macos")]
    pub fn remove(provider: &str) -> Result<(), String> {
        Command::new("security")
            .args(["delete-generic-password", "-s", SERVICE, "-a", provider])
            .output()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    #[cfg(not(target_os = "macos"))]
    pub fn set(provider: &str, secret: &str) -> Result<(), String> {
        run_with_stdin(
            "secret-tool",
            &["store", "--label", "Ulpia", "service", SERVICE, "account", provider],
            secret,
        )
    }

    #[cfg(not(target_os = "macos"))]
    pub fn get(provider: &str) -> Result<Option<String>, String> {
        let out = Command::new("secret-tool")
            .args(["lookup", "service", SERVICE, "account", provider])
            .output()
            .map_err(|e| e.to_string())?;
        Ok(out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()))
    }

    #[cfg(not(target_os = "macos"))]
    pub fn remove(provider: &str) -> Result<(), String> {
        Command::new("secret-tool")
            .args(["clear", "service", SERVICE, "account", provider])
            .output()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn run_with_stdin(program: &str, args: &[&str], secret: &str) -> Result<(), String> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("{program} did not start: {e}"))?;
        child
            .stdin
            .take()
            .ok_or("no stdin on the child")?
            .write_all(secret.as_bytes())
            .map_err(|e| e.to_string())?;
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The round trip, and the property that matters: what lands on disk is not the secret.
    ///
    /// Uses a provider name no real key will ever occupy, and removes it afterwards, so a
    /// test run never touches a credential somebody is using.
    #[test]
    fn a_secret_survives_the_round_trip_and_never_lands_in_the_clear() {
        let fake = "not-a-real-key-0123456789abcdef";
        set("gemini-selftest", fake).expect("store");

        assert_eq!(get("gemini-selftest").as_deref(), Some(fake), "reads back identical");

        #[cfg(windows)]
        {
            let base = std::env::var("APPDATA").unwrap_or_default();
            let path = std::path::Path::new(&base)
                .join("Ulpia").join("keys").join("gemini-selftest.dpapi");
            let raw = std::fs::read(&path).expect("ciphertext on disk");
            assert!(
                !raw.windows(fake.len()).any(|w| w == fake.as_bytes()),
                "the secret must not appear in the stored bytes"
            );
        }

        remove("gemini-selftest").expect("remove");
        assert!(get("gemini-selftest").is_none(), "removal is real");
    }

    /// The fallback exists so no migration is needed, and the order matters: a key put in
    /// the store deliberately must beat one left in a shell profile years ago.
    #[test]
    fn an_unknown_provider_has_no_environment_variable_to_fall_back_to() {
        assert!(!is_provider("nope"));
        assert!(get("nope").is_none());
    }
}

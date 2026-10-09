//! S8 spike. `launch` (run non-elevated) listens on loopback, starts `host` elevated through UAC,
//! and forwards stdin lines to the elevated pwsh. Raw bytes only; no Marshell protocol.
use std::io::{BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("launch") => launch(),
        Some("host") => {
            // The helper window is hidden, so leave a trace of how it ended.
            let r = host(args[2].parse()?, &args[3]);
            let log = std::env::temp_dir().join("s8-host.log");
            std::fs::write(log, format!("{r:?}
")).ok();
            r
        }
        _ => anyhow::bail!("usage: elevated-host-spike launch"),
    }
}

fn launch() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let mut raw = [0u8; 32];
    getrandom::fill(&mut raw).expect("rng");
    let token = hex::encode(raw);
    let exe = std::env::current_exe()?;
    println!("elevated before UAC? {}", is_elevated());
    shell_execute_runas(&exe, &format!("host {port} {token}"))?;
    let (mut stream, peer) = listener.accept()?;
    let mut line = String::new();
    std::io::BufReader::new(stream.try_clone()?).read_line(&mut line)?;
    anyhow::ensure!(line.trim() == token, "helper sent a wrong token");
    println!("helper connected from {peer}; type commands (e.g. `whoami /groups | findstr Mandatory`)");
    let mut out = stream.try_clone()?;
    let mut answer = stream.try_clone()?;
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n) = out.read(&mut buf) {
            if n == 0 {
                break;
            }
            // ConPTY (portable-pty 0.9) asks for the cursor position at startup and holds all
            // output until it gets an answer; the Marshell core does the same (pty/dsr.rs).
            if buf[..n].windows(4).any(|w| w == b"[6n") {
                answer.write_all(b"[1;1R").ok();
            }
            std::io::stdout().write_all(&buf[..n]).ok();
            std::io::stdout().flush().ok();
        }
    });
    for l in std::io::stdin().lock().lines() {
        stream.write_all(format!("{}\r", l?).as_bytes())?;
    }
    Ok(())
}

fn host(port: u16, token: &str) -> anyhow::Result<()> {
    anyhow::ensure!(is_elevated(), "host must run elevated");
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.write_all(format!("{token}\n").as_bytes())?;
    let pty = portable_pty::native_pty_system().openpty(portable_pty::PtySize {
        rows: 30,
        cols: 120,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    let _child = pty.slave.spawn_command(portable_pty::CommandBuilder::new("pwsh.exe"))?;
    let mut reader = pty.master.try_clone_reader()?;
    let mut writer = pty.master.take_writer()?;
    let mut out = stream.try_clone()?;
    std::thread::spawn(move || std::io::copy(&mut reader, &mut out));
    // The helper accepts input bytes only. In production it also accepts resize, and nothing else.
    std::io::copy(&mut stream, &mut writer)?;
    Ok(())
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn shell_execute_runas(exe: &std::path::Path, params: &str) -> anyhow::Result<()> {
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;
    let (verb, file, params) = (wide("runas"), wide(&exe.display().to_string()), wide(params));
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = SW_HIDE;
    let ok = unsafe { ShellExecuteExW(&mut info) };
    anyhow::ensure!(ok != 0, "UAC declined or launch failed: {}", std::io::Error::last_os_error());
    Ok(())
}

#[cfg(windows)]
fn is_elevated() -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
fn shell_execute_runas(_: &std::path::Path, _: &str) -> anyhow::Result<()> {
    anyhow::bail!("Windows only")
}

#[cfg(not(windows))]
fn is_elevated() -> bool {
    false
}

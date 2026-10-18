use std::io::Write;

fn main() {
    // No panic message on stderr; `run` logs an internal error itself.
    std::panic::set_hook(Box::new(|_| {}));
    // `args()` would panic on a non-Unicode argument, before the guard below.
    let argv: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let out = std::panic::catch_unwind(|| {
        marshell_hook::run(&argv, &mut std::io::stdin().lock(), &|k| std::env::var(k).ok())
    })
    .unwrap_or_default();
    if !out.is_empty() {
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(&out);
        let _ = stdout.flush();
    }
    std::process::exit(0);
}

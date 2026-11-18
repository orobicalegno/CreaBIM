//! The activity log: what the app did, one line at a time, in a plain
//! text file the owner can open (Help › Open the activity log) and send
//! with a report.
//!
//! It records what helps find a bug, not what the user types: drags
//! frame by frame (pointer, the point it gave, the snap and step that
//! applied, the value that resulted, how long the frame took), edits and
//! undos, tool changes. Nothing leaves the computer.
//!
//! The file is `activity.log` in the app's local data folder; when it
//! passes 4 MB it is moved to `activity.old.log` and a new one starts.

use std::io::Write;
use std::sync::Mutex;
use std::time::Instant;

struct Log {
    file: Option<std::fs::File>,
    start: Instant,
}

static LOG: Mutex<Option<Log>> = Mutex::new(None);

/// The last lines, for the Log view (MU40): what the file holds lately,
/// without reading it back.
static RECENT: Mutex<std::collections::VecDeque<String>> =
    Mutex::new(std::collections::VecDeque::new());

/// How many lines the Log view keeps.
const KEEP: usize = 4000;

/// The lines written lately, oldest first, and how many were ever written
/// (so a view can tell whether there is anything new).
pub fn recent() -> (Vec<String>, usize) {
    let r = RECENT.lock().unwrap_or_else(|e| e.into_inner());
    (
        r.iter().cloned().collect(),
        WRITTEN.load(std::sync::atomic::Ordering::Relaxed),
    )
}

static WRITTEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Is this line about something going wrong?
pub fn is_problem(line: &str) -> bool {
    let l = line.to_lowercase();
    l.contains("crash")
        || l.contains("couldn't")
        || l.contains("error")
        || l.contains("failed")
        || l.contains("not changed")
        || l.contains("panicked")
        || l.contains("slow:")
}

/// Largest the log grows before it starts again.
const MAX_BYTES: u64 = 4 * 1024 * 1024;

/// Where the log is kept.
pub fn path() -> Option<std::path::PathBuf> {
    let var = |k: &str| std::env::var_os(k).map(std::path::PathBuf::from);
    let base = if cfg!(windows) {
        var("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library").join("Logs"))
    } else {
        var("XDG_STATE_HOME").or_else(|| var("HOME").map(|h| h.join(".local").join("state")))
    }?;
    Some(base.join("CreaBIM").join("activity.log"))
}

fn open() -> Option<std::fs::File> {
    let p = path()?;
    std::fs::create_dir_all(p.parent()?).ok()?;
    if std::fs::metadata(&p).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = std::fs::rename(&p, p.with_file_name("activity.old.log"));
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(p)
        .ok()
}

/// Start the log for this run: a header with the version and the time.
pub fn start() {
    let mut log = LOG.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = open();
    if let Some(f) = &mut file {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let _ = writeln!(
            f,
            "\n=== Volumen {} (build {}) started, unix time {secs} ===",
            env!("CARGO_PKG_VERSION"),
            env!("CREABIM_COMMIT"),
        );
    }
    *log = Some(Log {
        file,
        start: Instant::now(),
    });
}

/// Write one line, stamped with the seconds since the app started.
pub fn line(text: impl AsRef<str>) {
    let mut log = LOG.lock().unwrap_or_else(|e| e.into_inner());
    let Some(log) = log.as_mut() else {
        return;
    };
    let t = log.start.elapsed().as_secs_f64();
    let row = format!("{t:10.3}  {}", text.as_ref());
    if let Some(f) = &mut log.file {
        let _ = writeln!(f, "{row}");
    }
    let mut r = RECENT.lock().unwrap_or_else(|e| e.into_inner());
    // a crash's trace is many lines: each its own row in the view
    for l in row.lines() {
        if r.len() >= KEEP {
            r.pop_front();
        }
        r.push_back(l.to_string());
    }
    WRITTEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// A length in millimetres, for the log.
pub fn mm(v: f64) -> String {
    format!("{:.1}", v * 1000.0)
}

/// A point in millimetres, for the log.
pub fn pt(p: glam::DVec3) -> String {
    format!("({}, {}, {})", mm(p.x), mm(p.y), mm(p.z))
}

/// A step slower than this is lag worth knowing about, ms.
pub const SLOW_MS: f64 = 50.0;

/// Slow steps by kind: how many, the worst, the total (ms), and when the
/// last was written to the log.
type Slow = std::collections::BTreeMap<&'static str, (u32, f64, f64, Option<Instant>)>;
static SLOW: Mutex<Slow> = Mutex::new(std::collections::BTreeMap::new());

/// Note how long a step took: past [`SLOW_MS`] it counts as lag.
pub fn slow(what: &'static str, ms: f64) {
    if ms < SLOW_MS {
        return;
    }
    let write = {
        let mut t = SLOW.lock().unwrap_or_else(|e| e.into_inner());
        let e = t.entry(what).or_insert((0, 0.0, 0.0, None));
        e.0 += 1;
        e.1 = e.1.max(ms);
        e.2 += ms;
        let due = e.3.is_none_or(|w| w.elapsed().as_secs_f64() >= 1.0);
        if due {
            e.3 = Some(Instant::now());
        }
        due
    };
    if write {
        line(format!("slow: {what} took {ms:.0} ms"));
    }
}

/// The lag of this run as a table: each kind of step, how often it was
/// slow, its worst and its mean, the worst first.
pub fn slow_table() -> String {
    let t = SLOW.lock().unwrap_or_else(|e| e.into_inner());
    let mut rows: Vec<_> = t.iter().collect();
    rows.sort_by(|a, b| b.1 .1.total_cmp(&a.1 .1));
    if rows.is_empty() {
        return format!(
            "No step took over {SLOW_MS:.0} ms.
"
        );
    }
    let mut out = format!(
        "{:<28} {:>6} {:>9} {:>9}
",
        "step", "times", "worst ms", "mean ms"
    );
    for (what, (n, worst, total, _)) in rows {
        out.push_str(&format!(
            "{what:<28} {n:>6} {worst:>9.0} {:>9.0}
",
            total / *n as f64
        ));
    }
    out
}

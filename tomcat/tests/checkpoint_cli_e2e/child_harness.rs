use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(super) struct CapturedChildOutput {
    pub status: ExitStatus,
    pub stderr: String,
    pub stdout: String,
}

pub(super) struct CheckpointChild {
    child: Option<Child>,
    pid: u32,
    stderr: Arc<Mutex<Vec<u8>>>,
    stderr_reader: Option<JoinHandle<()>>,
    stdin: Option<ChildStdin>,
    stdout: Arc<Mutex<Vec<u8>>>,
    stdout_reader: Option<JoinHandle<()>>,
}

impl CheckpointChild {
    pub(super) fn spawn(command: &mut Command) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("chat child should start");
        let pid = child.id();
        let stdin = child.stdin.take().expect("stdin should be piped");
        let stdout = child.stdout.take().expect("stdout should be piped");
        let stderr = child.stderr.take().expect("stderr should be piped");
        let stdout_buffer = Arc::new(Mutex::new(Vec::new()));
        let stderr_buffer = Arc::new(Mutex::new(Vec::new()));
        let stdout_reader = Some(spawn_reader(stdout, Arc::clone(&stdout_buffer)));
        let stderr_reader = Some(spawn_reader(stderr, Arc::clone(&stderr_buffer)));
        Self {
            child: Some(child),
            pid,
            stderr: stderr_buffer,
            stderr_reader,
            stdin: Some(stdin),
            stdout: stdout_buffer,
            stdout_reader,
        }
    }

    pub(super) fn pid(&self) -> u32 {
        self.pid
    }

    pub(super) fn write_line(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().expect("child stdin is closed");
        stdin.write_all(line.as_bytes()).expect("write child stdin");
        stdin.write_all(b"\n").expect("terminate child input line");
        stdin.flush().expect("flush child stdin");
    }

    pub(super) fn close_stdin(&mut self) {
        self.stdin.take();
    }

    pub(super) fn stderr_snapshot(&self) -> String {
        String::from_utf8_lossy(&self.stderr.lock().expect("stderr buffer lock")).into_owned()
    }

    pub(super) fn stdout_snapshot(&self) -> String {
        String::from_utf8_lossy(&self.stdout.lock().expect("stdout buffer lock")).into_owned()
    }

    pub(super) fn wait_for_stderr(&self, needle: &str, timeout: Duration) {
        self.wait_for_output("stderr", needle, timeout, || self.stderr_snapshot());
    }

    pub(super) fn wait_for_stdout(&self, needle: &str, timeout: Duration) {
        self.wait_for_output("stdout", needle, timeout, || self.stdout_snapshot());
    }

    fn wait_for_output(
        &self,
        stream_name: &str,
        needle: &str,
        timeout: Duration,
        snapshot: impl Fn() -> String,
    ) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if snapshot().contains(needle) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "child pid={} {stream_name} did not contain {needle:?} within {timeout:?}; diagnostics={}",
            self.pid,
            self.diagnostics(),
        );
    }

    pub(super) fn finish(mut self, timeout: Duration) -> CapturedChildOutput {
        self.close_stdin();
        let deadline = Instant::now() + timeout;
        let mut timed_out = false;
        let status = loop {
            let child = self.child.as_mut().expect("child process missing");
            if let Some(status) = child.try_wait().expect("query child status") {
                break status;
            }
            if Instant::now() >= deadline {
                timed_out = true;
                terminate_process_tree(child, self.pid);
                break child.wait().expect("reap timed-out child");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        self.child.take();
        self.join_readers();
        let output = CapturedChildOutput {
            status,
            stderr: self.stderr_snapshot(),
            stdout: self.stdout_snapshot(),
        };
        if timed_out {
            panic!(
                "checkpoint child timed out after {timeout:?}; pid={}; status={}; stdout={:?}; stderr={:?}",
                self.pid,
                output.status,
                output.stdout,
                output.stderr,
            );
        }
        output
    }

    fn diagnostics(&self) -> String {
        format!(
            "pid={} stdout={:?} stderr={:?}",
            self.pid,
            self.stdout_snapshot(),
            self.stderr_snapshot(),
        )
    }

    fn join_readers(&mut self) {
        if let Some(reader) = self.stdout_reader.take() {
            let _ = reader.join();
        }
        if let Some(reader) = self.stderr_reader.take() {
            let _ = reader.join();
        }
    }
}

impl Drop for CheckpointChild {
    fn drop(&mut self) {
        self.stdin.take();
        if let Some(mut child) = self.child.take() {
            match child.try_wait() {
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => {
                    terminate_process_tree(&mut child, self.pid);
                    let _ = child.wait();
                }
            }
        }
        self.join_readers();
    }
}

fn terminate_process_tree(child: &mut Child, pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
    #[cfg(not(unix))]
    let _ = pid;
    let _ = child.kill();
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn timeout_kills_reaps_and_drains_the_process_group() {
        let mut command = Command::new("sh");
        command.args(["-c", "printf 'ready'; printf 'diagnostic' >&2; sleep 30"]);
        let child = CheckpointChild::spawn(&mut command);
        let pid = child.pid();
        child.wait_for_stdout("ready", Duration::from_secs(2));
        child.wait_for_stderr("diagnostic", Duration::from_secs(2));

        let timed_out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            child.finish(Duration::from_millis(40));
        }));
        assert!(timed_out.is_err(), "timeout must report structured failure");
        let alive = unsafe { libc::kill(pid as i32, 0) };
        assert_eq!(alive, -1, "timed-out child must already be reaped");
    }
}

fn spawn_reader<R>(mut reader: R, output: Arc<Mutex<Vec<u8>>>) -> JoinHandle<()>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(size) => output
                    .lock()
                    .expect("child output buffer lock")
                    .extend_from_slice(&chunk[..size]),
                Err(_) => break,
            }
        }
    })
}

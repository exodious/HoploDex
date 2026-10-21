//! A private `dbus-daemon` for the tests that stand in for a desktop portal (011
//! research.md §7, §8). Each test starts its own bus on a private address, never
//! the developer's session bus, and hands the code under test a connection to it
//! as a parameter. Nothing goes through an environment variable: the integration
//! program shares one environment.
//!
//! `dbus-daemon` is in the dev container; a missing one fails the test with a
//! message naming it, rather than skipping it.

use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Child, ChildStdout, Command, Stdio};

use tempfile::TempDir;

const BUS_CONFIG: &str = r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:dir=@DIR@</listen>
  <auth>EXTERNAL</auth>
  <servicedir>@SERVICES@</servicedir>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#;

/// A private `dbus-daemon`, killed when this is dropped.
pub struct Bus {
    child: Child,
    address: String,
    _stdout: BufReader<ChildStdout>,
    _dir: TempDir,
}

// Each test program uses some of these.
#[allow(dead_code)]
impl Bus {
    /// A bus that can start nothing: its service directory is empty, so nothing
    /// on the machine (a real `xdg-desktop-portal`) can be started by
    /// activation.
    pub fn start() -> Self {
        Self::start_with_services(&[])
    }

    /// A bus whose service directory holds one `.service` file per
    /// `(name, Exec line)`.
    pub fn start_with_services(services: &[(&str, &str)]) -> Self {
        let dir = TempDir::new().unwrap();
        let service_dir = dir.path().join("services");
        fs::create_dir(&service_dir).unwrap();
        for (name, exec) in services {
            fs::write(
                service_dir.join(format!("{name}.service")),
                format!("[D-BUS Service]\nName={name}\nExec={exec}\n"),
            )
            .unwrap();
        }
        let config = dir.path().join("bus.conf");
        fs::write(
            &config,
            BUS_CONFIG
                .replace("@DIR@", &dir.path().to_string_lossy())
                .replace("@SERVICES@", &service_dir.to_string_lossy()),
        )
        .unwrap();
        let mut child = Command::new("dbus-daemon")
            .arg("--config-file")
            .arg(&config)
            .args(["--nofork", "--print-address"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|err| {
                panic!(
                    "this test needs `dbus-daemon` (it is in the dev container, started through \
                     scripts/dev-container.sh), and starting it failed: {err}"
                )
            });
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        let address = line.trim().to_owned();
        assert!(!address.is_empty(), "`dbus-daemon` printed no address");
        Self { child, address, _stdout: stdout, _dir: dir }
    }

    /// A connection to this bus, as the app's portal route would make.
    pub fn connect(&self) -> zbus::Connection {
        zbus::block_on(async {
            zbus::connection::Builder::address(self.address.as_str())?.build().await
        })
        .expect("connecting to the test bus")
    }

    /// Owns `name` on this bus and serves `object` at `path`.
    pub fn serve<T: zbus::object_server::Interface>(
        &self,
        name: &str,
        path: &str,
        object: T,
    ) -> zbus::Connection {
        zbus::block_on(async {
            zbus::connection::Builder::address(self.address.as_str())?
                .name(name.to_owned())?
                .serve_at(path.to_owned(), object)?
                .build()
                .await
        })
        .expect("serving a stand-in")
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

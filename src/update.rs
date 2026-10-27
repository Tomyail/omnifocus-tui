use crate::i18n::Lang;
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

const REPO: &str = "Tomyail/omnifocus-tui";

const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// `omnifocus-tui update`：立即更新并在终端汇报结果。
pub fn run(lang: Lang) -> Result<(), Box<dyn std::error::Error>> {
    let current = env!("CARGO_PKG_VERSION");
    match install(lang, true)? {
        Some(latest) => println!("{}", lang.update_done(current, &latest)),
        None => println!("{}", lang.update_already(current)),
    }
    Ok(())
}

/// 启动时的后台自动更新：每 24 小时最多检查一次，失败一律静默。
/// 返回已安装的新版本号；开发构建（位于 target/ 下）从不自动替换。
pub fn auto(lang: Lang) -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    if exe.components().any(|part| part.as_os_str() == "target") {
        return None;
    }
    let stamp = stamp_path()?;
    let fresh = fs::metadata(&stamp)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.elapsed().ok())
        .is_some_and(|age| age < CHECK_INTERVAL);
    if fresh {
        return None;
    }
    // 先记录检查时间：即使网络失败也不会在每次启动时重试。
    fs::create_dir_all(stamp.parent()?).ok()?;
    fs::write(&stamp, b"").ok()?;
    install(lang, false).ok().flatten()
}

fn stamp_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("omnifocus-tui").join("last-update-check"))
}

/// 拉取最新 release，校验后替换当前二进制；已是最新返回 None，否则返回新版本号。
/// 网络与解压复用系统自带的 curl / tar / shasum，不引入额外依赖。
fn install(lang: Lang, announce: bool) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let current = env!("CARGO_PKG_VERSION");
    let cur =
        parse_version(current).ok_or_else(|| format!("unparsable current version: {current}"))?;

    let latest_tag = latest_tag().map_err(|_| lang.update_fail_fetch())?;
    let latest = latest_tag.trim_start_matches('v');
    let lat =
        parse_version(latest).ok_or_else(|| format!("unparsable release tag: {latest_tag}"))?;
    if lat <= cur {
        return Ok(None);
    }

    if announce {
        println!("{} v{latest}…", lang.update_downloading());
    }
    let target = format!("{}-apple-darwin", std::env::consts::ARCH);
    let dir = std::env::temp_dir().join(format!("omnifocus-tui-update-{}", std::process::id()));
    fs::create_dir_all(&dir).map_err(io_err)?;
    let cleanup = |dir: &Path| {
        let _ = fs::remove_dir_all(dir);
    };

    let base = format!("https://github.com/{REPO}/releases/download/{latest_tag}");
    let tarname = format!("omnifocus-tui-{latest}-{target}.tar.gz");
    let tarpath = dir.join(&tarname);
    if !download(&format!("{base}/{tarname}"), &tarpath)
        || !download(&format!("{base}/checksums.txt"), &dir.join("checksums.txt"))
    {
        cleanup(&dir);
        return Err(lang.update_fail_fetch().into());
    }

    if !checksum_matches(&dir.join("checksums.txt"), &tarname, &tarpath) {
        cleanup(&dir);
        return Err(lang.update_fail_checksum().into());
    }

    let extract = Command::new("tar")
        .args([
            "-xzf",
            &tarpath.to_string_lossy(),
            "-C",
            &dir.to_string_lossy(),
        ])
        .status()
        .map_err(io_err)?;
    if !extract.success() {
        cleanup(&dir);
        return Err(lang.update_fail_fetch().into());
    }

    let exe = std::env::current_exe().map_err(io_err)?;
    // 先写同目录临时文件再 rename，保证与目标在同一文件系统且替换是原子操作。
    let staging = exe
        .parent()
        .ok_or("current executable has no parent directory")?
        .join(".omnifocus-tui.update");
    let install = (|| -> Result<(), Box<dyn std::error::Error>> {
        fs::copy(dir.join("omnifocus-tui"), &staging).map_err(io_err)?;
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o755)).map_err(io_err)?;
        fs::rename(&staging, &exe)
            .map_err(|e| -> Box<dyn std::error::Error> { lang.update_fail_install(&e).into() })?;
        Ok(())
    })();
    cleanup(&dir);
    install?;
    Ok(Some(latest.to_owned()))
}

fn io_err(error: std::io::Error) -> Box<dyn std::error::Error> {
    error.into()
}

/// 读取 GitHub API 的 latest release tag。
fn latest_tag() -> Result<String, Box<dyn std::error::Error>> {
    let out = Command::new("curl")
        .args([
            "-fsSL",
            &format!("https://api.github.com/repos/{REPO}/releases/latest"),
        ])
        .output()
        .map_err(io_err)?;
    if !out.status.success() {
        return Err("GitHub API request failed".into());
    }
    let json: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    json["tag_name"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "release payload has no tag_name".into())
}

fn download(url: &str, to: &Path) -> bool {
    Command::new("curl")
        .args(["-fsSL", "-o", &to.to_string_lossy(), url])
        .status()
        .is_ok_and(|status| status.success())
}

/// 与 release 里的 checksums.txt 比对 sha256。
fn checksum_matches(sums_path: &Path, tarname: &str, tarpath: &Path) -> bool {
    let Ok(sums) = fs::read_to_string(sums_path) else {
        return false;
    };
    let Some(want) = sums
        .lines()
        .find(|line| line.contains(tarname))
        .and_then(|line| line.split_whitespace().next())
    else {
        return false;
    };
    let Ok(out) = Command::new("shasum")
        .args(["-a", "256", &tarpath.to_string_lossy()])
        .output()
    else {
        return false;
    };
    let Ok(got) = String::from_utf8(out.stdout) else {
        return false;
    };
    out.status.success()
        && got
            .split_whitespace()
            .next()
            .is_some_and(|hash| hash.eq_ignore_ascii_case(want))
}

/// "0.1.1" → (0, 1, 1)；解析失败返回 None。
fn parse_version(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_compares_versions() {
        let v = |s: &str| parse_version(s).unwrap();
        assert_eq!(v("0.1.1"), (0, 1, 1));
        assert!(v("0.1.2") > v("0.1.1"));
        assert!(v("0.2.0") > v("0.1.9"));
        assert!(v("1.0.0") > v("0.9.9"));
        assert_eq!(v("0.1.1"), v("0.1.1"));
        assert!(parse_version("abc").is_none());
        assert!(parse_version("0.1").is_none());
        assert!(parse_version("0.1.1.1").is_none());
    }
}

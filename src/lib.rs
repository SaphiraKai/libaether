/*!
# libaether - a library for creating software that interacts with Aether packages, repositories, and environments

This project is currently heavily WIP and experimental, and doesn't even have a roadmap.
The end goal is to create a modern replacement for ALPM and pacman, featuring
more flexible and comprehensive tools for managing packages and proper per-user package
management, no root required - while maintaining as much of the simplicity of
Arch as possible.
*/

use anyhow::{anyhow, Context, Result};
use mtree;
use std::fs::{metadata, read, read_dir};
use std::io::{Cursor, Write};
use std::process::{Command, Stdio};
use std::str::from_utf8;

/**
Contains data parsed from a .PKGINFO file

# Public fields:
```
pkgname: String
pkgbase: String
pkgver: String
pkgdesc: String
url: String
builddate: i32
packager: String
size: i32
arch: Vec<String>
license: String
conflict: Vec<String>
provides: Vec<String>
depend: Vec<String>
optdepend: Vec<String>
```

# Public methods:
```
// return an intialized PkgInfo instance
PkgInfo::new() : pub fn new() -> PkgInfo

// parse a file and return a PkgInfo instance
PkgInfo::parse() : pub fn parse(file: &str) -> Result<PkgInfo>
```
*/
#[derive(Debug)]
pub struct PkgInfo {
    pub pkgname: String,
    pub pkgbase: String,
    pub pkgver: String,
    pub pkgdesc: String,
    pub url: String,
    pub builddate: i32,
    pub packager: String,
    pub size: i32,
    pub arch: Vec<String>,
    pub license: String,
    pub conflict: Vec<String>,
    pub provides: Vec<String>,
    pub depend: Vec<String>,
    pub optdepend: Vec<String>,
}

impl PkgInfo {
    /// return an intialized PkgInfo instance
    pub fn new() -> PkgInfo {
        let pkginfo = PkgInfo {
            pkgname: String::new(),
            pkgbase: String::new(),
            pkgver: String::new(),
            pkgdesc: String::new(),
            url: String::new(),
            builddate: 0,
            packager: String::new(),
            size: 0,
            arch: vec![],
            license: String::new(),
            conflict: vec![],
            provides: vec![],
            depend: vec![],
            optdepend: vec![],
        };

        return pkginfo;
    }
    /// parse a file and return a PkgInfo instance
    pub fn parse(file: &str) -> Result<PkgInfo> {
        let pkginfo_raw = read(file).with_context(|| format!("unable to read file: {}", file))?;

        let pkginfo_lines = from_utf8(&pkginfo_raw)
            .context("found invalid utf-8 while attempting to parse for a PkgInfo")?
            .lines();

        let mut pkginfo = PkgInfo::new();
        for line in pkginfo_lines {
            if line.chars().nth(0) == Some('#') {
                continue;
            }

            let key = line.split(" = ").nth(0).unwrap();

            let value = line.split(" = ").nth(1).unwrap();

            match key {
                "pkgname" => pkginfo.pkgname = value.to_string(),
                "pkgbase" => pkginfo.pkgbase = value.to_string(),
                "pkgver" => pkginfo.pkgver = value.to_string(),
                "pkgdesc" => pkginfo.pkgdesc = value.to_string(),
                "url" => pkginfo.url = value.to_string(),
                "builddate" => pkginfo.builddate = value.parse().unwrap(),
                "packager" => pkginfo.packager = value.to_string(),
                "size" => pkginfo.size = value.parse().unwrap(),
                "arch" => pkginfo.arch.push(value.to_string()),
                "license" => pkginfo.license = value.to_string(),
                "conflict" => pkginfo.conflict.push(value.to_string()),
                "provides" => pkginfo.provides.push(value.to_string()),
                "depend" => pkginfo.depend.push(value.to_string()),
                "optdepend" => pkginfo.optdepend.push(value.to_string()),
                &_ => return Err(anyhow!("{}: unrecognized key name for PkgInfo", key)),
            }
        }

        Ok(pkginfo)
    }
}

impl Default for PkgInfo {
    fn default() -> Self {
        Self::new()
    }
}

/**
Contains data parsed from a .BUILDINFO file

# Public fields:
```
format: i32
pkgname: String
pkgbase: String
pkgver: String
pkgarch: Vec<String>
pkgbuild_sha256sum: String
pkgbuild_md5sum: String
pkgbuild_sha1sum: String
packager: String
builddate: i32
builddir: String
startdir: String
buildtool: String
buildtoolver: String
buildenv: Vec<String>
options: Vec<String>
installed: Vec<String>
```

# Public methods:
```
// return an intialized BuildInfo instance
BuildInfo::new() : pub fn new() -> BuildInfo

// parse a file and return a BuildInfo instance
BuildInfo::parse() : pub fn parse(file: &str) -> Result<BuildInfo>
```
*/
#[derive(Debug)]
pub struct BuildInfo {
    pub format: i32,
    pub pkgname: String,
    pub pkgbase: String,
    pub pkgver: String,
    pub pkgarch: Vec<String>,
    pub pkgbuild_sha256sum: String,
    pub pkgbuild_md5sum: String,
    pub pkgbuild_sha1sum: String,
    pub packager: String,
    pub builddate: i32,
    pub builddir: String,
    pub startdir: String,
    pub buildtool: String,
    pub buildtoolver: String,
    pub buildenv: Vec<String>,
    pub options: Vec<String>,
    pub installed: Vec<String>,
}

impl BuildInfo {
    /// return an intialized BuildInfo instance
    pub fn new() -> BuildInfo {
        let buildinfo = BuildInfo {
            format: 0,
            pkgname: String::new(),
            pkgbase: String::new(),
            pkgver: String::new(),
            pkgarch: vec![],
            pkgbuild_sha256sum: String::new(),
            pkgbuild_md5sum: String::new(),
            pkgbuild_sha1sum: String::new(),
            packager: String::new(),
            builddate: 0,
            builddir: String::new(),
            startdir: String::new(),
            buildtool: String::new(),
            buildtoolver: String::new(),
            buildenv: vec![],
            options: vec![],
            installed: vec![],
        };

        return buildinfo;
    }

    /// parse a file and return a BuildInfo instance
    pub fn parse(file: &str) -> Result<BuildInfo> {
        let buildinfo_raw = read(file).with_context(|| format!("unable to read file: {}", file))?;

        let buildinfo_lines = from_utf8(&buildinfo_raw)
            .context("found invalid utf-8 while attempting to parse for a BuildInfo")?
            .lines();

        let mut buildinfo = BuildInfo::new();
        for line in buildinfo_lines {
            let key = match line.split(" = ").nth(0) {
                Some(k) => k,
                None => panic!(
                    "unable to parse file for BuildInfo: couldn't find key in '{}'",
                    line
                ),
            };

            let value = match line.split(" = ").nth(1) {
                Some(v) => v,
                None => panic!(
                    "unable to parse file for BuildInfo: couldn't find value in '{}'",
                    line
                ),
            };

            match key {
                "format" => buildinfo.format = value.parse().context("unable to parse format")?,
                "pkgname" => buildinfo.pkgname = value.to_string(),
                "pkgbase" => buildinfo.pkgbase = value.to_string(),
                "pkgver" => buildinfo.pkgver = value.to_string(),
                "pkgarch" => buildinfo.pkgarch.push(value.to_string()),
                "pkgbuild_sha256sum" => buildinfo.pkgbuild_sha256sum = value.to_string(),
                "pkgbuild_md5sum" => buildinfo.pkgbuild_md5sum = value.to_string(),
                "pkgbuild_sha1sum" => buildinfo.pkgbuild_sha1sum = value.to_string(),
                "packager" => buildinfo.packager = value.to_string(),
                "builddate" => {
                    buildinfo.builddate = value.parse().context("unable to parse builddate")?
                }
                "builddir" => buildinfo.builddir = value.to_string(),
                "startdir" => buildinfo.startdir = value.to_string(),
                "buildtool" => buildinfo.buildtool = value.to_string(),
                "buildtoolver" => buildinfo.buildtoolver = value.to_string(),
                "buildenv" => buildinfo.buildenv.push(value.to_string()),
                "options" => buildinfo.options.push(value.to_string()),
                "installed" => buildinfo.installed.push(value.to_string()),
                &_ => panic!("{}: unrecognized key name for BuildInfo", key),
            }
        }

        Ok(buildinfo)
    }
}

impl Default for BuildInfo {
    fn default() -> Self {
        Self::new()
    }
}

/**
Contains data parsed from a .MTREE file
- This struct is a wrapper for the [`crate::mtree::MTree`] struct

# Public fields:
```
mtree: crate::mtree::Mtree<Cursor<Vec<u8>>>
```

# Public methods:
```
// parse a file and return an MTree instance
MTree::parse() : pub fn parse(file: &str) -> Result<MTree>
```
*/
pub struct MTree {
    pub mtree: mtree::MTree<Cursor<Vec<u8>>>,
}

impl MTree {
    /// parse a file and return an MTree instance
    pub fn parse(file: &str) -> Result<MTree> {
        let mtree_gzipped = read(file).with_context(|| format!("unable to read {}", file))?;

        let mut gunzip = Command::new("gunzip")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .context("unable to spawn process")?;

        let gunzip_stdin = gunzip.stdin.as_mut().unwrap();
        gunzip_stdin
            .write_all(&mtree_gzipped)
            .context("unable to write to process's stdin")?;
        drop(gunzip_stdin);

        let gunzip_stdout = gunzip
            .wait_with_output()
            .context("unable to wait for process")?;

        let mtree = MTree {
            mtree: mtree::MTree::from_reader(Cursor::new(gunzip_stdout.stdout)),
        };

        Ok(mtree)
    }
}

/**
Contains information about a libaether environment

Not yet implemented - WIP
*/
#[derive(Debug)]
pub struct AetherEnv {
    pub name: String,
    pub path: String,
    pub pkgs: Vec<String>,
}

/**
Contains all information related to a single libaether or ALPM compatible package

# Public fields:
```
files: Vec<String>
buildinfo: Option<BuildInfo>
mtree: MTree
pkginfo: PkgInfo
```

# Public methods:
```
// parse the specified directory and return a Pkg from its contents
Pkg::from_dir() : pub fn from_dir(dir: &str) -> Result<Pkg>

// parse the specified directory and return a Result<()> of whether or not it's
// a valid package
Pkg::is_valid_dir() : pub fn is_valid_dir(dir: &str) -> Result<()>

// wrapper for several [println!] calls that simply prints all stored package
// information, intended for debugging
Pkg::show() : pub fn show(&mut self)
```
*/
pub struct Pkg {
    pub files: Vec<String>,
    pub buildinfo: Option<BuildInfo>,
    pub mtree: MTree,
    pub pkginfo: PkgInfo,
}

impl Pkg {
    /// parse the specified directory and return a Pkg from its contents
    pub fn from_dir(dir: &str) -> Result<Pkg> {
        Pkg::is_valid_dir(dir).with_context(|| format!("package failed to validate: {}", dir))?;

        let mut files = vec![];
        let buildinfo_path = format!("{}/{}", dir, ".BUILDINFO");
        let mtree_path = format!("{}/{}", dir, ".MTREE");
        let pkginfo_path = format!("{}/{}", dir, ".PKGINFO");

        for file in read_dir(dir).unwrap() {
            files.push(file.unwrap().path().to_str().unwrap().to_string());
        }

        let buildinfo = BuildInfo::parse(&buildinfo_path).ok();
        let mtree = MTree::parse(&mtree_path)
            .with_context(|| format!("unable to parse {} into an MTree", &mtree_path))?;
        let pkginfo = PkgInfo::parse(&pkginfo_path)
            .with_context(|| format!("unable to parse {} into a PkgInfo", &pkginfo_path))?;

        let pkg = Pkg {
            files,
            buildinfo,
            mtree,
            pkginfo,
        };

        Ok(pkg)
    }

    /// parse the specified directory and return a Result<()> of whether or not
    /// it's a valid package
    pub fn is_valid_dir(dir: &str) -> Result<()> {
        let files = read_dir(dir).with_context(|| format!("unable to read directory: {}", dir))?;

        if files.count() == 0 {
            return Err(anyhow!("{}: package contains no data", dir));
        }

        if !metadata(format!("{}/{}", dir, ".MTREE")).is_ok() {
            return Err(anyhow!("{}: package is missing .MTREE", dir));
        } else if !metadata(format!("{}/{}", dir, ".PKGINFO")).is_ok() {
            return Err(anyhow!("{}: package is missing .PKGINFO", dir));
        };

        return Ok(());
    }

    /// wrapper for several [println!] calls that simply prints all stored
    ///package information, intended for debugging
    pub fn show(&mut self) {
        println!("{:#?}\n", self.files);
        println!("{:#?}\n", self.buildinfo);
        for entry in &mut self.mtree.mtree {
            println!("{}", entry.unwrap())
        }
        println!("{:#?}", self.pkginfo);
    }
}

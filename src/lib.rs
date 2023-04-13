/*!
# libaether - a library for software that interacts with Aether packages, repositories, and environments

This project is currently heavily WIP and experimental, and doesn't even have a roadmap.
The end goal is to create a modern replacement for ALPM and pacman, featuring
more flexible and comprehensive tools for managing packages and proper per-user package
management, no root required - while maintaining as much of the simplicity of
Arch as is practical.
*/

#![warn(clippy::all)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]

use fs_extra::dir;
use scan_dir::ScanDir;
use std::fmt;
use std::fs::{metadata, read, read_dir, DirEntry};
use std::io::{Cursor, Write};
use std::os::unix::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::from_utf8;
use thiserror::Error;

#[must_use]
pub fn bin_dir() -> PathBuf {
    dirs::executable_dir().unwrap()
}

#[must_use]
pub fn cache_dir() -> PathBuf {
    dirs::cache_dir().unwrap().join(&"aether")
}

#[must_use]
pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap().join(&"aether")
}

#[must_use]
pub fn pkg_dir() -> PathBuf {
    dirs::state_dir().unwrap().join(&"aether/pkg")
}

#[derive(Error, Debug)]
pub enum AetherError {
    #[error("file already exists: {0}")]
    AlreadyExists(String),

    #[error("unable to copy '{from}' -> '{to}'")]
    CopyError {
        from: PathBuf,
        to: PathBuf,
        source: fs_extra::error::Error,
    },

    #[error("invalid key name for {kind}: '{key}'")]
    InfoKeyError { kind: String, key: String },

    #[error("unable to parse {field} from '{line}'")]
    InfoParseError { field: String, line: String },

    #[error("invalid value for {kind}: '{value}'")]
    InfoValueError { kind: String, value: String },

    #[error("invalid package: {path}: {note}")]
    InvalidPkg { path: PathBuf, note: String },

    #[error("invalid value for {key}: '{value}'")]
    InvalidValue { key: String, value: String },

    #[error("unable to link '{from}' -> '{to}'")]
    LinkError {
        from: PathBuf,
        to: PathBuf,
        source: std::io::Error,
    },

    #[error("missing package execs: {0:?}")]
    MissingExec(Vec<PathBuf>),

    #[error("missing package files: {0:?}")]
    MissingFile(Vec<PathBuf>),

    #[error("package not found: {name}-{ver}")]
    MissingPkg { name: String, ver: String },

    #[error("file/directory does not exist: {file}")]
    NotFound {
        file: PathBuf,
        source: std::io::Error,
    },

    #[error("error executing process")]
    ProcessError(#[from] std::io::Error),

    #[error("unable to read file/directory: {file}")]
    ReadError {
        file: PathBuf,
        source: std::io::Error,
    },

    #[error("unknown error")]
    Unknown,

    #[error("unable to parse utf-8")]
    Utf8Error(#[from] std::str::Utf8Error),

    #[error("unable to modify file: {file}")]
    WriteError {
        file: PathBuf,
        source: std::io::Error,
    },
}

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
#[derive(Clone, Debug)]
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
    pub makedepend: Vec<String>,
    pub checkdepend: Vec<String>,
    pub backup: Vec<String>,
    pub group: Vec<String>,
}

impl PkgInfo {
    /// return an intialized `PkgInfo` instance
    #[must_use]
    pub fn new() -> PkgInfo {
        PkgInfo {
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
            makedepend: vec![],
            checkdepend: vec![],
            backup: vec![],
            group: vec![],
        }
    }

    /// parse a file and return a `PkgInfo` instance
    pub fn parse(file: &dyn AsRef<Path>) -> Result<PkgInfo, AetherError> {
        let file = &file.as_ref();

        let pkginfo_raw = read(file).map_err(|source| AetherError::ReadError {
            file: file.to_path_buf(),
            source,
        })?;

        let pkginfo_lines = from_utf8(&pkginfo_raw)
            .map_err(AetherError::Utf8Error)?
            .lines();

        let mut pkginfo = PkgInfo::new();
        for line in pkginfo_lines {
            if line.starts_with('#') {
                continue;
            }

            let key = line.split(" = ").next().unwrap();

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
                "makedepend" => pkginfo.makedepend.push(value.to_string()),
                "checkdepend" => pkginfo.checkdepend.push(value.to_string()),
                "backup" => pkginfo.backup.push(value.to_string()),
                "group" => pkginfo.group.push(value.to_string()),
                &_ => {
                    return Err(AetherError::InfoKeyError {
                        kind: "PkgInfo".into(),
                        key: key.into(),
                    })
                }
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
#[derive(Clone, Debug)]
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
    /// return an intialized `BuildInfo` instance
    #[must_use]
    pub fn new() -> BuildInfo {
        BuildInfo {
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
        }
    }

    /// parse a file and return a `BuildInfo` instance
    pub fn parse(file: &dyn AsRef<Path>) -> Result<BuildInfo, AetherError> {
        let file = &file.as_ref();

        let buildinfo_raw = read(file).map_err(|source| AetherError::ReadError {
            file: file.to_path_buf(),
            source,
        })?;

        let buildinfo_lines = from_utf8(&buildinfo_raw)
            .map_err(AetherError::Utf8Error)?
            .lines();

        let mut buildinfo = BuildInfo::new();
        for line in buildinfo_lines {
            let key = match line.split(" = ").next() {
                Some(v) => v,
                None => {
                    return Err(AetherError::InfoParseError {
                        field: "key".into(),
                        line: line.into(),
                    })
                }
            };

            let value = match line.split(" = ").nth(1) {
                Some(v) => v,
                None => {
                    return Err(AetherError::InfoParseError {
                        field: "value".into(),
                        line: line.into(),
                    })
                }
            };

            match key {
                "format" => {
                    buildinfo.format = value.parse().map_err(|_| AetherError::InvalidValue {
                        key: key.into(),
                        value: value.into(),
                    })?;
                }
                "pkgname" => buildinfo.pkgname = value.into(),
                "pkgbase" => buildinfo.pkgbase = value.into(),
                "pkgver" => buildinfo.pkgver = value.into(),
                "pkgarch" => buildinfo.pkgarch.push(value.into()),
                "pkgbuild_sha256sum" => buildinfo.pkgbuild_sha256sum = value.into(),
                "pkgbuild_md5sum" => buildinfo.pkgbuild_md5sum = value.into(),
                "pkgbuild_sha1sum" => buildinfo.pkgbuild_sha1sum = value.into(),
                "packager" => buildinfo.packager = value.into(),
                "builddate" => {
                    buildinfo.builddate = value.parse().map_err(|_| AetherError::InvalidValue {
                        key: key.into(),
                        value: value.into(),
                    })?;
                }
                "builddir" => buildinfo.builddir = value.into(),
                "startdir" => buildinfo.startdir = value.into(),
                "buildtool" => buildinfo.buildtool = value.into(),
                "buildtoolver" => buildinfo.buildtoolver = value.into(),
                "buildenv" => buildinfo.buildenv.push(value.into()),
                "options" => buildinfo.options.push(value.into()),
                "installed" => buildinfo.installed.push(value.into()),
                &_ => {
                    return Err(AetherError::InfoKeyError {
                        kind: "BuildInfo".into(),
                        key: key.into(),
                    })
                }
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
Contains data read from a .MTREE file
- This struct provides a wrapper for the [`mtree::MTree`] struct

# Public methods:
```
/// parse an mtree::MTree from this struct
fn get(&self) -> mtree::MTree<Cursor<Vec<u8>>>

// parse a file and return an MTree instance
MTree::parse() : pub fn parse(file: &str) -> Result<MTree>
```
*/
#[derive(Clone, Debug)]
pub struct MTree {
    raw: Vec<u8>,
}

impl MTree {
    /// parse an `mtree::MTree` from this struct
    fn get(&self) -> mtree::MTree<Cursor<Vec<u8>>> {
        mtree::MTree::from_reader(Cursor::new(self.raw.clone()))
    }

    /// read a file into an `MTree` instance
    fn parse(file: &dyn AsRef<Path>) -> Result<MTree, AetherError> {
        let file = &file.as_ref();

        let mtree_gzipped = read(file).map_err(|source| AetherError::ReadError {
            file: file.into(),
            source,
        })?;

        let mut gunzip = Command::new("gunzip")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(AetherError::ProcessError)?;

        let gunzip_stdin = gunzip.stdin.as_mut().unwrap();
        gunzip_stdin
            .write_all(&mtree_gzipped)
            .map_err(AetherError::ProcessError)?;

        let gunzip_stdout = gunzip
            .wait_with_output()
            .map_err(AetherError::ProcessError)?;

        let mtree = MTree {
            raw: gunzip_stdout.stdout,
        };

        Ok(mtree)
    }
}

/**
Contains all information related to a single Aether or ALPM compatible package

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
#[derive(Clone, Debug)]
pub struct Pkg {
    pub files: Vec<PathBuf>,
    pub buildinfo: Option<BuildInfo>,
    pub mtree: MTree,
    pub pkginfo: PkgInfo,
    pub path: PathBuf,
}

impl Pkg {
    /// parse the specified directory and return a Pkg from its contents
    pub fn from_dir(path: &dyn AsRef<Path>) -> Result<Pkg, AetherError> {
        let path = &path.as_ref();

        Pkg::is_valid_dir(path)?;

        let mut files = vec![];
        let buildinfo_path = Path::join(path, ".BUILDINFO");
        let mtree_path = Path::join(path, ".MTREE");
        let pkginfo_path = Path::join(path, ".PKGINFO");

        ScanDir::all()
            .walk(path, |iter| {
                for (entry, _) in iter {
                    files.push(entry.path());
                }
            })
            .unwrap();

        let buildinfo = BuildInfo::parse(&buildinfo_path).ok();
        let mtree = MTree::parse(&Path::new(&mtree_path))?;
        let pkginfo = PkgInfo::parse(&Path::new(&pkginfo_path))?;

        let pkg = Pkg {
            files,
            buildinfo,
            mtree,
            pkginfo,
            path: PathBuf::from(path),
        };

        Ok(pkg)
    }

    /// parse the specified directory and return a Result<()> of whether or not
    /// it's a valid package
    pub fn is_valid_dir(dir: &dyn AsRef<Path>) -> Result<(), AetherError> {
        let path = &dir.as_ref();

        let file_count: usize = read_dir(path)
            .map_err(|source| AetherError::ReadError {
                file: path.into(),
                source,
            })?
            .count();

        if file_count == 0 {
            return Err(AetherError::InvalidPkg {
                path: path.into(),
                note: "no data".into(),
            });
        }

        if metadata(Path::join(path, ".MTREE")).is_err() {
            return Err(AetherError::InvalidPkg {
                path: path.into(),
                note: "missing .MTREE file".into(),
            });
        } else if metadata(Path::join(path, ".PKGINFO")).is_err() {
            return Err(AetherError::InvalidPkg {
                path: path.into(),
                note: "missing .PKGINFO file".into(),
            });
        };

        MTree::parse(&Path::join(path, ".MTREE")).map_err(|_| AetherError::InvalidPkg {
            path: path.into(),
            note: "invalid .MTREE file".into(),
        })?;
        PkgInfo::parse(&Path::join(path, ".PKGINFO")).map_err(|_| AetherError::InvalidPkg {
            path: path.into(),
            note: "invalid .PKGINFO file".into(),
        })?;

        Ok(())
    }

    pub fn get_refstr(&self) -> String {
        format!("{}-{}", self.pkginfo.pkgname, self.pkginfo.pkgver)
    }

    pub fn check_files(&self) -> Result<Vec<PathBuf>, AetherError> {
        let mut checked = vec![];
        let mut missing = vec![];

        for file in self.files.clone() {
            let name = match file.file_name() {
                Some(name) => name,
                None => return Err(AetherError::Unknown),
            };

            let path = pkg_dir().join(name);

            if Path::exists(&path) {
                checked.push(path);
            } else {
                missing.push(path);
            }
        }

        if missing.is_empty() {
            Ok(checked)
        } else {
            Err(AetherError::MissingFile(missing))
        }
    }

    pub fn check_execs(&self) -> Result<Vec<PathBuf>, AetherError> {
        let execs = self.list_execs()?;

        let mut checked = vec![];
        let mut missing = vec![];

        for exec in execs {
            let path = bin_dir().join(exec.file_name());
            if Path::exists(&path) {
                checked.push(path);
            } else {
                missing.push(path);
            }
        }

        if missing.is_empty() {
            Ok(checked)
        } else {
            Err(AetherError::MissingExec(missing))
        }
    }

    pub fn list_execs(&self) -> Result<Vec<DirEntry>, AetherError> {
        let usr_bin = &self.path.join("usr/bin/");
        let bin = &self.path.join("bin/");

        let mut entries: Vec<Result<DirEntry, std::io::Error>> = vec![];

        match read_dir(usr_bin) {
            Err(err) => {
                if let std::io::ErrorKind::NotFound = err.kind() {
                } else {
                    return Err(AetherError::ReadError {
                        file: usr_bin.into(),
                        source: err,
                    });
                }
            }
            Ok(res) => res.for_each(|entry| entries.push(entry)),
        };

        match read_dir(bin) {
            Err(err) => {
                if let std::io::ErrorKind::NotFound = err.kind() {
                } else {
                    return Err(AetherError::ReadError {
                        file: bin.into(),
                        source: err,
                    });
                }
            }
            Ok(res) => res.for_each(|entry| entries.push(entry)),
        };

        let mut execs = vec![];
        for entry in entries {
            let file = entry.map_err(|_| AetherError::Unknown)?;
            execs.push(file);
        }

        Ok(execs)
    }

    /// wrapper for several [println!] calls that simply prints all stored
    /// package information, intended for debugging
    // TODO: get rid of this godawful &mut
    pub fn show_all(&mut self) {
        println!("{:#?}\n", self.files);
        println!("{:#?}\n", self.buildinfo);
        for entry in &mut self.mtree.get() {
            println!("{}", entry.unwrap());
        }
        println!("{:#?}", self.pkginfo);
        println!("{}", self.path.display());
    }

    pub fn symlink_execs(&self) -> Result<Vec<PathBuf>, AetherError> {
        let files = self.list_execs()?;
        let mut symlinked: Vec<PathBuf> = vec![];

        for file in files {
            let path = bin_dir().join(file.file_name());

            fs::symlink(file.path(), &path).map_err(|source| AetherError::LinkError {
                from: file.path(),
                to: path.clone(),
                source,
            })?;

            symlinked.push(path);
        }

        Ok(symlinked)
    }

    fn remove_files(&self) -> Result<Vec<PathBuf>, AetherError> {
        let files = &self.files;

        println!("files: {:#?}", files);

        let mut removed = vec![];

        for file in files {
            let name = match file.file_name() {
                Some(name) => name,
                None => return Err(AetherError::Unknown),
            };

            let path = pkg_dir().join(file);

            println!("removing: {}", &path.display());

            std::fs::remove_file(&path).map_err(|source| AetherError::WriteError {
                file: path.clone(),
                source,
            })?;

            removed.push(path);
        }

        Ok(removed)
    }

    pub fn unlink_execs(&self) -> Result<Vec<PathBuf>, AetherError> {
        let files = self.list_execs()?;
        let mut unlinked: Vec<PathBuf> = vec![];

        for file in files {
            let path = bin_dir().join(file.file_name());

            std::fs::remove_file(&path).map_err(|source| AetherError::WriteError {
                file: path.clone(),
                source,
            })?;

            unlinked.push(path);
        }

        Ok(unlinked)
    }
}

impl fmt::Display for Pkg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let info = &self.pkginfo;
        write!(
            f,
            "Pkg {{ name: {}, version: {}, desc: {}, size: {} }}",
            info.pkgname, info.pkgver, info.pkgdesc, info.size
        )
    }
}

// TODO: fix debug printing stuff ughhh
// impl Debug for Pkg {
//     fn fmt(&self, f: &mut Formatter) -> Result<(), AetherError> {
//         write!(
//             f,
//             "Pkg {{ files: {:?}, buildinfo: {:?}, mtree: {:?}, pkginfo: {:?} }}",
//             self.files,
//             self.buildinfo,
//             self.mtree.0.show(),
//             self.pkginfo
//         )
//     }
// }

#[derive(Clone, Debug)]
pub struct PkgList {
    pkgs: Vec<Pkg>,
}

impl PkgList {
    pub fn check_exec_conflicts(&mut self, pkg: Pkg) -> Result<Option<Vec<PathBuf>>, AetherError> {
        let mut test_pkglist = self.clone();

        test_pkglist.pkgs.push(pkg);

        let result = test_pkglist.exec_conflicts()?;

        Ok(result)
    }

    pub fn exec_conflicts(&self) -> Result<Option<Vec<PathBuf>>, AetherError> {
        let pkgs = &self.pkgs;

        let mut checked: Vec<PathBuf> = vec![];
        let mut conflicts: Vec<PathBuf> = vec![];
        for pkg in pkgs {
            for exec in pkg.list_execs()? {
                let name = &exec.file_name();

                if checked.iter().any(|x| x == name) {
                    conflicts.push(name.into())
                } else {
                    checked.push(name.into());
                }
            }
        }

        if conflicts.is_empty() {
            Ok(None)
        } else {
            Ok(Some(conflicts))
        }
    }

    pub fn install(&mut self, pkg: Pkg) -> Result<u64, AetherError> {
        let path = &pkg_dir();
        let to = &path.join(&pkg.get_refstr());

        self.install_to(pkg, to)
    }

    pub fn install_to(&mut self, pkg: Pkg, path: &dyn AsRef<Path>) -> Result<u64, AetherError> {
        if self
            .pkgs()
            .iter()
            .any(|x| (x.get_refstr() == pkg.get_refstr()))
        {
            return Err(AetherError::AlreadyExists(format!(
                "{} already exists in PkgList",
                pkg.get_refstr()
            )));
        }

        self.pkgs.push(pkg.clone());

        if let Some(conflicts) = self.exec_conflicts()? {
            return Err(AetherError::AlreadyExists(format!(
                "conflicts found in /bin or /usr/bin: {:?}",
                conflicts
            )));
        }

        let from: &Path = pkg.path.as_ref();
        let to: &Path = path.as_ref();
        let mut options = dir::CopyOptions::new();
        options.content_only = true;

        let result = dir::copy(from, to, &options).map_err(|source| AetherError::CopyError {
            from: from.into(),
            to: to.into(),
            source,
        })?;

        pkg.symlink_execs()?;

        Ok(result)
    }

    pub fn new() -> Result<Self, AetherError> {
        Self::new_from(&pkg_dir())
    }

    pub fn new_from(path: &dyn AsRef<Path>) -> Result<Self, AetherError> {
        let path = &path.as_ref();
        let mut pkgs: Vec<Pkg> = vec![];

        let paths = read_dir(path).map_err(|source| AetherError::ReadError {
            file: path.into(),
            source,
        })?;

        for path in paths {
            let path = path?;
            if let Ok(file_type) = path.file_type() {
                if file_type.is_dir() {
                    pkgs.push(Pkg::from_dir(&path.path())?);
                }
            }
        }

        Ok(Self { pkgs })
    }

    pub fn pkg_exists(&self, pkg: &Pkg) -> bool {
        self.into_iter()
            .any(|x| *pkg.get_refstr() == x.get_refstr())
    }

    #[must_use]
    pub fn pkgs(&self) -> &Vec<Pkg> {
        &self.pkgs
    }

    pub fn remove(&mut self, pkg: &Pkg) -> Result<(), AetherError> {
        let path = &pkg_dir();
        let from = &path.join(pkg.get_refstr());

        self.remove_from(pkg, from)
    }

    pub fn remove_from(&mut self, pkg: &Pkg, _path: &dyn AsRef<Path>) -> Result<(), AetherError> {
        let pkg = pkg.clone();

        if !self
            .pkgs()
            .iter()
            .any(|x| (x.get_refstr() == pkg.get_refstr()))
        {
            let name = pkg.pkginfo.pkgname;
            let ver = pkg.pkginfo.pkgver;
            return Err(AetherError::MissingPkg { name, ver });
        }

        match pkg.check_execs() {
            Ok(_) => {
                pkg.unlink_execs()?;
            }
            Err(AetherError::MissingExec(_)) => {
                // TODO: figure out how to communicate that a package is missing some executables
            }
            Err(err) => return Err(err),
        }

        match pkg.check_files() {
            Ok(_) => {
                println!("removing files");
                pkg.remove_files()?;
            }
            Err(AetherError::MissingFile(_)) => {
                // TODO: figure out how to communicate that a package is missing some files
            }
            Err(err) => return Err(err),
        }

        Ok(())
    }

    pub fn show_all(&mut self) {
        for pkg in &mut self.pkgs {
            pkg.show_all();
        }
    }
}

impl fmt::Display for PkgList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "packages [")?;

        for pkg in self.pkgs() {
            let refstr = pkg.get_refstr();
            let desc = &pkg.pkginfo.pkgdesc;

            writeln!(f, "    {}: \"{}\",", refstr, desc)?;
        }

        write!(f, "]")
    }
}

impl IntoIterator for PkgList {
    type Item = Pkg;
    type IntoIter = <Vec<Self::Item> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.pkgs.into_iter()
    }
}

impl<'a> IntoIterator for &'a PkgList {
    type Item = Pkg;
    type IntoIter = <Vec<Self::Item> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.pkgs.clone().into_iter()
    }
}

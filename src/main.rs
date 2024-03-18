extern crate fuser;
extern crate libc;
use clap::{arg, Command};
use clap::{Arg, Parser};
use fuser::FileType;
use fuser::MountOption;
use fuser::ReplyData;
use fuser::ReplyDirectory;
use fuser::ReplyEntry;
use fuser::{FileAttr, Filesystem, ReplyAttr, Request};
use jpegxl_rs::decode::Data;
use jpegxl_rs::decoder_builder;
use jpegxl_rs::image::ToDynamic;
use libc::{ENOENT, ENOSYS};
use log::info;
use log::warn;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs::DirEntry;
use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::{self, Duration};
struct JxlFilesystem {
    attrs: BTreeMap<u64, FileAttr>,
    inodes: BTreeMap<u64, PathBuf>,
    caches: BTreeMap<PathBuf, Vec<u8>>,
    basedir: PathBuf,
}
impl JxlFilesystem {
    fn new(Directory: PathBuf) -> JxlFilesystem {
        let mut attrs = BTreeMap::new();
        let mut inodes = BTreeMap::new();
        let mut caches = BTreeMap::new();
        let ts = SystemTime::now();
        let attr = FileAttr {
            ino: 1,
            size: 0,
            blocks: 0,
            atime: ts,
            mtime: ts,
            ctime: ts,
            crtime: ts,
            kind: FileType::Directory,
            perm: 0o755,
            nlink: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
            flags: 0,
            blksize: 0,
        };
        attrs.insert(1, attr);
        inodes.insert(1, Directory.clone());

        JxlFilesystem {
            attrs,
            inodes,
            caches,
            basedir: Directory,
        }
    }
}
impl Filesystem for JxlFilesystem {
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        println!("getattr(ino={})", ino);

        let ttl = Duration::new(1, 0);
        let f = self.attrs.get(&ino);
        match f {
            Some(attr) => println!("replied: {:?}", reply.attr(&ttl, attr)),
            None => reply.error(ENOSYS),
        }
    }

    fn read(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        size: u32,
        flags: i32,
        lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        warn!(
            "read(ino: {:#x?}, fh: {}, offset: {}, size: {}, \
            flags: {:#x?}, lock_owner: {:?})",
            ino, fh, offset, size, flags, lock_owner
        );
        let f = self.inodes.get(&ino);
        match f {
            Some(d) => {
                warn!("file : {:?}, extension: {:?}", d, d.extension());
                match d.extension() {
                    Some(ext) => {
                        if !ext.eq_ignore_ascii_case("jxl") {
                            let data = std::fs::read(d).unwrap();
                            let mut end = offset as usize + size as usize;
                            if end > data.len() {
                                end = data.len()
                            }
                            if offset as usize > data.len() {
                                reply.error(ENOSYS);
                                return;
                            }
                            warn!("replying original data");
                            reply.data(&data[offset as usize..end]);
                            return;
                        }
                    }
                    None => {
                        let data = std::fs::read(d).unwrap();
                        let mut end = offset as usize + size as usize;
                        if end > data.len() {
                            end = data.len()
                        }
                        if offset as usize > data.len() {
                            reply.error(ENOSYS);
                            return;
                        }
                        warn!("replying original data");
                        reply.data(&data[offset as usize..end]);
                        return;
                    }
                }
                let file = std::fs::read(d).unwrap();
                if let Some(jpeg) = self.caches.get(d) {
                    info!("replying with caches");
                    let mut end = offset as usize + size as usize;
                    if end > jpeg.len() {
                        end = jpeg.len()
                    }
                    if offset as usize > jpeg.len() {
                        reply.error(ENOSYS);
                        return;
                    }
                    reply.data(&jpeg[offset as usize..end]);
                    return;
                } else {
                    let mut decoder = decoder_builder().build().unwrap();
                    let (metadata, data) = decoder.reconstruct(&file).unwrap();
                    // warn!("content:{:?}", file);
                    decoder.decode_with::<u8>(&file);
                    match data {
                        Data::Jpeg(jpeg) => {
                            self.caches.insert(d.to_owned(), jpeg.clone());
                            /* do something with the JPEG data */
                            info!("replying");
                            //reply.data(String::from("hi").as_bytes());
                            let mut end = offset as usize + size as usize;
                            if end > jpeg.len() {
                                end = jpeg.len()
                            }
                            if offset as usize > jpeg.len() {
                                reply.error(ENOSYS);
                                return;
                            }
                            reply.data(&jpeg[offset as usize..end]);
                            return;
                        }
                        Data::Pixels(pixels) => {
                            /* do something with the pixels data */
                            match pixels {
                                jpegxl_rs::decode::Pixels::Float(_) => todo!(),
                                jpegxl_rs::decode::Pixels::Uint8(_) => todo!(),
                                jpegxl_rs::decode::Pixels::Uint16(_) => todo!(),
                                jpegxl_rs::decode::Pixels::Float16(_) => todo!(),
                            }
                        }
                    }
                }
            }
            None => reply.error(ENOSYS),
        }
        //reply.error(ENOSYS);
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        println!("lookup(parent={}, name={})", parent, name.to_str().unwrap());
        let mut path = PathBuf::new();
        if let Some(pat) = self.inodes.get(&parent) {
            path.push(pat);
        } else {
            reply.error(ENOENT);
            return;
        }
        warn!("lookup: {:?}", path);
        let cc = name.to_owned().into_string().unwrap();
        if cc.ends_with(".jxl.jpg") {
            path.push(cc.strip_suffix(".jpg").unwrap().to_owned());
        } else {
            path.push(name);
        }

        let meta = match std::fs::metadata(path) {
            Ok(e) => e,
            Err(_) => {
                reply.error(ENOENT);
                return;
            }
        };
        let ttl = Duration::new(0, 0);
        let ts = SystemTime::now();
        let kind = || -> FileType {
            if meta.is_dir() {
                return FileType::Directory;
            }
            if meta.is_file() {
                return FileType::RegularFile;
            }
            if meta.is_symlink() {
                return FileType::Symlink;
            }
            return FileType::RegularFile;
        }();
        let attr = FileAttr {
            ino: meta.ino(),
            size: meta.size(),
            blocks: meta.blocks(),
            atime: SystemTime::UNIX_EPOCH + Duration::from_secs(meta.atime().try_into().unwrap()),
            mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(meta.mtime().try_into().unwrap()),
            ctime: SystemTime::UNIX_EPOCH + Duration::from_secs(meta.ctime().try_into().unwrap()),
            crtime: meta.created().unwrap(),
            kind: kind,
            perm: meta.permissions().mode() as u16,
            nlink: meta.nlink() as u32,
            uid: meta.uid(),
            gid: meta.gid(),
            rdev: meta.rdev() as u32,
            flags: 0,
            blksize: meta.blksize() as u32,
        };
        reply.entry(&ttl, &attr, 0);
        //reply.error(ENOENT);
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        println!("readdir(ino={}, fh={}, offset={})", ino, fh, offset);

        let mut d = None;

        if ino == 1 {
            if offset == 0 {
                reply.add(1, 0, FileType::Directory, &Path::new("."));
                reply.add(1, 1, FileType::Directory, &Path::new(".."));
                // println!("{}", d.count());
                d = Some(std::fs::read_dir(self.basedir.clone()).unwrap());
            }
        } else if let Some(aa) = self.inodes.get(&ino) {
            if offset != 0 {
                reply.ok();
                return;
            }
            d = Some(aa.read_dir().unwrap());
        } else {
            reply.error(ENOENT);
            return;
        }
        if let Some(d) = d {
            d.enumerate()
                .for_each(|f: (usize, Result<DirEntry, std::io::Error>)| {
                    let (index, res) = f;
                    let name = res.unwrap().path();

                    let kind = || -> FileType {
                        if name.is_dir() {
                            return FileType::Directory;
                        };
                        if name.is_file() {
                            return FileType::RegularFile;
                        };
                        return FileType::RegularFile;
                    };

                    let meta = std::fs::metadata(name.clone()).unwrap();
                    let ttl = Duration::new(0, 0);
                    let ts = SystemTime::now();
                    let kind = || -> FileType {
                        if meta.is_dir() {
                            return FileType::Directory;
                        }
                        if meta.is_file() {
                            return FileType::RegularFile;
                        }
                        if meta.is_symlink() {
                            return FileType::Symlink;
                        }
                        return FileType::RegularFile;
                    }();

                    let mut attrib = FileAttr {
                        ino: meta.ino(),
                        size: meta.size(),
                        blocks: meta.blocks(),
                        atime: SystemTime::UNIX_EPOCH
                            + Duration::from_secs(meta.atime().try_into().unwrap()),
                        mtime: SystemTime::UNIX_EPOCH
                            + Duration::from_secs(meta.mtime().try_into().unwrap()),
                        ctime: SystemTime::UNIX_EPOCH
                            + Duration::from_secs(meta.ctime().try_into().unwrap()),
                        crtime: meta.created().unwrap(),
                        kind: kind,
                        perm: meta.permissions().mode() as u16,
                        nlink: meta.nlink() as u32,
                        uid: meta.uid(),
                        gid: meta.gid(),
                        rdev: meta.rdev() as u32,
                        flags: 0,
                        blksize: meta.blksize() as u32,
                    };
                    let namee = name.clone();
                    let jxlname = move || -> OsString {
                        return match namee.extension() {
                            Some(namea) => {
                                info!("namea : {:?}", namea);
                                if namea == OsString::from("jxl") {
                                    return namee
                                        .with_extension("jxl.jpg")
                                        .file_name()
                                        .unwrap()
                                        .to_owned();
                                }
                                return namee.file_name().unwrap().to_owned();
                            }
                            None => namee.file_name().unwrap().to_owned(),
                        };
                    }();
                    if jxlname.clone().into_string().unwrap().ends_with(".jxl.jpg") {
                        // let file = std::fs::read(name.clone()).unwrap();
                        // let mut decoder = decoder_builder().build().unwrap();
                        // let (metadata, data) = decoder.reconstruct(&file).unwrap();
                        // let size = match data {
                        //     Data::Jpeg(dat) => dat.len(),
                        //     Data::Pixels(dat) => 0,
                        // };
                        // attrib.size = size as u64;
                        attrib.size = u32::MAX as u64;
                    }
                    self.attrs.insert(attrib.ino, attrib);
                    self.inodes.insert(attrib.ino, name.clone().into());
                    info!(
                        "pathname: {:?},offset: {}, kind: {:?}",
                        name,
                        2 + index as i64,
                        kind
                    );
                    reply.add(1, 2 + index as i64, kind, &jxlname);
                });
        }
        reply.ok();
        return;
    }
}

#[derive(Parser)] // requires `derive` feature
#[command(version, about, long_about = None)]
struct Cli {
    #[arg()]
    basedir: PathBuf,

    #[arg()]
    mountpoint: PathBuf,
}
fn main() {
    env_logger::init();
    let args = Cli::parse();
    let basedir: PathBuf = args.basedir;
    let mountpoint: PathBuf = args.mountpoint;
    // let basedir: Result<PathBuf, &str> = match env::args().nth(1) {
    //     Some(path) => {
    //         let mut f = PathBuf::new();
    //         f.push(path);
    //         match f.is_dir() {
    //             true => Ok(f),
    //             false => Err("None"),
    //         }
    //     }
    //     None => Err("None"),
    // };
    // if basedir.is_err() {
    //     println!("Cannot access basedir.");
    //     return;
    // }
    // let mountpoint = match env::args().nth(2) {
    //     Some(path) => path,
    //     None => {
    //         println!(
    //             "Usage: {} <BASEDIR> <MOUNTPOINT>",
    //             env::args().nth(0).unwrap()
    //         );
    //         return;
    //     }
    // };
    fuser::mount2(
        JxlFilesystem::new(basedir),
        mountpoint,
        &[
            MountOption::AutoUnmount,
            MountOption::AllowOther,
            MountOption::CUSTOM("direct_io".to_string()),
        ],
    )
    .unwrap();
}

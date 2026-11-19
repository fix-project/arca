#![allow(unsafe_op_in_unsafe_fn)]
use crate::{doorbell::VMToHostDoorBell, prelude::*};

use alloc::format;
use common::hypercall;

pub struct HostLogger;

impl log::Log for HostLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            crate::interrupts::critical(|| {
                let level = record.level() as u8;
                let target = (
                    crate::vm::ka2pa(record.target().as_ptr()),
                    record.target().len(),
                );
                let file = record
                    .file()
                    .map(|x| (crate::vm::ka2pa(x.as_ptr()), x.len()));
                let line = record.line();
                let module_path = record
                    .module_path()
                    .map(|x| (crate::vm::ka2pa(x.as_ptr()), x.len()));
                let msg = format!("{}", record.args());
                let message = (crate::vm::ka2pa(msg.as_ptr()), msg.len());
                let record = common::LogRecord {
                    level,
                    target,
                    file,
                    line,
                    module_path,
                    message,
                };
                let p = crate::vm::ka2pa(&raw const record);
                unsafe {
                    crate::io::hypercall1(hypercall::LOG, p as u64);
                }
            });
        }
    }

    fn flush(&self) {
        todo!()
    }
}

pub static HOST: HostLogger = HostLogger;

pub fn symname(addr: *const ()) -> Option<(String, usize)> {
    unsafe {
        let mut buffer: Box<[u8]> = Box::new_uninit_slice(1024).assume_init();
        loop {
            let parts = (vm::ka2pa(buffer.as_mut_ptr()), buffer.len());
            let mut symtab = common::SymtabRecord {
                addr: addr as usize,
                file_buffer: parts,
                ..Default::default()
            };
            let p = crate::vm::ka2pa(&raw mut symtab);
            crate::io::hypercall1(hypercall::SYMNAME, p as u64);
            if !symtab.found {
                return None;
            }
            if symtab.file_len > buffer.len() {
                buffer = Box::new_uninit_slice(buffer.len() * 2).assume_init();
                continue;
            }
            let bytes = &buffer[..symtab.file_len];
            let name = core::str::from_utf8(bytes).expect("got back invalid UTF-8 from host");
            return Some((name.into(), addr as usize - symtab.addr));
        }
    }
}

pub fn memset(region: *mut [u8], value: u8) {
    let (p, n) = region.to_raw_parts();
    let p = vm::ka2pa(p);
    unsafe {
        crate::io::hypercall3(hypercall::MEMSET, p as u64, value as u64, n as u64);
    }
}

pub fn memclr(region: *mut [u8]) {
    let (p, n) = region.to_raw_parts();
    let p = vm::ka2pa(p);
    unsafe {
        crate::io::hypercall2(hypercall::MEMCLR, p as u64, n as u64);
    }
}

pub mod os {
    pub fn node_identity() -> [u8; 16] {
        match crate::pipe::HOST
            .get()
            .unwrap()
            .request(&common::protocol::control::Request::NodeIdentity)
        {
            common::protocol::control::Response::NodeIdentity(value) => value,
            _ => panic!("invalid runtime identity response"),
        }
    }
    pub fn cpu_time() -> Option<u64> {
        match crate::pipe::HOST
            .get()
            .unwrap()
            .request(&common::protocol::control::Request::CpuTime)
        {
            common::protocol::control::Response::CpuTime(value) => value,
            _ => None,
        }
    }

    pub fn evaluation_step(step: common::protocol::control::EvaluationStep) {
        let _ = crate::pipe::HOST
            .get()
            .unwrap()
            .request(&common::protocol::control::Request::EvaluationStep(step));
    }

    pub fn observe_evaluation() -> bool {
        matches!(
            crate::pipe::HOST
                .get()
                .unwrap()
                .request(&common::protocol::control::Request::ObserveEvaluation),
            common::protocol::control::Response::Bool(true)
        )
    }

    pub fn evaluation_progress(snapshot: common::protocol::control::EvaluationSnapshot) {
        let _ = crate::pipe::HOST.get().unwrap().request(
            &common::protocol::control::Request::EvaluationProgress(snapshot),
        );
    }

    pub fn sleep(milliseconds: u64) {
        let _ = crate::pipe::HOST
            .get()
            .unwrap()
            .request(&common::protocol::control::Request::Sleep(milliseconds));
    }

    pub fn tree_progress(task: [u8; 32], children: Vec<[u8; 32]>, completed: u64, total: u64) {
        let _ = crate::pipe::HOST.get().unwrap().request(
            &common::protocol::control::Request::TreeProgress {
                task,
                children,
                completed,
                total,
            },
        );
    }

    pub fn task_finished(
        task: [u8; 32],
        operation: Option<common::protocol::control::EvaluationOperation>,
    ) {
        let _ = crate::pipe::HOST
            .get()
            .unwrap()
            .request(&common::protocol::control::Request::TaskFinished { task, operation });
    }

    pub fn task_advice(
        task: [u8; 32],
        operation: Option<common::protocol::control::EvaluationOperation>,
        key: &[u8],
        payload: &[u8],
    ) {
        let _ = crate::pipe::HOST.get().unwrap().request(
            &common::protocol::control::Request::TaskAdvice {
                task,
                operation,
                key: key.to_vec(),
                payload: payload.to_vec(),
            },
        );
    }
    use crate::prelude::*;
    use common::protocol::*;

    pub fn ssh_sign(identity: &str, namespace: &str, message: &[u8]) -> Result<String, String> {
        let buffer = crate::allocator::buffer(Vec::from(message));
        match crate::pipe::HOST
            .get()
            .unwrap()
            .request_serialized(&control::Request::SshSign {
                identity: identity.into(),
                namespace: namespace.into(),
                message: buffer.slice(..),
            }) {
            control::Response::Signature(signature) => Ok(signature),
            control::Response::SignatureError(error) => Err(error),
            control::Response::Err(error) => Err(alloc::format!("{error:?}")),
            _ => Err("invalid SSH signing response".into()),
        }
    }

    pub fn argv() -> Vec<String> {
        let host = crate::pipe::HOST.get().unwrap();
        let control::Response::Args(args) = host.request(&control::Request::GetArgs) else {
            panic!("bad response");
        };
        args
    }

    pub fn set_current_dir(path: &str) -> Result<(), control::ErrorKind> {
        match crate::pipe::HOST
            .get()
            .unwrap()
            .request(&control::Request::SetCurrentDirectory(path.into()))
        {
            control::Response::Ack => Ok(()),
            control::Response::Err(error) => Err(error.into()),
            _ => Err(control::ErrorKind::Other),
        }
    }
}

use super::pipe::HostPipe;
unsafe fn get_pipe(data: common::protocol::control::PipeData) -> HostPipe {
    use common::pipe::{Pipe, Reader, Writer};
    let rxp: *const u8 = BuddyAllocator.from_offset(data.rx_ptr);
    let txp: *const u8 = BuddyAllocator.from_offset(data.tx_ptr);
    let rx = Arc::from_raw_in(core::ptr::from_raw_parts(rxp, data.rx_len), BuddyAllocator);
    let rx = Reader::from_inner(rx);
    let tx = Arc::from_raw_in(core::ptr::from_raw_parts(txp, data.tx_len), BuddyAllocator);
    let tx = Writer::from_inner(tx);
    let rx_avail = VMToHostDoorBell::from_raw_parts(data.rx_avail);
    let tx_avail = VMToHostDoorBell::from_raw_parts(data.tx_avail);
    let pipe = Pipe::from_inner(rx, tx, rx_avail, tx_avail);
    HostPipe::new(pipe)
}

pub mod net {
    use super::get_pipe;
    use crate::pipe::*;
    use crate::prelude::*;
    use common::protocol::*;
    use control::ErrorKind;

    pub struct TcpListener {
        pipe: KMutex<ListenerPipe>,
    }

    pub struct Stream {
        id: u64,
    }

    pub type TcpStream = Stream;

    impl TcpListener {
        pub fn bind(ip: &[u8; 4], port: u16) -> TcpListener {
            Self::try_bind(ip, port).expect("host TCP listen failed")
        }

        pub fn try_bind(ip: &[u8; 4], port: u16) -> Result<TcpListener, ErrorKind> {
            let host = crate::pipe::HOST.get().unwrap();
            let ip = *ip;
            match host.request(&control::Request::Listen { ip, port }) {
                control::Response::Pipe(id) => Ok(TcpListener {
                    pipe: KMutex::new(unsafe { ListenerPipe::new(get_pipe(id)) }),
                }),
                control::Response::Err(error) => Err(error.into()),
                _ => Err(ErrorKind::Other),
            }
        }

        pub fn local_addr(&self) -> Result<([u8; 4], u16), ErrorKind> {
            match self.pipe.lock().request(&listener::Request::LocalAddress) {
                listener::Response::Address(ip, port) => Ok((ip, port)),
                listener::Response::Err(error) => Err(error.into()),
                _ => Err(ErrorKind::Other),
            }
        }

        pub fn accept(&self) -> TcpStream {
            self.try_accept().expect("host TCP accept failed")
        }

        pub fn try_accept(&self) -> Result<TcpStream, ErrorKind> {
            match self.pipe.lock().request(&listener::Request::Accept) {
                listener::Response::Stream(id) => Ok(TcpStream { id }),
                listener::Response::Err(error) => Err(error.into()),
                _ => Err(ErrorKind::Other),
            }
        }
    }

    impl Drop for TcpListener {
        fn drop(&mut self) {
            let mut pipe = self.pipe.lock();
            let listener::Response::Ack = pipe.request(&listener::Request::Close) else {
                todo!();
            };
        }
    }

    impl Stream {
        pub fn from_id(id: u64) -> Self {
            Self { id }
        }
        pub fn connect(hostname: &str, port: u16) -> TcpStream {
            Self::try_connect(hostname, port).expect("host TCP connect failed")
        }

        pub fn try_connect(hostname: &str, port: u16) -> Result<TcpStream, ErrorKind> {
            let host = crate::pipe::HOST.get().unwrap();
            Self::from_response(host.request(&control::Request::Connect {
                host: hostname.into(),
                port,
            }))
        }

        pub fn ssh_connect(
            host: &str,
            repository: &str,
            scheduler: Option<&str>,
            collector: Option<&str>,
        ) -> Result<Stream, ErrorKind> {
            Self::from_response(crate::pipe::HOST.get().unwrap().request(
                &control::Request::SshConnect {
                    host: host.into(),
                    repository: repository.into(),
                    scheduler: scheduler.map(str::to_owned),
                    collector: collector.map(str::to_owned),
                },
            ))
        }

        pub fn stdio_stream() -> Result<Stream, ErrorKind> {
            Self::from_response(
                crate::pipe::HOST
                    .get()
                    .unwrap()
                    .request(&control::Request::StdioStream),
            )
        }

        pub fn open_uri(uri: &str) -> Result<Stream, ErrorKind> {
            let host = crate::pipe::HOST.get().unwrap();
            Self::from_response(host.request(&control::Request::OpenUri(uri.into())))
        }

        fn from_response(response: control::Response) -> Result<TcpStream, ErrorKind> {
            match response {
                control::Response::Stream(id) => Ok(TcpStream { id }),
                control::Response::Err(error) => Err(error.into()),
                _ => Err(ErrorKind::Other),
            }
        }

        pub fn send(&self, bytes: &[u8]) -> usize {
            self.try_send(bytes).expect("host TCP send failed")
        }

        pub fn try_send(&self, bytes: &[u8]) -> Result<usize, ErrorKind> {
            match crate::pipe::HOST
                .get()
                .unwrap()
                .request(&control::Request::StreamSend {
                    id: self.id,
                    bytes: bytes.into(),
                }) {
                control::Response::Length(len) if len <= bytes.len() => Ok(len),
                control::Response::Err(error) => Err(error.into()),
                _ => Err(ErrorKind::Other),
            }
        }

        pub fn recv(&self, bytes: &mut [u8]) -> usize {
            self.try_recv(bytes).expect("host TCP receive failed")
        }

        pub fn try_recv(&self, bytes: &mut [u8]) -> Result<usize, ErrorKind> {
            match crate::pipe::HOST
                .get()
                .unwrap()
                .request(&control::Request::StreamReceive {
                    id: self.id,
                    length: bytes.len(),
                }) {
                control::Response::StreamBytes(buf) if buf.len() <= bytes.len() => {
                    bytes[..buf.len()].copy_from_slice(&buf);
                    Ok(buf.len())
                }
                control::Response::Err(error) => Err(error.into()),
                _ => Err(ErrorKind::Other),
            }
        }

        pub fn shutdown(&self) {
            let _ = crate::pipe::HOST
                .get()
                .unwrap()
                .request(&control::Request::StreamClose(self.id));
        }

        pub fn close(self) {
            self.shutdown();
        }
    }

    impl Drop for TcpStream {
        fn drop(&mut self) {
            self.shutdown();
        }
    }

    impl core::fmt::Write for TcpStream {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let mut s = s.as_bytes();
            while !s.is_empty() {
                let len = self.send(s);
                s = &s[len..];
            }
            Ok(())
        }
    }
}

pub mod fs {
    use super::get_pipe;
    use crate::pipe::*;
    use alloc::{boxed::Box, string::String, vec, vec::Vec};
    use common::buffer::Buffer;
    pub use common::protocol::file::Whence;
    use common::{
        protocol::control::{ErrorKind, FileMode},
        protocol::*,
    };
    use core::ops::{Deref, DerefMut};

    pub struct File {
        pipe: FilePipe,
    }

    /// Recursively creates a directory and all of its parents on the host, like
    /// `mkdir -p`.  Mirrors [`File::open`], but the host has nothing to stream
    /// back, so success is a bare ack and failure carries an [`ErrorKind`].
    pub fn mkdir(path: &str) -> Result<(), ErrorKind> {
        let host = crate::pipe::HOST.get().unwrap();
        match host.request(&control::Request::Mkdir(path.into())) {
            control::Response::Ack => Ok(()),
            control::Response::Err(e) => Err(e.into()),
            _ => Err(ErrorKind::Other),
        }
    }

    fn request(request: control::Request) -> Result<control::Response, ErrorKind> {
        request_serialized(&request)
    }

    fn request_serialized(request: &impl serde::Serialize) -> Result<control::Response, ErrorKind> {
        match crate::pipe::HOST.get().unwrap().request_serialized(request) {
            control::Response::Err(e) => Err(e.into()),
            response => Ok(response),
        }
    }

    fn ack(request_value: control::Request) -> Result<(), ErrorKind> {
        match request(request_value)? {
            control::Response::Ack => Ok(()),
            _ => Err(ErrorKind::Other),
        }
    }

    pub fn is_directory(path: &str) -> Result<bool, ErrorKind> {
        match request(control::Request::IsDirectory(path.into()))? {
            control::Response::Bool(value) => Ok(value),
            _ => Err(ErrorKind::Other),
        }
    }

    pub fn exists(path: &str) -> Result<bool, ErrorKind> {
        match request(control::Request::Exists(path.into()))? {
            control::Response::Bool(value) => Ok(value),
            _ => Err(ErrorKind::Other),
        }
    }

    pub fn read(path: &str) -> Result<Vec<u8>, ErrorKind> {
        let control::Response::Bytes(region) = request(control::Request::ReadFile(path.into()))?
        else {
            return Err(ErrorKind::Other);
        };
        let (bytes, _) = Box::into_raw_with_allocator(unsafe { region.into_owned() });
        // The global allocator accepts page-aligned Buddy allocations.
        Ok(unsafe { Box::from_raw(bytes) }.into_vec())
    }

    pub fn read_range(path: &str, offset: u64, length: usize) -> Result<Vec<u8>, ErrorKind> {
        let mut file = File::open(path, true, false, false, false, false)?;
        file.seek(Whence::Start(offset));
        let mut buffer = crate::allocator::buffer(vec![0; length]);
        let mut read = 0;
        while read < length {
            let count = file.try_read(&mut buffer.slice_mut(read..))?;
            if count == 0 {
                return Err(ErrorKind::InvalidData);
            }
            read += count;
        }
        Ok(buffer.to_vec())
    }

    pub fn write(path: &str, bytes: &[u8]) -> Result<(), ErrorKind> {
        write_with(
            path,
            bytes,
            FileMode {
                write: true,
                create: true,
                truncate: true,
                ..FileMode::default()
            },
        )
    }

    pub fn write_with(path: &str, bytes: &[u8], mode: FileMode) -> Result<(), ErrorKind> {
        let buffer = crate::allocator::buffer(Vec::from(bytes));
        match request_serialized(&control::Request::WriteFile(
            path.into(),
            buffer.slice(..),
            mode,
        ))? {
            control::Response::Ack => Ok(()),
            _ => Err(ErrorKind::Other),
        }
    }

    pub fn read_dir(path: &str) -> Result<Vec<String>, ErrorKind> {
        match request(control::Request::ReadDirectory(path.into()))? {
            control::Response::Names(names) => Ok(names),
            _ => Err(ErrorKind::Other),
        }
    }

    pub fn read_directory_uri(uri: &str) -> Result<Vec<control::DirectoryEntry>, ErrorKind> {
        match request(control::Request::ReadDirectoryUri(uri.into()))? {
            control::Response::DirectoryEntries(entries) => Ok(entries),
            _ => Err(ErrorKind::Other),
        }
    }

    pub fn rename(from: &str, to: &str) -> Result<(), ErrorKind> {
        ack(control::Request::Rename(from.into(), to.into()))
    }

    pub fn symlink(target: &str, path: &str) -> Result<(), ErrorKind> {
        ack(control::Request::Symlink(target.into(), path.into()))
    }

    pub fn canonicalize(path: &str) -> Result<String, ErrorKind> {
        match request(control::Request::Canonicalize(path.into()))? {
            control::Response::Path(path) => Ok(path),
            _ => Err(ErrorKind::Other),
        }
    }

    pub fn read_link(path: &str) -> Result<String, ErrorKind> {
        match request(control::Request::ReadLink(path.into()))? {
            control::Response::Path(target) => Ok(target),
            _ => Err(ErrorKind::Other),
        }
    }

    pub fn remove_file(path: &str) -> Result<(), ErrorKind> {
        ack(control::Request::RemoveFile(path.into()))
    }

    impl File {
        pub fn open(
            path: &str,
            read: bool,
            write: bool,
            create: bool,
            append: bool,
            truncate: bool,
        ) -> Result<File, ErrorKind> {
            Self::open_with(
                path,
                FileMode {
                    read,
                    write,
                    create,
                    append,
                    truncate,
                    create_new: false,
                },
            )
        }

        pub fn open_with(path: &str, mode: FileMode) -> Result<File, ErrorKind> {
            Self::from_response(request(control::Request::Open(path.into(), mode))?)
        }

        pub fn stdio(stream: control::StandardStream) -> Result<File, ErrorKind> {
            Self::from_response(request(control::Request::Stdio(stream))?)
        }

        fn from_response(response: control::Response) -> Result<File, ErrorKind> {
            match response {
                control::Response::Pipe(id) => unsafe {
                    Ok(File {
                        pipe: FilePipe::new(get_pipe(id)),
                    })
                },
                control::Response::Err(e) => Err(e.into()),
                _ => Err(ErrorKind::Other),
            }
        }

        pub fn close(self) {}

        pub fn read<B>(&mut self, buf: &mut Buffer<B>) -> usize
        where
            Buffer<B>: DerefMut<Target = [u8]>,
        {
            self.try_read(buf).expect("host file read failed")
        }

        pub fn try_read<B>(&mut self, buf: &mut Buffer<B>) -> Result<usize, ErrorKind>
        where
            Buffer<B>: DerefMut<Target = [u8]>,
        {
            match self
                .pipe
                .request_serialized(&file::Request::Read(buf.slice_mut(..)))
            {
                file::Response::Length(len) if len <= buf.len() => Ok(len),
                file::Response::Err(e) => Err(e.into()),
                _ => Err(ErrorKind::Other),
            }
        }

        pub fn read_exact<B>(&mut self, buf: &mut Buffer<B>) -> usize
        where
            Buffer<B>: DerefMut<Target = [u8]>,
        {
            let mut total = 0;
            while total < buf.len() {
                let n = self.read::<&mut [u8]>(&mut buf.slice_mut(total..));
                if n == 0 {
                    break;
                }
                total += n;
            }
            total
        }

        pub fn write<B>(&mut self, buf: &Buffer<B>) -> usize
        where
            Buffer<B>: Deref<Target = [u8]>,
        {
            self.try_write(buf).expect("host file write failed")
        }

        pub fn try_write<B>(&mut self, buf: &Buffer<B>) -> Result<usize, ErrorKind>
        where
            Buffer<B>: Deref<Target = [u8]>,
        {
            match self
                .pipe
                .request_serialized(&file::Request::Write(buf.slice(..)))
            {
                file::Response::Length(len) if len <= buf.len() => Ok(len),
                file::Response::Err(e) => Err(e.into()),
                _ => Err(ErrorKind::Other),
            }
        }

        pub fn try_read_to_end(&mut self) -> Result<Vec<u8>, ErrorKind> {
            let mut bytes = Vec::new();
            let mut buffer = crate::allocator::buffer(vec![0; 65536]);
            loop {
                let len = self.try_read(&mut buffer)?;
                if len == 0 {
                    return Ok(bytes);
                }
                bytes.extend_from_slice(&buffer[..len]);
            }
        }

        pub fn try_write_all(&mut self, bytes: &[u8]) -> Result<(), ErrorKind> {
            let buffer = crate::allocator::buffer(Vec::from(bytes));
            let mut offset = 0;
            while offset < buffer.len() {
                let len = self.try_write(&buffer.slice(offset..))?;
                if len == 0 {
                    return Err(ErrorKind::Other);
                }
                offset += len;
            }
            Ok(())
        }

        pub fn write_exact<B>(&mut self, buf: &Buffer<B>) -> usize
        where
            Buffer<B>: Deref<Target = [u8]>,
        {
            let mut total = 0;
            while total < buf.len() {
                let n = self.write::<&[u8]>(&buf.slice(total..));
                if n == 0 {
                    break;
                }
                total += n;
            }
            total
        }

        pub fn seek(&mut self, whence: Whence) -> u64 {
            let file::Response::Offset(offset) = self.pipe.request(&file::Request::Seek(whence))
            else {
                panic!("bad response");
            };
            offset
        }
    }

    impl Drop for File {
        fn drop(&mut self) {
            let file::Response::Ack = self.pipe.request(&file::Request::Close) else {
                panic!("bad response");
            };
        }
    }

    impl core::fmt::Write for File {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let bytes = crate::allocator::buffer(alloc::vec::Vec::from(s.as_bytes()));
            if self.write_exact(&bytes) != bytes.len() {
                return Err(core::fmt::Error);
            }
            Ok(())
        }
    }

    impl !Sync for File {}
}

pub mod session {
    use common::protocol::{
        control::{ErrorKind, Request, Response},
        session::Command,
    };
    pub fn next() -> Result<Option<Command>, ErrorKind> {
        match crate::pipe::HOST
            .get()
            .unwrap()
            .request(&Request::SessionNext)
        {
            Response::SessionCommand(command) => Ok(command),
            Response::Err(error) => Err(error.into()),
            _ => Err(ErrorKind::Other),
        }
    }
    pub fn reply(
        connection: common::protocol::session::Id,
        request: u64,
        status: u8,
        output: alloc::vec::Vec<u8>,
    ) {
        crate::pipe::HOST
            .get()
            .unwrap()
            .request(&Request::SessionReply {
                connection,
                request,
                status,
                output,
            });
    }
}

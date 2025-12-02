use alloc::format;
use common::ipaddr::IpAddr;
use kernel::prelude::*;
use vfs::{File, Open};

use crate::proc::Namespace;

pub async fn connect_to(ns: &Arc<Namespace>, addr: IpAddr) -> Result<Box<dyn File>, ()> {
    let tcp_ctl_result = ns.walk("/net/tcp/clone", vfs::Open::ReadWrite).await;
    let mut tcp_ctl = match tcp_ctl_result {
        Ok(obj) => match obj.as_file() {
            Ok(file) => file,
            Err(_) => {
                log::error!("Failed to get TCP control file");
                return Err(());
            }
        },
        Err(e) => {
            log::error!("Failed to walk to /net/tcp/clone: {:?}", e);
            return Err(());
        }
    };

    // Read the connection ID
    let mut id_buf = [0u8; 32];
    let size = match tcp_ctl.read(&mut id_buf).await {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to read connection ID: {:?}", e);
            return Err(());
        }
    };
    let conn_id = match core::str::from_utf8(&id_buf[..size]) {
        Ok(s) => s.trim(),
        Err(_) => {
            log::error!("Invalid UTF-8 in connection ID");
            return Err(());
        }
    };

    // Connect to localhost:11212
    let connect_cmd = alloc::format!("connect {}\n", addr);
    if let Err(e) = tcp_ctl.write(connect_cmd.as_bytes()).await {
        log::error!("Failed to send connect command: {:?}", e);
        return Err(());
    }

    // Get the data file for this connection
    let data_path = alloc::format!("/net/tcp/{}/data", conn_id);
    let data_file = match ns.walk(&data_path, vfs::Open::ReadWrite).await {
        Ok(obj) => match obj.as_file() {
            Ok(file) => file,
            Err(_) => {
                log::error!("Failed to get data file");
                return Err(());
            }
        },
        Err(e) => {
            log::error!("Failed to walk to data path: {:?}", e);
            return Err(());
        }
    };

    Ok(data_file)
}

pub async fn listen_on(ns: &Arc<Namespace>, addr: IpAddr) -> Result<String, ()> {
    // Get id from /tcp/clone
    let mut tcp_ctl = ns
        .walk("/net/tcp/clone", Open::ReadWrite)
        .await
        .unwrap()
        .as_file()
        .unwrap();
    let mut id = [0; 32];
    let size = tcp_ctl.read(&mut id).await.unwrap();
    let id = core::str::from_utf8(&id[..size]).unwrap().trim();
    log::info!("TCP connection ID string: {}", id);
    let id: usize = id.parse().unwrap();

    log::info!("Got TCP connection ID: {}", id);

    // Listen to incoming connections on the specified tcp_port
    let tcp_port_str = addr.to_string();
    log::info!("Listening on TCP port: {}", tcp_port_str);
    let tcp_announcement = alloc::format!("announce {}\n", tcp_port_str);
    log::info!("Announcing on: {}", tcp_announcement.trim());
    match tcp_ctl.write(tcp_announcement.as_bytes()).await {
        Ok(bytes_written) => {
            log::info!("Successfully wrote {} bytes to tcp control", bytes_written);
        }
        Err(e) => {
            log::error!("Failed to announce TCP listener: {:?}", e);
            return Err(());
        }
    }

    let listen_path = format!("/net/tcp/{id}/listen");
    log::info!("Listen path: {}", listen_path);
    Ok(listen_path)
}

pub async fn get_one_incoming_connection(
    ns: &Arc<Namespace>,
    listen_path: String,
) -> Result<Box<dyn File>, ()> {
    log::info!("Waiting for connections...");
    let mut listen_file = ns
        .walk(&listen_path, Open::ReadWrite)
        .await
        .unwrap()
        .as_file()
        .unwrap();

    // Accept a new connection - this blocks until someone connects
    let mut id = [0; 32];
    let size = listen_file.read(&mut id).await.unwrap();
    let id = core::str::from_utf8(&id[..size]).unwrap().trim();
    let data_path = format!("/net/tcp/{id}/data");

    // TODO is this connection closed on Ctrl-C?
    log::info!("New connection accepted: {}", id);

    // Get data file for the accepted connection
    let mut data_file = ns
        .walk(&data_path, Open::ReadWrite)
        .await
        .unwrap()
        .as_file()
        .unwrap();

    Ok(data_file)
}

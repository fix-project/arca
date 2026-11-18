use ssh_key::{Algorithm, HashAlg, PublicKey, Signature, SshSig};
use std::{
    env, io,
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
};

pub fn sign(identity: &str, namespace: &str, message: &[u8]) -> io::Result<String> {
    sign_with_agent(&agent_socket()?, identity, namespace, message)
}

pub fn identities() -> io::Result<Vec<String>> {
    identities_from_agent(&agent_socket()?)
}

fn agent_socket() -> io::Result<PathBuf> {
    env::var_os("SSH_AUTH_SOCK")
        .filter(|socket| !socket.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(
            io::ErrorKind::NotFound,
            "no SSH agent found: SSH_AUTH_SOCK is unset or empty; start ssh-agent, then run ssh-add",
        ))
}

pub fn identities_from_agent(socket: &Path) -> io::Result<Vec<String>> {
    let response = exchange(socket, &[11])?;
    let Some((&12, mut bytes)) = response.split_first() else {
        return Err(io::ErrorKind::InvalidData.into());
    };
    let count = take_u32(&mut bytes)?;
    let mut keys = Vec::new();
    for _ in 0..count {
        let blob = take_string(&mut bytes)?;
        take_string(&mut bytes)?;
        let mut fields = blob;
        if take_string(&mut fields)? == b"ssh-ed25519" {
            let key = PublicKey::from_bytes(blob).map_err(signature_error)?;
            keys.push(key.to_openssh().map_err(signature_error)?);
        }
    }
    if !bytes.is_empty() {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(keys)
}

pub fn sign_with_agent(
    socket: &Path,
    identity: &str,
    namespace: &str,
    message: &[u8],
) -> io::Result<String> {
    let key = PublicKey::from_openssh(identity).map_err(signature_error)?;
    if key.algorithm() != Algorithm::Ed25519 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "expected an Ed25519 identity",
        ));
    }
    let data = SshSig::signed_data(namespace, HashAlg::Sha512, message).map_err(signature_error)?;
    let mut request = vec![13];
    push_string(&mut request, &key.to_bytes().map_err(signature_error)?)?;
    push_string(&mut request, &data)?;
    request.extend_from_slice(&0u32.to_be_bytes());
    let response = exchange(socket, &request)?;
    if response == [5] {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "ssh-agent refused to sign",
        ));
    }
    if response[0] != 14 || response.len() < 5 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let length = u32::from_be_bytes(response[1..5].try_into().unwrap()) as usize;
    if length != response.len() - 5 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let signature = Signature::try_from(&response[5..]).map_err(signature_error)?;
    let signature = SshSig::new(
        key.key_data().clone(),
        namespace,
        HashAlg::Sha512,
        signature,
    )
    .map_err(signature_error)?;
    key.verify(namespace, message, &signature)
        .map_err(signature_error)?;
    signature
        .to_pem(ssh_key::LineEnding::LF)
        .map_err(signature_error)
}

fn exchange(socket: &Path, request: &[u8]) -> io::Result<Vec<u8>> {
    use io::{Read, Write};
    let mut agent = UnixStream::connect(socket).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "cannot connect to SSH agent at {}: {error}; check SSH_AUTH_SOCK",
                socket.display()
            ),
        )
    })?;
    agent.write_all(
        &u32::try_from(request.len())
            .map_err(io::Error::other)?
            .to_be_bytes(),
    )?;
    agent.write_all(request)?;
    let mut length = [0; 4];
    agent.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > 1 << 20 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut response = vec![0; length];
    agent.read_exact(&mut response)?;
    Ok(response)
}

fn take_u32(bytes: &mut &[u8]) -> io::Result<u32> {
    let (value, rest) = bytes
        .split_at_checked(4)
        .ok_or(io::ErrorKind::InvalidData)?;
    *bytes = rest;
    Ok(u32::from_be_bytes(value.try_into().unwrap()))
}

fn take_string<'a>(bytes: &mut &'a [u8]) -> io::Result<&'a [u8]> {
    let length = take_u32(bytes)? as usize;
    let (value, rest) = bytes
        .split_at_checked(length)
        .ok_or(io::ErrorKind::InvalidData)?;
    *bytes = rest;
    Ok(value)
}

fn signature_error(error: ssh_key::Error) -> io::Error {
    io::Error::other(error.to_string())
}

fn push_string(out: &mut Vec<u8>, bytes: &[u8]) -> io::Result<()> {
    out.extend_from_slice(
        &u32::try_from(bytes.len())
            .map_err(io::Error::other)?
            .to_be_bytes(),
    );
    out.extend_from_slice(bytes);
    Ok(())
}

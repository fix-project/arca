use ssh_key::{Algorithm, HashAlg, PublicKey, Signature, SshSig};
use std::{env, io, os::unix::net::UnixStream, path::Path};

pub fn sign(identity: &str, namespace: &str, message: &[u8]) -> io::Result<String> {
    let socket = env::var_os("SSH_AUTH_SOCK")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "SSH_AUTH_SOCK is not set"))?;
    sign_with_agent(Path::new(&socket), identity, namespace, message)
}

pub fn sign_with_agent(
    socket: &Path,
    identity: &str,
    namespace: &str,
    message: &[u8],
) -> io::Result<String> {
    use io::{Read, Write};
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
    let mut agent = UnixStream::connect(socket)?;
    agent.write_all(
        &u32::try_from(request.len())
            .map_err(io::Error::other)?
            .to_be_bytes(),
    )?;
    agent.write_all(&request)?;
    let mut length = [0; 4];
    agent.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > 4096 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut response = vec![0; length];
    agent.read_exact(&mut response)?;
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
